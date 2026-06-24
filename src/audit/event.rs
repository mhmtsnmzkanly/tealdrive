use serde::{Deserialize, Serialize};

use crate::config::{RelativePath, RootId, UserContext};
use crate::protocol::frame::RequestId;

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
        Self {
            timestamp: "1970-01-01T00:00:00Z".to_owned(),
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
        }
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditStatus {
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
    FeatureDisabledAttempt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditReason {
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
}
