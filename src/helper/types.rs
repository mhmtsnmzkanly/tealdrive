use serde::{Deserialize, Serialize};

use crate::config::{RelativePath, RootId};
use crate::errors::ErrorKind;
use crate::protocol::frame::{RequestId, TransferId};
use crate::protocol::schema::{
    DirectoryCursor, FileCapabilities, FileKind, FilterMode, GroupName, OwnerName, SortMode,
    UnixMode,
};

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
    pub list_options: Option<HelperListDirectoryOptions>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HelperCommand {
    ListDirectory,
    FileMetadata,
    DownloadFile,
    ReadTextFile,
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
    pub max_chunk_size: usize,
    pub max_download_helper_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperListDirectoryOptions {
    pub cursor: Option<DirectoryCursor>,
    pub limit: usize,
    pub include_hidden: bool,
    pub sort: SortMode,
    pub filter: FilterMode,
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
    pub payload: HelperResponsePayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HelperResponsePayload {
    None,
    DirectoryList(HelperDirectoryList),
    FileMetadata(HelperDirectoryEntry),
    DownloadFile(HelperDownloadFile),
    TextFileContent(HelperTextFileContent),
    TrashEntry(HelperTrashEntry),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperTrashEntry {
    pub trash_id: String,
    pub display_name: String,
    pub deleted_at: u64,
    pub original_relative_path: RelativePath,
    pub original_root_id: RootId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperTextFileContent {
    pub name: String,
    pub content: String,
    pub encoding: String,
    pub size: u64,
    pub modified: Option<String>,
    pub owner: Option<OwnerName>,
    pub group: Option<GroupName>,
    pub mode: Option<UnixMode>,
    pub permissions: Option<String>,
    pub is_sensitive: bool,
    pub is_hidden: bool,
    pub is_read_only: bool,
    pub truncated: bool,
    pub line_count: Option<usize>,
    pub language_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperDownloadFile {
    pub transfer_id: TransferId,
    pub total_bytes: u64,
    pub chunks: Vec<HelperDownloadChunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperDownloadChunk {
    pub chunk_index: u32,
    pub offset: u64,
    pub chunk_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperDirectoryList {
    pub entries: Vec<HelperDirectoryEntry>,
    pub has_more: bool,
    pub next_cursor: Option<DirectoryCursor>,
    pub total_estimate: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperDirectoryEntry {
    pub name: String,
    pub file_type: FileKind,
    pub size: u64,
    pub modified: Option<String>,
    pub owner: Option<OwnerName>,
    pub group: Option<GroupName>,
    pub mode: Option<UnixMode>,
    pub permissions: Option<String>,
    pub is_hidden: bool,
    pub is_sensitive: bool,
    pub is_protected: bool,
    pub is_symlink: bool,
    pub symlink_target: Option<RelativePath>,
    pub is_web_root: bool,
    pub is_world_writable: bool,
    pub capabilities: FileCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperAuditMetadata {
    pub request_id: RequestId,
    pub command: HelperCommand,
    pub root_id: RootId,
    pub relative_path: RelativePath,
    pub target_relative_path: Option<RelativePath>,
    pub bytes_read: u64,
    pub bytes_written: u64,
    pub status: String,
    pub reason: Option<String>,
}

impl HelperResponse {
    pub fn not_implemented(request: &HelperRequest) -> Self {
        Self::error(
            request,
            ErrorKind::NotImplemented,
            "Helper command is not implemented yet.",
            "not_implemented",
        )
    }

    pub fn feature_disabled(request: &HelperRequest) -> Self {
        Self::error(
            request,
            ErrorKind::FeatureDisabled,
            "Helper command is disabled in TealDrive V1.",
            "feature_disabled",
        )
    }

    pub fn permission_denied(request: &HelperRequest, message: &'static str) -> Self {
        Self::error(
            request,
            ErrorKind::PermissionDenied,
            message,
            "permission_denied",
        )
    }

    fn error(
        request: &HelperRequest,
        error_kind: ErrorKind,
        safe_message: &'static str,
        reason: &'static str,
    ) -> Self {
        Self {
            success: false,
            error_kind: Some(error_kind),
            safe_message: Some(safe_message.to_owned()),
            os_error_code: None,
            bytes_read: 0,
            bytes_written: 0,
            audit_metadata: Some(HelperAuditMetadata {
                request_id: request.request_id,
                command: request.command,
                root_id: request.root_id.clone(),
                relative_path: request.relative_path.clone(),
                target_relative_path: request.target_relative_path.clone(),
                bytes_read: 0,
                bytes_written: 0,
                status: "denied".to_owned(),
                reason: Some(reason.to_owned()),
            }),
            payload: HelperResponsePayload::None,
        }
    }

    pub fn directory_list(request: &HelperRequest, list: HelperDirectoryList) -> Self {
        Self {
            success: true,
            error_kind: None,
            safe_message: None,
            os_error_code: None,
            bytes_read: 0,
            bytes_written: 0,
            audit_metadata: Some(HelperAuditMetadata {
                request_id: request.request_id,
                command: request.command,
                root_id: request.root_id.clone(),
                relative_path: request.relative_path.clone(),
                target_relative_path: request.target_relative_path.clone(),
                bytes_read: 0,
                bytes_written: 0,
                status: "success".to_owned(),
                reason: None,
            }),
            payload: HelperResponsePayload::DirectoryList(list),
        }
    }

    pub fn file_metadata(request: &HelperRequest, entry: HelperDirectoryEntry) -> Self {
        Self {
            success: true,
            error_kind: None,
            safe_message: None,
            os_error_code: None,
            bytes_read: 0,
            bytes_written: 0,
            audit_metadata: Some(HelperAuditMetadata {
                request_id: request.request_id,
                command: request.command,
                root_id: request.root_id.clone(),
                relative_path: request.relative_path.clone(),
                target_relative_path: request.target_relative_path.clone(),
                bytes_read: 0,
                bytes_written: 0,
                status: "success".to_owned(),
                reason: None,
            }),
            payload: HelperResponsePayload::FileMetadata(entry),
        }
    }

    pub fn download_file(request: &HelperRequest, download: HelperDownloadFile) -> Self {
        let bytes_read = download.total_bytes;
        Self {
            success: true,
            error_kind: None,
            safe_message: None,
            os_error_code: None,
            bytes_read,
            bytes_written: 0,
            audit_metadata: Some(HelperAuditMetadata {
                request_id: request.request_id,
                command: request.command,
                root_id: request.root_id.clone(),
                relative_path: request.relative_path.clone(),
                target_relative_path: request.target_relative_path.clone(),
                bytes_read,
                bytes_written: 0,
                status: "success".to_owned(),
                reason: None,
            }),
            payload: HelperResponsePayload::DownloadFile(download),
        }
    }

    pub fn text_file_content(request: &HelperRequest, content: HelperTextFileContent) -> Self {
        let bytes_read = content.size;
        Self {
            success: true,
            error_kind: None,
            safe_message: None,
            os_error_code: None,
            bytes_read,
            bytes_written: 0,
            audit_metadata: Some(HelperAuditMetadata {
                request_id: request.request_id,
                command: request.command,
                root_id: request.root_id.clone(),
                relative_path: request.relative_path.clone(),
                target_relative_path: request.target_relative_path.clone(),
                bytes_read,
                bytes_written: 0,
                status: "success".to_owned(),
                reason: None,
            }),
            payload: HelperResponsePayload::TextFileContent(content),
        }
    }

    pub fn trash_entry(request: &HelperRequest, entry: HelperTrashEntry) -> Self {
        Self {
            success: true,
            error_kind: None,
            safe_message: Some("Moved to trash.".to_owned()),
            os_error_code: None,
            bytes_read: 0,
            bytes_written: 0,
            audit_metadata: Some(HelperAuditMetadata {
                request_id: request.request_id,
                command: request.command,
                root_id: request.root_id.clone(),
                relative_path: request.relative_path.clone(),
                target_relative_path: request.target_relative_path.clone(),
                bytes_read: 0,
                bytes_written: 0,
                status: "success".to_owned(),
                reason: None,
            }),
            payload: HelperResponsePayload::TrashEntry(entry),
        }
    }

    pub fn operation_done(request: &HelperRequest, safe_message: &'static str) -> Self {
        Self {
            success: true,
            error_kind: None,
            safe_message: Some(safe_message.to_owned()),
            os_error_code: None,
            bytes_read: 0,
            bytes_written: 0,
            audit_metadata: Some(HelperAuditMetadata {
                request_id: request.request_id,
                command: request.command,
                root_id: request.root_id.clone(),
                relative_path: request.relative_path.clone(),
                target_relative_path: request.target_relative_path.clone(),
                bytes_read: 0,
                bytes_written: 0,
                status: "success".to_owned(),
                reason: None,
            }),
            payload: HelperResponsePayload::None,
        }
    }
}

pub fn command_disabled_in_v1(command: HelperCommand) -> bool {
    matches!(
        command,
        HelperCommand::DeletePermanently | HelperCommand::Chmod | HelperCommand::ExtractArchive
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RootId;
    use crate::helper::ipc::tests_support::sample_request;
    use crate::protocol::schema::CapabilitySet;

    #[test]
    fn command_enum_msgpack_roundtrip() {
        let command = HelperCommand::MoveToTrash;
        let bytes = rmp_serde::to_vec_named(&command).expect("encode");
        let decoded: HelperCommand = rmp_serde::from_slice(&bytes).expect("decode");

        assert_eq!(decoded, command);
    }

    #[test]
    fn disabled_command_response_shape() {
        let request = sample_request(HelperCommand::DeletePermanently);
        let response = HelperResponse::feature_disabled(&request);

        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::FeatureDisabled));
        assert_eq!(
            response
                .audit_metadata
                .as_ref()
                .map(|meta| meta.reason.as_deref()),
            Some(Some("feature_disabled"))
        );
    }

    #[test]
    fn audit_metadata_creation() {
        let request = sample_request(HelperCommand::ListDirectory);
        let response = HelperResponse::not_implemented(&request);
        let metadata = response.audit_metadata.expect("metadata");

        assert_eq!(metadata.request_id, request.request_id);
        assert_eq!(metadata.command, HelperCommand::ListDirectory);
        assert_eq!(metadata.root_id, RootId::new("home"));
    }

    #[test]
    fn directory_payload_response_shape() {
        let request = sample_request(HelperCommand::ListDirectory);
        let list = HelperDirectoryList {
            entries: vec![HelperDirectoryEntry {
                name: "file.txt".to_owned(),
                file_type: FileKind::File,
                size: 4,
                modified: None,
                owner: None,
                group: None,
                mode: None,
                permissions: None,
                is_hidden: false,
                is_sensitive: false,
                is_protected: false,
                is_symlink: false,
                symlink_target: None,
                is_web_root: false,
                is_world_writable: false,
                capabilities: CapabilitySet::default(),
            }],
            has_more: false,
            next_cursor: None,
            total_estimate: Some(1),
        };
        let response = HelperResponse::directory_list(&request, list);

        assert!(response.success);
        assert!(matches!(
            response.payload,
            HelperResponsePayload::DirectoryList(_)
        ));
    }

    #[test]
    fn file_metadata_payload_response_shape() {
        let request = sample_request(HelperCommand::FileMetadata);
        let entry = HelperDirectoryEntry {
            name: "file.txt".to_owned(),
            file_type: FileKind::File,
            size: 4,
            modified: None,
            owner: None,
            group: None,
            mode: None,
            permissions: None,
            is_hidden: false,
            is_sensitive: false,
            is_protected: false,
            is_symlink: false,
            symlink_target: None,
            is_web_root: false,
            is_world_writable: false,
            capabilities: CapabilitySet::default(),
        };
        let response = HelperResponse::file_metadata(&request, entry);

        assert!(response.success);
        assert!(matches!(
            response.payload,
            HelperResponsePayload::FileMetadata(_)
        ));
    }

    #[test]
    fn download_payload_response_shape() {
        let request = sample_request(HelperCommand::DownloadFile);
        let download = HelperDownloadFile {
            transfer_id: TransferId::new(),
            total_bytes: 3,
            chunks: vec![HelperDownloadChunk {
                chunk_index: 0,
                offset: 0,
                chunk_bytes: b"abc".to_vec(),
            }],
        };
        let response = HelperResponse::download_file(&request, download);

        assert!(response.success);
        assert_eq!(response.bytes_read, 3);
        assert!(matches!(
            response.payload,
            HelperResponsePayload::DownloadFile(_)
        ));
    }

    #[test]
    fn text_file_content_payload_response_shape() {
        let request = sample_request(HelperCommand::ReadTextFile);
        let content = HelperTextFileContent {
            name: "notes.txt".to_owned(),
            content: "hello".to_owned(),
            encoding: "utf-8".to_owned(),
            size: 5,
            modified: None,
            owner: None,
            group: None,
            mode: None,
            permissions: None,
            is_sensitive: false,
            is_hidden: false,
            is_read_only: false,
            truncated: false,
            line_count: Some(1),
            language_hint: Some("text".to_owned()),
        };
        let response = HelperResponse::text_file_content(&request, content);

        assert!(response.success);
        assert_eq!(response.bytes_read, 5);
        assert!(matches!(
            response.payload,
            HelperResponsePayload::TextFileContent(_)
        ));
    }
}
