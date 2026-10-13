use serde::{Deserialize, Serialize};

use crate::config::{RelativePath, RootId};
use crate::errors::{ErrorPayload, TealDriveError};
use crate::limits::Limits;
use crate::policy::feature_gate::FeatureFlags;
use crate::protocol::compression::Compression;
use crate::protocol::frame::{RequestId, TransferId};
use crate::protocol::header::Encoding;
use crate::protocol::schema::{
    CapabilitySet, DirectoryCursor, FileCapabilities, FileKind, FilterMode, GroupName, OwnerName,
    SortMode, UnixMode,
};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthenticationCapabilities {
    pub web_password: bool,
    pub password_change: bool,
    pub self_revoke: bool,
    pub account_management: bool,
    pub account_pagination: bool,
    pub linux_approval: bool,
    pub policy_management: bool,
    pub virtual_accounts: bool,
    pub virtual_delete: bool,
    pub enrollment: bool,
    pub self_registration: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerHelloPayload {
    #[serde(default)]
    pub authentication: AuthenticationCapabilities,
    pub protocol_version: u8,
    pub server_name: String,
    pub selected_encoding: Option<Encoding>,
    pub supported_encodings: Vec<Encoding>,
    pub supported_compressions: Vec<Compression>,
    pub max_frame_size: usize,
    pub max_control_payload_size: usize,
    pub max_chunk_size: usize,
    pub feature_flags: FeatureFlags,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientHelloPayload {
    pub protocol_version: u8,
    pub supported_encodings: Vec<Encoding>,
    pub supported_compressions: Vec<Compression>,
    pub max_frame_size: usize,
    pub max_control_payload_size: usize,
    pub max_chunk_size: usize,
    pub feature_flags: FeatureFlags,
}

impl ClientHelloPayload {
    pub fn validate(&self, limits: &Limits) -> Result<(), TealDriveError> {
        if self.max_frame_size > limits.max_frame_payload
            || self.max_control_payload_size > limits.max_control_payload
            || self.max_chunk_size > limits.max_chunk_size
        {
            return Err(TealDriveError::Validation);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolReadyPayload {
    pub protocol_version: u8,
    pub selected_encoding: Encoding,
    pub selected_compression: Compression,
    pub limits: Limits,
    pub server_time: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionStatusPayload {
    pub protocol_ready: bool,
    pub message: Option<String>,
    #[serde(default)]
    pub shutting_down: bool,
    #[serde(default)]
    pub grace_period_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListDirectoryRequest {
    pub root_id: RootId,
    pub relative_path: RelativePath,
    pub cursor: Option<DirectoryCursor>,
    pub limit: usize,
    pub include_hidden: bool,
    pub sort: SortMode,
    pub filter: FilterMode,
}

impl ListDirectoryRequest {
    pub fn validate(&self, limits: &Limits) -> Result<(), TealDriveError> {
        if self.limit == 0 || self.limit > limits.max_directory_page_size || !self.filter.is_valid()
        {
            return Err(TealDriveError::Validation);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryListResponse {
    pub entries: Vec<DirectoryEntryPayload>,
    pub next_cursor: Option<DirectoryCursor>,
    pub has_more: bool,
    pub total_estimate: Option<usize>,
    pub current_path: RelativePath,
    pub capabilities: CapabilitySet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryEntryPayload {
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
pub struct FileMetadataRequest {
    pub root_id: RootId,
    pub relative_path: RelativePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetadataPayload {
    pub root_id: RootId,
    pub relative_path: RelativePath,
    pub entry: DirectoryEntryPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadTextFileRequest {
    pub root_id: RootId,
    pub relative_path: RelativePath,
    pub max_bytes: Option<usize>,
    pub encoding: Option<String>,
}

impl ReadTextFileRequest {
    pub fn validate(&self, limits: &Limits) -> Result<(), TealDriveError> {
        if self
            .max_bytes
            .is_some_and(|max| max > limits.max_text_edit_size)
        {
            return Err(TealDriveError::Validation);
        }
        if let Some(encoding) = &self.encoding {
            if !encoding.eq_ignore_ascii_case("utf-8") {
                return Err(TealDriveError::Validation);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextFileContentPayload {
    #[serde(default)]
    pub revision: String,
    pub root_id: RootId,
    pub relative_path: RelativePath,
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
pub struct SaveTextFileRequest {
    #[serde(default)]
    pub expected_revision: Option<String>,
    pub root_id: RootId,
    pub relative_path: RelativePath,
    pub content: String,
    pub expected_modified: Option<String>,
}

impl SaveTextFileRequest {
    pub fn validate(&self, limits: &Limits) -> Result<(), TealDriveError> {
        if self.expected_revision.as_ref().is_some_and(|r| {
            r != "missing" && (r.len() != 64 || !r.bytes().all(|c| c.is_ascii_hexdigit()))
        }) || self.content.len() > limits.max_text_edit_size
        {
            return Err(TealDriveError::Validation);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadBeginRequest {
    pub root_id: RootId,
    pub relative_path: RelativePath,
    pub filename: String,
    pub total_size: u64,
    pub chunk_size: usize,
    pub sha256: Option<String>,
}

impl UploadBeginRequest {
    pub fn validate(&self, limits: &Limits) -> Result<(), TealDriveError> {
        if self.chunk_size > limits.max_chunk_size {
            return Err(TealDriveError::Validation);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadBeginResponse {
    pub transfer_id: TransferId,
    pub chunk_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadEndRequest {
    pub transfer_id: TransferId,
    pub total_size: u64,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadEndResponse {
    pub transfer_id: TransferId,
    pub bytes_written: u64,
    pub sha256_verified: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadAbortRequest {
    pub transfer_id: TransferId,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadBeginRequest {
    pub root_id: RootId,
    pub relative_path: RelativePath,
    pub requested_chunk_size: Option<usize>,
}

impl DownloadBeginRequest {
    pub fn validate(&self, limits: &Limits) -> Result<(), TealDriveError> {
        if self
            .requested_chunk_size
            .is_some_and(|chunk_size| chunk_size > limits.max_chunk_size)
        {
            return Err(TealDriveError::Validation);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadBeginResponse {
    pub transfer_id: TransferId,
    pub size: u64,
    pub chunk_size: usize,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadEndPayload {
    pub transfer_id: TransferId,
    pub bytes_sent: u64,
    pub chunk_count: u32,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateFolderRequest {
    pub root_id: RootId,
    pub parent_relative_path: RelativePath,
    pub folder_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenameFileRequest {
    pub root_id: RootId,
    pub relative_path: RelativePath,
    pub new_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveFileRequest {
    pub root_id: RootId,
    pub relative_path: RelativePath,
    pub target_root_id: RootId,
    pub target_relative_path: RelativePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopyFileRequest {
    pub root_id: RootId,
    pub relative_path: RelativePath,
    pub target_root_id: RootId,
    pub target_relative_path: RelativePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveToTrashRequest {
    pub root_id: RootId,
    pub relative_path: RelativePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreFromTrashRequest {
    pub trash_id: String,
    pub target_root_id: Option<RootId>,
    pub target_relative_path: Option<RelativePath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveToTrashPayload {
    pub request_id: RequestId,
    pub operation: String,
    pub trash_id: String,
    pub display_name: String,
    pub deleted_at: u64,
    pub original_relative_path: RelativePath,
    pub original_root_id: RootId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletePermanentlyRequest {
    pub root_id: RootId,
    pub relative_path: RelativePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChmodFileRequest {
    pub root_id: RootId,
    pub relative_path: RelativePath,
    pub mode: UnixMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompressZipRequest {
    pub root_id: RootId,
    pub relative_paths: Vec<RelativePath>,
    pub target_relative_path: RelativePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompressTarGzRequest {
    pub root_id: RootId,
    pub relative_paths: Vec<RelativePath>,
    pub target_relative_path: RelativePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractArchiveRequest {
    pub root_id: RootId,
    pub archive_relative_path: RelativePath,
    pub target_relative_path: RelativePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationStartedPayload {
    pub request_id: RequestId,
    pub operation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationProgressPayload {
    pub request_id: RequestId,
    pub operation: String,
    pub bytes_done: Option<u64>,
    pub bytes_total: Option<u64>,
    pub items_done: Option<u64>,
    pub items_total: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationDonePayload {
    pub request_id: RequestId,
    pub operation: String,
    pub safe_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationFailedPayload {
    pub request_id: RequestId,
    pub operation: String,
    pub error: ErrorPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationErrorPayload {
    pub error: ErrorPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDeniedPayload {
    pub error: ErrorPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionDeniedPayload {
    pub error: ErrorPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchFilesRequest {
    pub root_id: RootId,
    #[serde(default)]
    pub base_path: RelativePath,
    pub query: String,
    #[serde(default)]
    pub extension: Option<String>,
    #[serde(default)]
    pub max_results: Option<usize>,
    #[serde(default)]
    pub include_hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchMatchEntry {
    pub relative_path: RelativePath,
    pub entry: DirectoryEntryPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResultsPayload {
    pub root_id: RootId,
    pub query: String,
    pub matches: Vec<SearchMatchEntry>,
    pub total_matches: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListTrashRequest {
    pub root_id: RootId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrashItemEntry {
    pub trash_id: String,
    pub display_name: String,
    pub original_path: RelativePath,
    pub original_root_id: RootId,
    pub deleted_at: u64,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrashListPayload {
    pub root_id: RootId,
    pub items: Vec<TrashItemEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::{ErrorKind, PolicyReason};
    use crate::limits::{MAX_CHUNK_SIZE, MAX_CONTROL_PAYLOAD, MAX_DIRECTORY_PAGE_SIZE};
    use crate::protocol::codec::{decode_msgpack, encode_msgpack};

    fn server_hello() -> ServerHelloPayload {
        let limits = Limits::default();
        ServerHelloPayload {
            authentication: AuthenticationCapabilities::default(),
            protocol_version: 1,
            server_name: "tealdrive".to_owned(),
            selected_encoding: Some(Encoding::MessagePack),
            supported_encodings: vec![Encoding::MessagePack, Encoding::RawBinary],
            supported_compressions: vec![Compression::None, Compression::Gzip],
            max_frame_size: limits.max_frame_payload,
            max_control_payload_size: limits.max_control_payload,
            max_chunk_size: limits.max_chunk_size,
            feature_flags: FeatureFlags::default(),
        }
    }

    fn client_hello() -> ClientHelloPayload {
        let limits = Limits::default();
        ClientHelloPayload {
            protocol_version: 1,
            supported_encodings: vec![Encoding::MessagePack, Encoding::RawBinary],
            supported_compressions: vec![Compression::None],
            max_frame_size: limits.max_frame_payload,
            max_control_payload_size: limits.max_control_payload,
            max_chunk_size: limits.max_chunk_size,
            feature_flags: FeatureFlags::default(),
        }
    }

    fn directory_entry() -> DirectoryEntryPayload {
        DirectoryEntryPayload {
            name: "file.txt".to_owned(),
            file_type: FileKind::File,
            size: 12,
            modified: Some("2026-05-29T00:00:00Z".to_owned()),
            owner: Some(OwnerName("alice".to_owned())),
            group: Some(GroupName("alice".to_owned())),
            mode: Some(UnixMode(0o644)),
            permissions: Some("rw-r--r--".to_owned()),
            is_hidden: false,
            is_sensitive: false,
            is_protected: false,
            is_symlink: false,
            symlink_target: None,
            is_web_root: false,
            is_world_writable: false,
            capabilities: FileCapabilities::default(),
        }
    }

    #[test]
    fn msgpack_server_hello_roundtrip() {
        let value = server_hello();
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: ServerHelloPayload = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn msgpack_client_hello_roundtrip() {
        let value = client_hello();
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: ClientHelloPayload = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn msgpack_list_directory_request_roundtrip() {
        let value = ListDirectoryRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new("docs"),
            cursor: Some(DirectoryCursor("cursor-1".to_owned())),
            limit: 200,
            include_hidden: false,
            sort: SortMode::NameAsc,
            filter: FilterMode::default(),
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: ListDirectoryRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn msgpack_directory_list_response_roundtrip() {
        let value = DirectoryListResponse {
            entries: vec![directory_entry()],
            next_cursor: None,
            has_more: false,
            total_estimate: Some(1),
            current_path: RelativePath::new("docs"),
            capabilities: CapabilitySet::default(),
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: DirectoryListResponse = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn msgpack_file_metadata_request_roundtrip() {
        let value = FileMetadataRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new("docs/file.txt"),
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: FileMetadataRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn msgpack_file_metadata_payload_roundtrip() {
        let value = FileMetadataPayload {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new("docs/file.txt"),
            entry: directory_entry(),
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: FileMetadataPayload = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn msgpack_download_begin_request_roundtrip() {
        let value = DownloadBeginRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new("file.bin"),
            requested_chunk_size: Some(MAX_CHUNK_SIZE),
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: DownloadBeginRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn msgpack_download_end_payload_roundtrip() {
        let value = DownloadEndPayload {
            transfer_id: TransferId::new(),
            bytes_sent: 1024,
            chunk_count: 4,
            sha256: None,
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: DownloadEndPayload = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn msgpack_read_text_file_request_roundtrip() {
        let value = ReadTextFileRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new("notes.txt"),
            max_bytes: Some(1024),
            encoding: Some("utf-8".to_owned()),
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: ReadTextFileRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn msgpack_text_file_content_payload_roundtrip() {
        let value = TextFileContentPayload {
            revision: String::new(),
            root_id: RootId::new("home"),
            relative_path: RelativePath::new("notes.txt"),
            name: "notes.txt".to_owned(),
            content: "hello".to_owned(),
            encoding: "utf-8".to_owned(),
            size: 5,
            modified: None,
            owner: None,
            group: None,
            mode: Some(UnixMode(0o644)),
            permissions: Some("rw-r--r--".to_owned()),
            is_sensitive: false,
            is_hidden: false,
            is_read_only: false,
            truncated: false,
            line_count: Some(1),
            language_hint: Some("text".to_owned()),
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: TextFileContentPayload = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn msgpack_operation_failed_roundtrip() {
        let request_id = RequestId::new();
        let error = ErrorPayload {
            code: "POLICY_DENIED".to_owned(),
            error_kind: ErrorKind::PolicyDenied,
            safe_message: "Operation denied by policy.".to_owned(),
            request_id,
            operation: Some("LIST_DIRECTORY".to_owned()),
            retryable: false,
            policy_reason: Some(PolicyReason::SensitivePath),
            debug_ref: None,
        };
        let value = OperationFailedPayload {
            request_id,
            operation: "LIST_DIRECTORY".to_owned(),
            error,
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: OperationFailedPayload = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn invalid_msgpack_decode_returns_safe_error() {
        let result: Result<ServerHelloPayload, TealDriveError> = decode_msgpack(&[0xc1]);

        assert!(matches!(result, Err(TealDriveError::Protocol)));
    }

    #[test]
    fn list_directory_over_max_limit_rejected() {
        let request = ListDirectoryRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new("docs"),
            cursor: None,
            limit: MAX_DIRECTORY_PAGE_SIZE + 1,
            include_hidden: false,
            sort: SortMode::NameAsc,
            filter: FilterMode::default(),
        };

        assert!(matches!(
            request.validate(&Limits::default()),
            Err(TealDriveError::Validation)
        ));
    }

    #[test]
    fn client_hello_too_large_chunk_size_rejected() {
        let mut payload = client_hello();
        payload.max_chunk_size = MAX_CHUNK_SIZE + 1;

        assert!(matches!(
            payload.validate(&Limits::default()),
            Err(TealDriveError::Validation)
        ));
    }

    #[test]
    fn download_begin_too_large_chunk_size_rejected() {
        let request = DownloadBeginRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new("file.bin"),
            requested_chunk_size: Some(MAX_CHUNK_SIZE + 1),
        };

        assert!(matches!(
            request.validate(&Limits::default()),
            Err(TealDriveError::Validation)
        ));
    }

    #[test]
    fn read_text_file_too_large_max_bytes_rejected() {
        let request = ReadTextFileRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new("notes.txt"),
            max_bytes: Some(Limits::default().max_text_edit_size + 1),
            encoding: None,
        };

        assert!(matches!(
            request.validate(&Limits::default()),
            Err(TealDriveError::Validation)
        ));
    }

    #[test]
    fn oversized_control_payload_rejected_before_decode() {
        let bytes = vec![0_u8; MAX_CONTROL_PAYLOAD + 1];
        let result: Result<ServerHelloPayload, TealDriveError> = decode_msgpack(&bytes);

        assert!(matches!(result, Err(TealDriveError::PayloadTooLarge)));
    }

    #[test]
    fn msgpack_upload_begin_request_roundtrip() {
        let value = UploadBeginRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new("uploads"),
            filename: "photo.jpg".to_owned(),
            total_size: 1048576,
            chunk_size: 262144,
            sha256: Some("abcdef1234567890".to_owned()),
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: UploadBeginRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn msgpack_upload_begin_response_roundtrip() {
        let value = UploadBeginResponse {
            transfer_id: TransferId::new(),
            chunk_size: 262144,
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: UploadBeginResponse = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn msgpack_upload_end_request_roundtrip() {
        let value = UploadEndRequest {
            transfer_id: TransferId::new(),
            total_size: 1048576,
            sha256: Some("abcdef1234567890".to_owned()),
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: UploadEndRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn msgpack_upload_end_response_roundtrip() {
        let value = UploadEndResponse {
            transfer_id: TransferId::new(),
            bytes_written: 1048576,
            sha256_verified: Some(true),
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: UploadEndResponse = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn msgpack_upload_abort_request_roundtrip() {
        let value = UploadAbortRequest {
            transfer_id: TransferId::new(),
            reason: Some("user cancelled".to_owned()),
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: UploadAbortRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn upload_begin_too_large_chunk_size_rejected() {
        let request = UploadBeginRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new("uploads"),
            filename: "photo.jpg".to_owned(),
            total_size: 1048576,
            chunk_size: MAX_CHUNK_SIZE + 1,
            sha256: None,
        };

        assert!(matches!(
            request.validate(&Limits::default()),
            Err(TealDriveError::Validation)
        ));
    }

    #[test]
    fn msgpack_save_text_file_request_roundtrip() {
        let value = SaveTextFileRequest {
            expected_revision: None,
            root_id: RootId::new("home"),
            relative_path: RelativePath::new("notes.txt"),
            content: "hello world".to_owned(),
            expected_modified: Some("2026-05-29T00:00:00Z".to_owned()),
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: SaveTextFileRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn save_text_file_too_large_content_rejected() {
        let request = SaveTextFileRequest {
            expected_revision: None,
            root_id: RootId::new("home"),
            relative_path: RelativePath::new("notes.txt"),
            content: "a".repeat(Limits::default().max_text_edit_size + 1),
            expected_modified: None,
        };

        assert!(matches!(
            request.validate(&Limits::default()),
            Err(TealDriveError::Validation)
        ));
    }

    #[test]
    fn msgpack_delete_permanently_request_roundtrip() {
        let value = DeletePermanentlyRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new("docs/old.txt"),
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: DeletePermanentlyRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn msgpack_chmod_file_request_roundtrip() {
        let value = ChmodFileRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new("scripts/run.sh"),
            mode: UnixMode(0o755),
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: ChmodFileRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn msgpack_compress_zip_request_roundtrip() {
        let value = CompressZipRequest {
            root_id: RootId::new("home"),
            relative_paths: vec![RelativePath::new("a.txt"), RelativePath::new("b.txt")],
            target_relative_path: RelativePath::new("bundle.zip"),
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: CompressZipRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn msgpack_compress_targz_request_roundtrip() {
        let value = CompressTarGzRequest {
            root_id: RootId::new("home"),
            relative_paths: vec![RelativePath::new("docs")],
            target_relative_path: RelativePath::new("docs.tar.gz"),
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: CompressTarGzRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }

    #[test]
    fn msgpack_extract_archive_request_roundtrip() {
        let value = ExtractArchiveRequest {
            root_id: RootId::new("home"),
            archive_relative_path: RelativePath::new("bundle.zip"),
            target_relative_path: RelativePath::new("extracted"),
        };
        let bytes = encode_msgpack(&value).expect("encoded");
        let decoded: ExtractArchiveRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, value);
    }
}

#[derive(Clone, Serialize, Deserialize, zeroize::ZeroizeOnDrop)]
pub struct AuthLoginRequest {
    #[serde(default)]
    #[zeroize(skip)]
    pub action: Option<String>,
    #[serde(default)]
    #[zeroize(skip)]
    pub username: String,
    pub password: String,
}
