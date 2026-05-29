use serde::{Deserialize, Serialize};
use crate::fs::metadata::FileEntry;
use uuid::Uuid;

// Handshake
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerHelloPayload {
    pub protocol_version: u8,
    pub max_frame_size: u32,
    pub supported_encodings: Vec<u8>,
    pub supported_compression: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientHelloPayload {
    pub selected_encoding: u8,
    pub selected_compression: u8,
}

// Auth
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthSessionRequest {
    pub session_id: Uuid,
}

// Directory
#[derive(Debug, Serialize, Deserialize)]
pub struct ListDirectoryRequest {
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DirectoryListResponse {
    pub path: String,
    pub entries: Vec<FileEntry>,
}

// Chunks (For MessagePack wrapping if needed, though raw is preferred)
#[derive(Debug, Serialize, Deserialize)]
pub struct ChunkHeader {
    pub transfer_id: Uuid,
    pub chunk_index: u32,
    pub offset: u64,
    pub chunk_len: u32,
}

// Errors
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub code: u16,
    pub error_kind: String,
    pub safe_message: String,
    pub request_id: Uuid,
    pub retryable: bool,
}
