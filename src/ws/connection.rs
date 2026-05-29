use axum::extract::ws::{Message, WebSocket};
use crate::state::SharedState;
use crate::protocol::frame::{FrameHeader, HEADER_SIZE};
use crate::protocol::message_type::MessageType;
use crate::protocol::payloads::{ListDirectoryRequest, DirectoryListResponse, ErrorPayload};
use crate::fs::operations::FsOperations;
use crate::policy::RootPolicy;
use bytes::BytesMut;
use futures::{SinkExt, StreamExt};
use tracing::{debug, warn, error};
use rmp_serde;

use crate::auth::session::Session;

pub struct WsConnection {
    socket: WebSocket,
    state: SharedState,
    buffer: BytesMut,
    session: Option<Session>,
}

impl WsConnection {
    pub fn new(socket: WebSocket, state: SharedState) -> Self {
        Self {
            socket,
            state,
            buffer: BytesMut::with_capacity(65536),
            session: None,
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        while let Some(msg) = self.socket.next().await {
            let msg = msg?;
            match msg {
                Message::Binary(data) => {
                    self.buffer.extend_from_slice(&data);
                    self.process_buffer().await?;
                }
                Message::Close(_) => break,
                Message::Ping(p) => {
                    self.socket.send(Message::Pong(p)).await?;
                }
                _ => {
                    warn!("Received non-binary message on binary WebSocket");
                }
            }
        }
        Ok(())
    }

    async fn process_buffer(&mut self) -> anyhow::Result<()> {
        while self.buffer.len() >= HEADER_SIZE {
            let mut payload_len_bytes = [0u8; 4];
            payload_len_bytes.copy_from_slice(&self.buffer[28..32]);
            let payload_len = u32::from_be_bytes(payload_len_bytes) as usize;

            if self.buffer.len() < HEADER_SIZE + payload_len {
                return Ok(());
            }

            match FrameHeader::decode(&mut self.buffer)? {
                Some(header) => {
                    let payload = self.buffer.split_to(payload_len);
                    self.handle_frame(header, payload).await?;
                }
                None => break,
            }
        }
        Ok(())
    }

    async fn handle_frame(&mut self, header: FrameHeader, payload: BytesMut) -> anyhow::Result<()> {
        debug!("Handling frame: {:?}", header.message_type);

        match header.message_type {
            MessageType::Ping => {
                self.send_response(MessageType::Pong, header.request_id, ()).await
            }
            MessageType::AuthSession => {
                let req: crate::protocol::payloads::AuthSessionRequest = rmp_serde::from_slice(&payload)?;
                if let Some(session) = self.state.sessions.get_session(&req.session_id) {
                    self.session = Some(session);
                    self.send_response(MessageType::ServerHello, header.request_id, ()).await
                } else {
                    let err_resp = ErrorPayload {
                        code: 401,
                        message: "Invalid session".to_string(),
                    };
                    self.send_response(MessageType::Error, header.request_id, err_resp).await
                }
            }
            MessageType::ListDirectory => {
                let session = match &self.session {
                    Some(s) => s,
                    None => {
                        let err_resp = ErrorPayload {
                            code: 401,
                            message: "Unauthorized".to_string(),
                        };
                        return self.send_response(MessageType::Error, header.request_id, err_resp).await;
                    }
                };

                let req: ListDirectoryRequest = rmp_serde::from_slice(&payload)?;
                let root_policy = RootPolicy::new(&session.username); 
                
                match FsOperations::list_directory(&req.path, &root_policy) {
                    Ok(entries) => {
                        let resp = DirectoryListResponse {
                            path: req.path,
                            entries,
                        };
                        self.send_response(MessageType::DirectoryList, header.request_id, resp).await
                    }
                    Err(e) => {
                        error!("List directory error: {:?}", e);
                        let err_resp = ErrorPayload {
                            code: 403,
                            message: e.to_string(),
                        };
                        self.send_response(MessageType::Error, header.request_id, err_resp).await
                    }
                }
            }
            _ => {
                warn!("Unhandled message type: {:?}", header.message_type);
                Ok(())
            }
        }
    }

    async fn send_response<T: serde::Serialize>(
        &mut self,
        msg_type: MessageType,
        request_id: uuid::Uuid,
        payload: T,
    ) -> anyhow::Result<()> {
        let payload_bytes = rmp_serde::to_vec(&payload)?;
        let header = FrameHeader::new(msg_type, request_id, payload_bytes.len() as u32);
        
        let mut buf = BytesMut::with_capacity(HEADER_SIZE + payload_bytes.len());
        header.encode(&mut buf);
        buf.extend_from_slice(&payload_bytes);
        
        self.socket.send(Message::Binary(buf.to_vec())).await?;
        Ok(())
    }
}
