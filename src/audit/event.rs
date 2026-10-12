use serde::{Deserialize, Serialize};

use crate::config::{RelativePath, RootId, UserContext};
use crate::protocol::frame::RequestId;

fn utc_timestamp() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs()) as libc::time_t;
    let mut date = std::mem::MaybeUninit::<libc::tm>::uninit();
    let mut buffer = [0u8; 32];
    unsafe {
        if libc::gmtime_r(&seconds, date.as_mut_ptr()).is_null() {
            return seconds.to_string();
        }
        let length = libc::strftime(
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            c"%Y-%m-%dT%H:%M:%SZ".as_ptr(),
            date.as_ptr(),
        );
        String::from_utf8_lossy(&buffer[..length]).into_owned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: String,
    pub request_id: RequestId,
    pub session_id_hash: String,
    pub username: String,
    pub uid: u32,
    pub gid: u32,
    pub action: AuditAction,
    pub root_id: Option<RootId>,
    pub relative_path: Option<RelativePath>,
    pub target_relative_path: Option<RelativePath>,
    pub status: AuditStatus,
    pub reason: Option<AuditReason>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub bytes_in: u64,
    pub bytes_out: u64,
    #[serde(default)]
    pub peer_ip: Option<String>,
    #[serde(default)]
    pub phase: Option<String>,
    #[serde(default)]
    pub operation: Option<String>,
    #[serde(default)]
    pub error_kind: Option<crate::errors::ErrorKind>,
    #[serde(default)]
    pub http_status: Option<u16>,
    #[serde(default)]
    pub http_method: Option<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub target_root_id: Option<RootId>,
    #[serde(default)]
    pub source_relative_paths: Vec<RelativePath>,
    #[serde(default)]
    pub source_count: Option<usize>,
    #[serde(default)]
    pub transfer_id: Option<crate::protocol::frame::TransferId>,
    #[serde(default)]
    pub mode: Option<u32>,
    #[serde(default)]
    pub trash_id: Option<String>,
    #[serde(default)]
    pub helper_bytes_read: u64,
    #[serde(default)]
    pub helper_bytes_written: u64,
}

impl AuditEvent {
    pub fn new(
        request_id: RequestId,
        username: impl Into<String>,
        uid: u32,
        gid: u32,
        action: AuditAction,
        status: AuditStatus,
    ) -> Self {
        let mut event = Self {
            timestamp: utc_timestamp(),
            request_id,
            session_id_hash: String::new(),
            username: username.into(),
            uid,
            gid,
            action,
            root_id: None,
            relative_path: None,
            target_relative_path: None,
            status,
            reason: None,
            ip: None,
            user_agent: None,
            bytes_in: 0,
            bytes_out: 0,
            peer_ip: None,
            phase: None,
            operation: None,
            error_kind: None,
            http_status: None,
            http_method: None,
            endpoint: None,
            target_root_id: None,
            source_relative_paths: Vec::new(),
            source_count: None,
            transfer_id: None,
            mode: None,
            trash_id: None,
            helper_bytes_read: 0,
            helper_bytes_written: 0,
        };
        crate::audit::context::current().enrich(&mut event);
        event
    }

    pub fn login_success(
        request_id: RequestId,
        username: impl Into<String>,
        uid: u32,
        gid: u32,
    ) -> Self {
        Self::new(
            request_id,
            username,
            uid,
            gid,
            AuditAction::Login,
            AuditStatus::Success,
        )
    }

    pub fn login_failure(
        request_id: RequestId,
        username: impl Into<String>,
        reason: AuditReason,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            username,
            0,
            0,
            AuditAction::Login,
            AuditStatus::Failed,
        );
        event.reason = Some(reason);
        event
    }

    pub fn logout(request_id: RequestId, username: impl Into<String>, uid: u32, gid: u32) -> Self {
        Self::new(
            request_id,
            username,
            uid,
            gid,
            AuditAction::Logout,
            AuditStatus::Success,
        )
    }

    pub fn logout_all(
        request_id: RequestId,
        username: impl Into<String>,
        uid: u32,
        gid: u32,
    ) -> Self {
        Self::new(
            request_id,
            username,
            uid,
            gid,
            AuditAction::LogoutAll,
            AuditStatus::Success,
        )
    }

    pub fn session_expired(
        request_id: RequestId,
        username: impl Into<String>,
        uid: u32,
        gid: u32,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            username,
            uid,
            gid,
            AuditAction::SessionExpired,
            AuditStatus::Failed,
        );
        event.reason = Some(AuditReason::SessionExpired);
        event
    }

    pub fn websocket_connect(
        request_id: RequestId,
        username: impl Into<String>,
        uid: u32,
        gid: u32,
    ) -> Self {
        Self::new(
            request_id,
            username,
            uid,
            gid,
            AuditAction::WebSocketConnect,
            AuditStatus::Success,
        )
    }

    pub fn websocket_disconnect(
        request_id: RequestId,
        username: impl Into<String>,
        uid: u32,
        gid: u32,
    ) -> Self {
        Self::new(
            request_id,
            username,
            uid,
            gid,
            AuditAction::WebSocketDisconnect,
            AuditStatus::Success,
        )
    }

    pub fn websocket_handshake_success(
        request_id: RequestId,
        username: impl Into<String>,
        uid: u32,
        gid: u32,
    ) -> Self {
        Self::new(
            request_id,
            username,
            uid,
            gid,
            AuditAction::WebSocketHandshake,
            AuditStatus::Success,
        )
    }

    pub fn websocket_handshake_failure(
        request_id: RequestId,
        username: impl Into<String>,
        reason: AuditReason,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            username,
            0,
            0,
            AuditAction::WebSocketHandshake,
            AuditStatus::Failed,
        );
        event.reason = Some(reason);
        event
    }

    pub fn websocket_rejection(
        request_id: RequestId,
        username: impl Into<String>,
        uid: u32,
        gid: u32,
        reason: AuditReason,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            username,
            uid,
            gid,
            AuditAction::WebSocketMessageRejected,
            AuditStatus::Denied,
        );
        event.reason = Some(reason);
        event
    }

    pub fn file_metadata(
        request_id: RequestId,
        user_context: &UserContext,
        root_id: RootId,
        relative_path: RelativePath,
        status: AuditStatus,
        reason: Option<AuditReason>,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            user_context.username.clone(),
            user_context.uid,
            user_context.gid,
            AuditAction::FileMetadata,
            status,
        );
        event.root_id = Some(root_id);
        event.relative_path = Some(relative_path);
        event.reason = reason;
        event
    }

    pub fn download(
        request_id: RequestId,
        user_context: &UserContext,
        root_id: RootId,
        relative_path: RelativePath,
        status: AuditStatus,
        reason: Option<AuditReason>,
        bytes_out: u64,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            user_context.username.clone(),
            user_context.uid,
            user_context.gid,
            AuditAction::Download,
            status,
        );
        event.root_id = Some(root_id);
        event.relative_path = Some(relative_path);
        event.reason = reason;
        event.bytes_out = bytes_out;
        event
    }

    pub fn text_preview(
        request_id: RequestId,
        user_context: &UserContext,
        root_id: RootId,
        relative_path: RelativePath,
        status: AuditStatus,
        reason: Option<AuditReason>,
        bytes_in: u64,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            user_context.username.clone(),
            user_context.uid,
            user_context.gid,
            AuditAction::FilePreview,
            status,
        );
        event.root_id = Some(root_id);
        event.relative_path = Some(relative_path);
        event.reason = reason;
        event.bytes_in = bytes_in;
        event
    }

    pub fn create_folder(
        request_id: RequestId,
        user_context: &UserContext,
        root_id: RootId,
        parent_relative_path: RelativePath,
        folder_name: impl Into<String>,
        status: AuditStatus,
        reason: Option<AuditReason>,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            user_context.username.clone(),
            user_context.uid,
            user_context.gid,
            AuditAction::CreateFolder,
            status,
        );
        event.root_id = Some(root_id);
        event.relative_path = Some(parent_relative_path);
        event.target_relative_path = Some(RelativePath::new(folder_name.into()));
        event.reason = reason;
        event
    }

    pub fn rename(
        request_id: RequestId,
        user_context: &UserContext,
        root_id: RootId,
        relative_path: RelativePath,
        new_name: impl Into<String>,
        status: AuditStatus,
        reason: Option<AuditReason>,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            user_context.username.clone(),
            user_context.uid,
            user_context.gid,
            AuditAction::Rename,
            status,
        );
        event.root_id = Some(root_id);
        event.relative_path = Some(relative_path);
        event.target_relative_path = Some(RelativePath::new(new_name.into()));
        event.reason = reason;
        event
    }

    pub fn move_file(
        request_id: RequestId,
        user_context: &UserContext,
        root_id: RootId,
        relative_path: RelativePath,
        target_relative_path: RelativePath,
        status: AuditStatus,
        reason: Option<AuditReason>,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            user_context.username.clone(),
            user_context.uid,
            user_context.gid,
            AuditAction::MoveFile,
            status,
        );
        event.root_id = Some(root_id);
        event.relative_path = Some(relative_path);
        event.target_relative_path = Some(target_relative_path);
        event.reason = reason;
        event
    }

    pub fn copy_file(
        request_id: RequestId,
        user_context: &UserContext,
        root_id: RootId,
        relative_path: RelativePath,
        target_relative_path: RelativePath,
        status: AuditStatus,
        reason: Option<AuditReason>,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            user_context.username.clone(),
            user_context.uid,
            user_context.gid,
            AuditAction::CopyFile,
            status,
        );
        event.root_id = Some(root_id);
        event.relative_path = Some(relative_path);
        event.target_relative_path = Some(target_relative_path);
        event.reason = reason;
        event
    }

    pub fn move_to_trash(
        request_id: RequestId,
        user_context: &UserContext,
        root_id: RootId,
        relative_path: RelativePath,
        trash_id: impl Into<String>,
        status: AuditStatus,
        reason: Option<AuditReason>,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            user_context.username.clone(),
            user_context.uid,
            user_context.gid,
            AuditAction::MoveToTrash,
            status,
        );
        event.root_id = Some(root_id);
        event.relative_path = Some(relative_path);
        event.target_relative_path = Some(RelativePath::new(trash_id.into()));
        event.reason = reason;
        event
    }

    pub fn restore_from_trash(
        request_id: RequestId,
        user_context: &UserContext,
        root_id: RootId,
        trash_id: impl Into<String>,
        status: AuditStatus,
        reason: Option<AuditReason>,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            user_context.username.clone(),
            user_context.uid,
            user_context.gid,
            AuditAction::RestoreFromTrash,
            status,
        );
        event.root_id = Some(root_id);
        event.relative_path = Some(RelativePath::new(trash_id.into()));
        event.reason = reason;
        event
    }

    pub fn upload(
        request_id: RequestId,
        user_context: &UserContext,
        root_id: RootId,
        relative_path: RelativePath,
        status: AuditStatus,
        reason: Option<AuditReason>,
        bytes_in: u64,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            user_context.username.clone(),
            user_context.uid,
            user_context.gid,
            AuditAction::Upload,
            status,
        );
        event.root_id = Some(root_id);
        event.relative_path = Some(relative_path);
        event.reason = reason;
        event.bytes_in = bytes_in;
        event
    }

    pub fn save_text_file(
        request_id: RequestId,
        user_context: &UserContext,
        root_id: RootId,
        relative_path: RelativePath,
        status: AuditStatus,
        reason: Option<AuditReason>,
        bytes_in: u64,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            user_context.username.clone(),
            user_context.uid,
            user_context.gid,
            AuditAction::SaveTextFile,
            status,
        );
        event.root_id = Some(root_id);
        event.relative_path = Some(relative_path);
        event.reason = reason;
        event.bytes_in = bytes_in;
        event
    }

    pub fn delete_permanently(
        request_id: RequestId,
        user_context: &UserContext,
        root_id: RootId,
        relative_path: RelativePath,
        status: AuditStatus,
        reason: Option<AuditReason>,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            user_context.username.clone(),
            user_context.uid,
            user_context.gid,
            AuditAction::DeletePermanently,
            status,
        );
        event.root_id = Some(root_id);
        event.relative_path = Some(relative_path);
        event.reason = reason;
        event
    }

    pub fn chmod(
        request_id: RequestId,
        user_context: &UserContext,
        root_id: RootId,
        relative_path: RelativePath,
        status: AuditStatus,
        reason: Option<AuditReason>,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            user_context.username.clone(),
            user_context.uid,
            user_context.gid,
            AuditAction::Chmod,
            status,
        );
        event.root_id = Some(root_id);
        event.relative_path = Some(relative_path);
        event.reason = reason;
        event
    }

    pub fn compress_zip(
        request_id: RequestId,
        user_context: &UserContext,
        root_id: RootId,
        target_relative_path: RelativePath,
        status: AuditStatus,
        reason: Option<AuditReason>,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            user_context.username.clone(),
            user_context.uid,
            user_context.gid,
            AuditAction::CompressZip,
            status,
        );
        event.root_id = Some(root_id);
        event.target_relative_path = Some(target_relative_path);
        event.reason = reason;
        event
    }

    pub fn compress_targz(
        request_id: RequestId,
        user_context: &UserContext,
        root_id: RootId,
        target_relative_path: RelativePath,
        status: AuditStatus,
        reason: Option<AuditReason>,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            user_context.username.clone(),
            user_context.uid,
            user_context.gid,
            AuditAction::CompressTarGz,
            status,
        );
        event.root_id = Some(root_id);
        event.target_relative_path = Some(target_relative_path);
        event.reason = reason;
        event
    }

    pub fn extract_archive(
        request_id: RequestId,
        user_context: &UserContext,
        root_id: RootId,
        archive_relative_path: RelativePath,
        target_relative_path: RelativePath,
        status: AuditStatus,
        reason: Option<AuditReason>,
    ) -> Self {
        let mut event = Self::new(
            request_id,
            user_context.username.clone(),
            user_context.uid,
            user_context.gid,
            AuditAction::ExtractArchive,
            status,
        );
        event.root_id = Some(root_id);
        event.relative_path = Some(archive_relative_path);
        event.target_relative_path = Some(target_relative_path);
        event.reason = reason;
        event
    }

    pub fn config_reload(
        request_id: RequestId,
        username: impl Into<String>,
        uid: u32,
        gid: u32,
        status: AuditStatus,
    ) -> Self {
        Self::new(
            request_id,
            username,
            uid,
            gid,
            AuditAction::ConfigReload,
            status,
        )
    }

    pub fn config_patch(
        request_id: RequestId,
        username: impl Into<String>,
        uid: u32,
        gid: u32,
        status: AuditStatus,
    ) -> Self {
        Self::new(
            request_id,
            username,
            uid,
            gid,
            AuditAction::ConfigPatch,
            status,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditStatus {
    Started,
    Cancelled,
    Success,
    Denied,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditAction {
    Login,
    Logout,
    LogoutAll,
    SessionExpired,
    WebSocketConnect,
    WebSocketDisconnect,
    WebSocketHandshake,
    WebSocketMessageRejected,
    ListDirectory,
    FileMetadata,
    FilePreview,
    Download,
    Upload,
    CreateFolder,
    Rename,
    MoveFile,
    CopyFile,
    MoveToTrash,
    RestoreFromTrash,
    SaveTextFile,
    DeletePermanently,
    Chmod,
    CompressZip,
    CompressTarGz,
    ExtractArchive,
    FeatureDisabledAttempt,
    ConfigReload,
    ConfigPatch,
    ServerShutdown,
    SessionRevoked,
    PasswordChanged,
    HttpRequest,
    SearchFiles,
    ListTrash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditReason {
    AccountNotAllowed,
    ProtocolError,
    Cancelled,
    Shutdown,
    ConnectionClosed,
    StreamFailed,
    ValidationError,
    InternalError,
    CsrfRejected,
    AdminRequired,
    Unauthenticated,
    ClientAborted,
    FeatureDisabled,
    PermissionDenied,
    PolicyDenied,
    PathTraversalAttempt,
    SymlinkEscapeAttempt,
    NotFound,
    InvalidCredentials,
    AccountLocked,
    AccountExpired,
    DisabledShell,
    RootLoginDisabled,
    SystemAccountDisabled,
    SessionExpired,
    HandshakeFailed,
    OperationBeforeReady,
    InvalidMessageDirection,
    RateLimited,
    InvalidConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_event_creation_works() {
        let event = AuditEvent::new(
            RequestId::new(),
            "alice",
            1000,
            1000,
            AuditAction::ListDirectory,
            AuditStatus::Success,
        );

        assert_eq!(event.username, "alice");
        assert_eq!(event.uid, 1000);
        assert_eq!(event.action, AuditAction::ListDirectory);
        assert_eq!(event.status, AuditStatus::Success);
    }

    #[test]
    fn websocket_handshake_success_event_created() {
        let event = AuditEvent::websocket_handshake_success(RequestId::new(), "alice", 1000, 1000);

        assert_eq!(event.action, AuditAction::WebSocketHandshake);
        assert_eq!(event.status, AuditStatus::Success);
        assert_eq!(event.username, "alice");
    }

    #[test]
    fn websocket_handshake_failure_event_created() {
        let event = AuditEvent::websocket_handshake_failure(
            RequestId::new(),
            "alice",
            AuditReason::HandshakeFailed,
        );

        assert_eq!(event.action, AuditAction::WebSocketHandshake);
        assert_eq!(event.status, AuditStatus::Failed);
        assert_eq!(event.reason, Some(AuditReason::HandshakeFailed));
    }

    #[test]
    fn file_metadata_audit_event_created() {
        let event = AuditEvent::file_metadata(
            RequestId::new(),
            &UserContext {
                username: "alice".to_owned(),
                uid: 1000,
                gid: 1000,
            },
            RootId::new("home"),
            RelativePath::new("file.txt"),
            AuditStatus::Success,
            None,
        );

        assert_eq!(event.action, AuditAction::FileMetadata);
        assert_eq!(event.status, AuditStatus::Success);
        assert_eq!(event.relative_path, Some(RelativePath::new("file.txt")));
    }

    #[test]
    fn download_audit_event_records_bytes_out() {
        let event = AuditEvent::download(
            RequestId::new(),
            &UserContext {
                username: "alice".to_owned(),
                uid: 1000,
                gid: 1000,
            },
            RootId::new("home"),
            RelativePath::new("file.bin"),
            AuditStatus::Success,
            None,
            1024,
        );

        assert_eq!(event.action, AuditAction::Download);
        assert_eq!(event.bytes_out, 1024);
    }

    #[test]
    fn text_preview_audit_event_records_bytes_in() {
        let event = AuditEvent::text_preview(
            RequestId::new(),
            &UserContext {
                username: "alice".to_owned(),
                uid: 1000,
                gid: 1000,
            },
            RootId::new("home"),
            RelativePath::new("notes.txt"),
            AuditStatus::Success,
            None,
            128,
        );

        assert_eq!(event.action, AuditAction::FilePreview);
        assert_eq!(event.bytes_in, 128);
    }

    #[test]
    fn create_folder_audit_event_records_target_name() {
        let event = AuditEvent::create_folder(
            RequestId::new(),
            &UserContext {
                username: "alice".to_owned(),
                uid: 1000,
                gid: 1000,
            },
            RootId::new("home"),
            RelativePath::new("docs"),
            "new-folder",
            AuditStatus::Success,
            None,
        );

        assert_eq!(event.action, AuditAction::CreateFolder);
        assert_eq!(
            event.target_relative_path,
            Some(RelativePath::new("new-folder"))
        );
    }

    #[test]
    fn rename_audit_event_records_new_name() {
        let event = AuditEvent::rename(
            RequestId::new(),
            &UserContext {
                username: "alice".to_owned(),
                uid: 1000,
                gid: 1000,
            },
            RootId::new("home"),
            RelativePath::new("old.txt"),
            "new.txt",
            AuditStatus::Success,
            None,
        );

        assert_eq!(event.action, AuditAction::Rename);
        assert_eq!(event.relative_path, Some(RelativePath::new("old.txt")));
        assert_eq!(
            event.target_relative_path,
            Some(RelativePath::new("new.txt"))
        );
    }

    #[test]
    fn move_file_audit_event_records_source_and_target() {
        let user = UserContext {
            username: "alice".to_owned(),
            uid: 1000,
            gid: 1000,
        };
        let event = AuditEvent::move_file(
            RequestId::new(),
            &user,
            RootId::new("home"),
            RelativePath::new("old.txt"),
            RelativePath::new("archive/old.txt"),
            AuditStatus::Success,
            None,
        );

        assert_eq!(event.action, AuditAction::MoveFile);
        assert_eq!(event.relative_path, Some(RelativePath::new("old.txt")));
        assert_eq!(
            event.target_relative_path,
            Some(RelativePath::new("archive/old.txt"))
        );
        assert_eq!(event.status, AuditStatus::Success);
    }

    #[test]
    fn copy_file_audit_event_records_source_and_target() {
        let user = UserContext {
            username: "alice".to_owned(),
            uid: 1000,
            gid: 1000,
        };
        let event = AuditEvent::copy_file(
            RequestId::new(),
            &user,
            RootId::new("home"),
            RelativePath::new("orig.txt"),
            RelativePath::new("copies/orig.txt"),
            AuditStatus::Denied,
            Some(AuditReason::PolicyDenied),
        );

        assert_eq!(event.action, AuditAction::CopyFile);
        assert_eq!(event.relative_path, Some(RelativePath::new("orig.txt")));
        assert_eq!(
            event.target_relative_path,
            Some(RelativePath::new("copies/orig.txt"))
        );
        assert_eq!(event.status, AuditStatus::Denied);
        assert_eq!(event.reason, Some(AuditReason::PolicyDenied));
    }

    #[test]
    fn upload_audit_event_records_fields() {
        let user = UserContext {
            username: "alice".to_owned(),
            uid: 1000,
            gid: 1000,
        };
        let event = AuditEvent::upload(
            RequestId::new(),
            &user,
            RootId::new("home"),
            RelativePath::new("uploads/file.png"),
            AuditStatus::Success,
            None,
            4096,
        );

        assert_eq!(event.action, AuditAction::Upload);
        assert_eq!(
            event.relative_path,
            Some(RelativePath::new("uploads/file.png"))
        );
        assert_eq!(event.bytes_in, 4096);
        assert_eq!(event.status, AuditStatus::Success);
        assert_eq!(event.reason, None);
    }

    #[test]
    fn save_text_file_audit_event_records_fields() {
        let user = UserContext {
            username: "alice".to_owned(),
            uid: 1000,
            gid: 1000,
        };
        let event = AuditEvent::save_text_file(
            RequestId::new(),
            &user,
            RootId::new("home"),
            RelativePath::new("docs/notes.txt"),
            AuditStatus::Success,
            None,
            128,
        );

        assert_eq!(event.action, AuditAction::SaveTextFile);
        assert_eq!(
            event.relative_path,
            Some(RelativePath::new("docs/notes.txt"))
        );
        assert_eq!(event.bytes_in, 128);
        assert_eq!(event.status, AuditStatus::Success);
        assert_eq!(event.reason, None);
    }

    #[test]
    fn delete_permanently_audit_event_records_fields() {
        let user = UserContext {
            username: "alice".to_owned(),
            uid: 1000,
            gid: 1000,
        };
        let event = AuditEvent::delete_permanently(
            RequestId::new(),
            &user,
            RootId::new("home"),
            RelativePath::new("docs/old.txt"),
            AuditStatus::Success,
            None,
        );

        assert_eq!(event.action, AuditAction::DeletePermanently);
        assert_eq!(event.relative_path, Some(RelativePath::new("docs/old.txt")));
        assert_eq!(event.status, AuditStatus::Success);
        assert_eq!(event.reason, None);
    }

    #[test]
    fn chmod_audit_event_records_fields() {
        let user = UserContext {
            username: "alice".to_owned(),
            uid: 1000,
            gid: 1000,
        };
        let event = AuditEvent::chmod(
            RequestId::new(),
            &user,
            RootId::new("home"),
            RelativePath::new("docs/script.sh"),
            AuditStatus::Success,
            None,
        );

        assert_eq!(event.action, AuditAction::Chmod);
        assert_eq!(
            event.relative_path,
            Some(RelativePath::new("docs/script.sh"))
        );
        assert_eq!(event.status, AuditStatus::Success);
        assert_eq!(event.reason, None);
    }

    #[test]
    fn archive_audit_events_record_fields() {
        let user = UserContext {
            username: "alice".to_owned(),
            uid: 1000,
            gid: 1000,
        };
        let zip_event = AuditEvent::compress_zip(
            RequestId::new(),
            &user,
            RootId::new("home"),
            RelativePath::new("out.zip"),
            AuditStatus::Success,
            None,
        );
        assert_eq!(zip_event.action, AuditAction::CompressZip);
        assert_eq!(
            zip_event.target_relative_path,
            Some(RelativePath::new("out.zip"))
        );

        let targz_event = AuditEvent::compress_targz(
            RequestId::new(),
            &user,
            RootId::new("home"),
            RelativePath::new("out.tar.gz"),
            AuditStatus::Success,
            None,
        );
        assert_eq!(targz_event.action, AuditAction::CompressTarGz);
        assert_eq!(
            targz_event.target_relative_path,
            Some(RelativePath::new("out.tar.gz"))
        );

        let extract_event = AuditEvent::extract_archive(
            RequestId::new(),
            &user,
            RootId::new("home"),
            RelativePath::new("in.zip"),
            RelativePath::new("out_dir"),
            AuditStatus::Success,
            None,
        );
        assert_eq!(extract_event.action, AuditAction::ExtractArchive);
        assert_eq!(
            extract_event.relative_path,
            Some(RelativePath::new("in.zip"))
        );
        assert_eq!(
            extract_event.target_relative_path,
            Some(RelativePath::new("out_dir"))
        );
    }
}

#[cfg(test)]
mod timestamp_tests {
    #[test]
    fn events_use_current_utc_timestamps() {
        let timestamp = super::utc_timestamp();
        assert_eq!(timestamp.len(), 20);
        assert!(timestamp.ends_with('Z'));
        assert_ne!(timestamp, "1970-01-01T00:00:00Z");
    }
}
