use serde::{Deserialize, Serialize};

use crate::config::UserContext;
use crate::limits::Limits;
use crate::policy::feature_gate::FeatureFlags;
use crate::protocol::compression::Compression;
use crate::protocol::frame::TdrvFrame;
use crate::protocol::header::Encoding;
use crate::protocol::message_type::MessageType;
use crate::session::SessionId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsConnectionContext {
    pub session_id: SessionId,
    pub username: String,
    pub uid: u32,
    pub gid: u32,
    pub user_agent: Option<String>,
    pub ip: Option<String>,
    pub connected_at: u64,
    pub protocol_ready: bool,
    pub selected_protocol: Option<WsSelectedProtocol>,
    pub features: FeatureFlags,
}

impl WsConnectionContext {
    pub fn user_context(&self) -> UserContext {
        UserContext {
            username: self.username.clone(),
            uid: self.uid,
            gid: self.gid,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsConnectionState {
    pub context: WsConnectionContext,
    pub protocol_state: WsProtocolState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsProtocolState {
    WaitingForClientHello,
    Ready,
    Closing,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsHandshakeState {
    ServerHelloSent,
    ClientHelloReceived,
    ProtocolReadySent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WsCapabilities {
    pub protocol_version: u8,
    pub supported_encodings: Vec<Encoding>,
    pub supported_compressions: Vec<Compression>,
    pub max_frame_size: usize,
    pub max_control_payload_size: usize,
    pub max_chunk_size: usize,
    pub feature_flags: FeatureFlags,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WsSelectedProtocol {
    pub protocol_version: u8,
    pub selected_encoding: Encoding,
    pub selected_compression: Compression,
    pub effective_limits: Limits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsInboundFrame {
    pub frame: TdrvFrame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsOutboundFrame {
    pub frame: TdrvFrame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboundFrameKind {
    Handshake(MessageType),
    Operation(MessageType),
    Protocol(MessageType),
}

pub fn classify_message(message_type: MessageType) -> InboundFrameKind {
    if message_type.is_handshake() {
        InboundFrameKind::Handshake(message_type)
    } else if message_type.is_operation() {
        InboundFrameKind::Operation(message_type)
    } else {
        InboundFrameKind::Protocol(message_type)
    }
}
