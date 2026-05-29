use serde::{Deserialize, Serialize};

use crate::config::{RelativePath, RootId};
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
    ListDirectory,
    FilePreview,
    Download,
    Upload,
    CreateFolder,
    Rename,
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
    InvalidCredentials,
    AccountLocked,
    AccountExpired,
    DisabledShell,
    RootLoginDisabled,
    SystemAccountDisabled,
    SessionExpired,
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
}
