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
}
