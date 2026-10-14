//! Optional Noise record transport underneath the existing TDRV protocol.
//! No application bytes are dispatched until the required channel is complete.
use crate::{config::AppConfig, errors::TealDriveError};
use axum::extract::ws::{Message, WebSocket};
use futures_util::{Sink, Stream};
use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
};
const PREFIX: &[u8; 4] = b"TDN1";
pub struct Socket {
    socket: WebSocket,
    transport: Option<tealdrive_channel::Transport>,
    outgoing: VecDeque<Message>,
    partial: Vec<u8>,
    limit: usize,
}
fn failure() -> axum::Error {
    axum::Error::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "Encrypted channel rejected",
    ))
}
impl Socket {
    pub async fn accept(mut socket: WebSocket, cfg: &AppConfig) -> Result<Self, TealDriveError> {
        let transport = if let Some(path) = &cfg.server.channel_key_path {
            let key = zeroize::Zeroizing::new(
                crate::tls::pem(path, true).map_err(|_| TealDriveError::Validation)?,
            );
            if key.len() != 32 {
                return Err(TealDriveError::Validation);
            }
            let public =
                tealdrive_channel::public_key(&key).map_err(|_| TealDriveError::Validation)?;
            let mut greeting = PREFIX.to_vec();
            greeting.extend(public);
            socket
                .send(Message::Binary(greeting))
                .await
                .map_err(|_| TealDriveError::Protocol)?;
            let message = tokio::time::timeout(std::time::Duration::from_secs(15), socket.recv())
                .await
                .map_err(|_| TealDriveError::Protocol)?;
            let Some(Ok(Message::Binary(bytes))) = message else {
                return Err(TealDriveError::Protocol);
            };
            if bytes.len() != 48 {
                return Err(TealDriveError::Protocol);
            }
            let mut handshake =
                tealdrive_channel::responder(&key).map_err(|_| TealDriveError::Protocol)?;
            let mut plain = [0; 128];
            if handshake
                .read_message(&bytes, &mut plain)
                .map_err(|_| TealDriveError::Protocol)?
                != 0
            {
                return Err(TealDriveError::Protocol);
            }
            let n = handshake
                .write_message(&[], &mut plain)
                .map_err(|_| TealDriveError::Protocol)?;
            socket
                .send(Message::Binary(plain[..n].to_vec()))
                .await
                .map_err(|_| TealDriveError::Protocol)?;
            Some(
                handshake
                    .into_transport_mode()
                    .map_err(|_| TealDriveError::Protocol)?,
            )
        } else {
            None
        };
        Ok(Self {
            socket,
            transport,
            outgoing: VecDeque::new(),
            partial: Vec::new(),
            limit: cfg.limits.max_frame_payload + crate::protocol::header::HEADER_LEN,
        })
    }
    pub async fn send(&mut self, message: Message) -> Result<(), axum::Error> {
        futures_util::SinkExt::send(self, message).await
    }
    pub async fn recv(&mut self) -> Option<Result<Message, axum::Error>> {
        futures_util::StreamExt::next(self).await
    }
    fn flush_queue(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), axum::Error>> {
        while !self.outgoing.is_empty() {
            match Pin::new(&mut self.socket).poll_ready(cx) {
                Poll::Ready(Ok(())) => {}
                other => return other,
            }
            let message = self.outgoing.pop_front().expect("nonempty queue");
            Pin::new(&mut self.socket).start_send(message)?;
        }
        Pin::new(&mut self.socket).poll_flush(cx)
    }
}
impl Stream for Socket {
    type Item = Result<Message, axum::Error>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            let message = match Pin::new(&mut this.socket).poll_next(cx) {
                Poll::Ready(Some(Ok(message))) => message,
                other => return other,
            };
            let Some(transport) = this.transport.as_mut() else {
                return Poll::Ready(Some(Ok(message)));
            };
            match message {
                Message::Binary(cipher) => {
                    if cipher.len() > tealdrive_channel::MAX_RECORD {
                        return Poll::Ready(Some(Err(failure())));
                    }
                    let mut plain = vec![0; tealdrive_channel::MAX_RECORD];
                    let n = match transport.read_message(&cipher, &mut plain) {
                        Ok(n) => n,
                        Err(_) => return Poll::Ready(Some(Err(failure()))),
                    };
                    if !(2..=tealdrive_channel::MAX_FRAGMENT + 1).contains(&n)
                        || (plain[0] == 0 && n != tealdrive_channel::MAX_FRAGMENT + 1)
                        || plain[0] > 1
                        || this.partial.len().saturating_add(n - 1) > this.limit
                    {
                        return Poll::Ready(Some(Err(failure())));
                    }
                    this.partial.extend_from_slice(&plain[1..n]);
                    if plain[0] == 1 {
                        return Poll::Ready(Some(Ok(Message::Binary(std::mem::take(
                            &mut this.partial,
                        )))));
                    }
                }
                Message::Text(_) => return Poll::Ready(Some(Err(failure()))),
                other => return Poll::Ready(Some(Ok(other))),
            }
        }
    }
}
impl Sink<Message> for Socket {
    type Error = axum::Error;
    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.get_mut().flush_queue(cx)
    }
    fn start_send(self: Pin<&mut Self>, message: Message) -> Result<(), Self::Error> {
        let this = self.get_mut();
        if let (Some(transport), Message::Binary(bytes)) = (&mut this.transport, &message) {
            if bytes.is_empty() || bytes.len() > this.limit {
                return Err(failure());
            }
            let chunks = bytes.chunks(tealdrive_channel::MAX_FRAGMENT);
            let count = chunks.len();
            for (index, chunk) in chunks.enumerate() {
                let mut plain = zeroize::Zeroizing::new(Vec::with_capacity(chunk.len() + 1));
                plain.push(u8::from(index + 1 == count));
                plain.extend_from_slice(chunk);
                let mut cipher = vec![0; tealdrive_channel::MAX_RECORD];
                let n = transport
                    .write_message(&plain, &mut cipher)
                    .map_err(|_| failure())?;
                cipher.truncate(n);
                this.outgoing.push_back(Message::Binary(cipher));
            }
        } else {
            if this.transport.is_some() && matches!(message, Message::Text(_)) {
                return Err(failure());
            }
            this.outgoing.push_back(message);
        }
        Ok(())
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.get_mut().flush_queue(cx)
    }
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.as_mut().get_mut().flush_queue(cx) {
            Poll::Ready(Ok(())) => Pin::new(&mut self.get_mut().socket).poll_close(cx),
            other => other,
        }
    }
}
