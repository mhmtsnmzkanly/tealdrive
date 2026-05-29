use serde::{Deserialize, Serialize};

use crate::config::{RelativePath, RootId};
use crate::errors::ErrorKind;
use crate::protocol::frame::RequestId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperRequest {
    pub request_id: RequestId,
    pub username: String,
    pub uid: u32,
    pub gid: u32,
    pub command: HelperCommand,
    pub root_id: RootId,
    pub relative_path: RelativePath,
    pub target_relative_path: Option<RelativePath>,
    pub policy_context: HelperPolicyContext,
    pub limits: HelperLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HelperCommand {
    ListDirectory,
    ReadFile,
    WriteFile,
    CreateFolder,
    Rename,
    Move,
    Copy,
    MoveToTrash,
    RestoreFromTrash,
    DeletePermanently,
    Chmod,
    CompressZip,
    CompressTarGz,
    ExtractArchive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperPolicyContext {
    pub read_only_root: bool,
    pub is_web_root: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperLimits {
    pub max_directory_page_size: usize,
    pub max_text_edit_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperResponse {
    pub success: bool,
    pub error_kind: Option<ErrorKind>,
    pub safe_message: Option<String>,
    pub os_error_code: Option<i32>,
    pub bytes_read: u64,
    pub bytes_written: u64,
    pub audit_metadata: Option<HelperAuditMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperAuditMetadata {
    pub operation: HelperCommand,
    pub status: String,
}
