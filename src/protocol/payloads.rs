use serde::{Deserialize, Serialize};
use crate::fs::metadata::FileEntry;

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthSessionRequest {
    pub session_id: uuid::Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListDirectoryRequest {
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DirectoryListResponse {
    pub path: String,
    pub entries: Vec<FileEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub code: u16,
    pub message: String,
}
