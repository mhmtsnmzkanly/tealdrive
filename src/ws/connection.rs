use axum::extract::ws::{Message, WebSocket};
use crate::state::SharedState;
use crate::protocol::frame::{FrameHeader, HEADER_SIZE};
use crate::protocol::message_type::MessageType;
use crate::protocol::payloads::*;
use crate::fs::operations::FsOperations;
use crate::policy::RootPolicy;
use crate::auth::session::Session;
use bytes::{BytesMut, BufMut};
use futures::{SinkExt, StreamExt};
use tracing::{debug, warn, error, info};
use rmp_serde;
use uuid::Uuid;

#[derive(PartialEq)]
enum ConnectionState {
    Negotiating,
    Authenticated,
    Ready,
}

pub struct WsConnection {
    socket: WebSocket,
    state: SharedState,
    buffer: BytesMut,
    session: Option<Session>,
    conn_state: ConnectionState,
}

impl WsConnection {
    pub fn new(socket: WebSocket, state: SharedState) -> Self {
        Self {
            socket,
            state,
            buffer: BytesMut::with_capacity(65536),
            session: None,
            conn_state: ConnectionState::Negotiating,
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        // Step 4: Server sends SERVER_HELLO
        self.send_server_hello().await?;

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

    async fn send_server_hello(&mut self) -> anyhow::Result<()> {
        let payload = ServerHelloPayload {
            protocol_version: 1,
            max_frame_size: 1024 * 1024, // 1 MB
            supported_encodings: vec![1], // 1 = MessagePack
            supported_compression: vec![0, 1], // 0 = None, 1 = Gzip
        };
        debug!("Sending SERVER_HELLO");
        self.send_response(MessageType::ServerHello, Uuid::nil(), payload).await
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
            MessageType::ClientHello => {
                let _req: ClientHelloPayload = rmp_serde::from_slice(&payload)?;
                // In V1, we just accept whatever for now or validate
                self.send_response(MessageType::ProtocolReady, header.request_id, ()).await?;
                self.conn_state = ConnectionState::Ready;
                Ok(())
            }
            MessageType::AuthSession => {
                let req: AuthSessionRequest = rmp_serde::from_slice(&payload)?;
                if let Some(session) = self.state.sessions.get_session(&req.session_id) {
                    self.session = Some(session);
                    info!("WebSocket bound to session: {}", req.session_id);
                    self.conn_state = ConnectionState::Authenticated;
                    // According to handshake flow: 
                    // 1. Server Hello (Done)
                    // 2. Client Hello (Wait for it)
                    // Wait, the user said:
                    // 1. HTTP login (Done)
                    // 2. Browser opens WS (Done)
                    // 3. WS upgrade validates cookie (Done in handler)
                    // 4. Server sends SERVER_HELLO (Done in run)
                    // 5. Client sends CLIENT_HELLO
                    // 6. Server sends PROTOCOL_READY
                    // 7. ACCEPT FILE OPS
                    // Where does AuthSession fit? 
                    // Usually right after upgrade or as first message.
                    // Let's assume it's part of the flow.
                    self.send_response(MessageType::ConnectionStatus, header.request_id, "Authenticated").await
                } else {
                    self.send_error(header.request_id, 401, "InvalidSession", "Invalid session ID").await
                }
            }
            MessageType::ListDirectory => {
                if self.conn_state != ConnectionState::Ready {
                    return self.send_error(header.request_id, 403, "ProtocolError", "Protocol not ready").await;
                }
                
                let session = match &self.session {
                    Some(s) => s,
                    None => return self.send_error(header.request_id, 401, "Unauthorized", "Not authenticated").await,
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
                        self.send_error(header.request_id, 403, "PolicyDenied", &e.to_string()).await
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
        request_id: Uuid,
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

    async fn send_error(
        &mut self,
        request_id: Uuid,
        code: u16,
        kind: &str,
        message: &str,
    ) -> anyhow::Result<()> {
        let payload = ErrorPayload {
            code,
            error_kind: kind.to_string(),
            safe_message: message.to_string(),
            request_id,
            retryable: false,
        };
        self.send_response(MessageType::Error, request_id, payload).await
    }
}
