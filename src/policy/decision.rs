use serde::{Deserialize, Serialize};

use crate::config::{AllowedRoot, AppConfig, RelativePath, RootId};
use crate::errors::{ErrorPayload, PolicyReason, TealDriveError};
use crate::fs::filename::SafeFileName;
use crate::policy::feature_gate::{FeatureGate, FeatureGateResult};
use crate::policy::roots::require_allowed_root;
use crate::policy::sensitive::{
    hidden_policy_decision, sensitive_edit_decision, sensitive_read_decision,
    SensitivePolicyDecision,
};
use crate::policy::webroot::{
    check_webroot_archive_extract, check_webroot_chmod_executable, check_webroot_upload,
};
use crate::protocol::frame::RequestId;
use crate::protocol::message_type::MessageType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyDecision {
    Allow,
    Deny(PolicyReason),
    WarnAllowed(PolicyReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileOperation {
    ListDirectory,
    FileMetadata,
    ReadTextFile,
    DownloadFile,
    UploadFile,
    CreateFolder,
    RenameFile,
    MoveFile,
    CopyFile,
    MoveToTrash,
    RestoreFromTrash,
    DeletePermanently,
    ChmodFile,
    SaveTextFile,
    CompressZip,
    CompressTarGz,
    ExtractArchive,
    Chown,
}

impl FileOperation {
    pub fn as_message_type(self) -> Option<MessageType> {
        match self {
            Self::ListDirectory => Some(MessageType::ListDirectory),
            Self::FileMetadata => Some(MessageType::FileMetadataRequest),
            Self::ReadTextFile => Some(MessageType::ReadTextFile),
            Self::DownloadFile => Some(MessageType::DownloadBegin),
            Self::UploadFile => Some(MessageType::UploadBegin),
            Self::CreateFolder => Some(MessageType::CreateFolder),
            Self::RenameFile => Some(MessageType::RenameFile),
            Self::MoveFile => Some(MessageType::MoveFile),
            Self::CopyFile => Some(MessageType::CopyFile),
            Self::MoveToTrash => Some(MessageType::MoveToTrash),
            Self::RestoreFromTrash => Some(MessageType::RestoreFromTrash),
            Self::DeletePermanently => Some(MessageType::DeletePermanently),
            Self::ChmodFile => Some(MessageType::ChmodFile),
            Self::SaveTextFile => Some(MessageType::SaveTextFile),
            Self::CompressZip => Some(MessageType::CompressZip),
            Self::CompressTarGz => Some(MessageType::CompressTarGz),
            Self::ExtractArchive => Some(MessageType::ExtractArchive),
            Self::Chown => None,
        }
    }

    pub fn is_write(self) -> bool {
        matches!(
            self,
            Self::UploadFile
                | Self::CreateFolder
                | Self::RenameFile
                | Self::MoveFile
                | Self::CopyFile
                | Self::MoveToTrash
                | Self::RestoreFromTrash
                | Self::DeletePermanently
                | Self::ChmodFile
                | Self::SaveTextFile
                | Self::ExtractArchive
                | Self::Chown
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyAuditMetadata {
    pub action: FileOperation,
    pub root_id: RootId,
    pub relative_path: RelativePath,
    pub target_relative_path: Option<RelativePath>,
    pub reason: PolicyReason,
    pub safe_message: String,
}

pub struct PolicyEngine<'a> {
    config: &'a AppConfig,
}

impl<'a> PolicyEngine<'a> {
    pub fn new(config: &'a AppConfig) -> Self {
        Self { config }
    }

    pub fn check_operation(
        &self,
        operation: FileOperation,
        root_id: &RootId,
        relative_path: &RelativePath,
        target_relative_path: Option<&RelativePath>,
        target_filename: Option<&SafeFileName>,
        include_hidden: bool,
    ) -> Result<PolicyDecision, TealDriveError> {
        if operation == FileOperation::Chown && !self.config.features.chown {
            return Ok(PolicyDecision::Deny(PolicyReason::FeatureDisabled));
        }
        if let Some(message_type) = operation.as_message_type() {
            if matches!(
                FeatureGate::check(message_type, &self.config.features),
                FeatureGateResult::Disabled { .. }
            ) {
                return Ok(PolicyDecision::Deny(PolicyReason::FeatureDisabled));
            }
        }

        let root = require_allowed_root(self.config.allowed_roots(), root_id)?;
        if let Some(decision) = self.check_root_operation(operation, root) {
            return Ok(decision);
        }

        if hidden_policy_decision(include_hidden, relative_path) == SensitivePolicyDecision::Deny {
            return Ok(PolicyDecision::Deny(PolicyReason::HiddenFile));
        }

        match operation {
            FileOperation::ReadTextFile => {
                match sensitive_read_decision(relative_path, &self.config.sensitive_policy, false) {
                    SensitivePolicyDecision::Deny => {
                        return Ok(PolicyDecision::Deny(PolicyReason::SensitivePath))
                    }
                    SensitivePolicyDecision::Warn => {
                        return Ok(PolicyDecision::WarnAllowed(PolicyReason::SensitivePath))
                    }
                    SensitivePolicyDecision::Allow => {}
                }
            }
            FileOperation::DownloadFile
                if sensitive_read_decision(relative_path, &self.config.sensitive_policy, true)
                    == SensitivePolicyDecision::Warn =>
            {
                return Ok(PolicyDecision::WarnAllowed(PolicyReason::SensitivePath));
            }
            FileOperation::SaveTextFile
                if sensitive_edit_decision(relative_path, &self.config.sensitive_policy)
                    == SensitivePolicyDecision::Deny =>
            {
                return Ok(PolicyDecision::Deny(PolicyReason::SensitivePath));
            }
            FileOperation::UploadFile => {
                if let Some(filename) = target_filename {
                    return Ok(check_webroot_upload(
                        root.is_web_root,
                        filename,
                        &self.config.webroot_policy,
                    ));
                }
            }
            FileOperation::ChmodFile => {
                return Ok(check_webroot_chmod_executable(root.is_web_root, 0o111));
            }
            FileOperation::ExtractArchive => {
                return Ok(check_webroot_archive_extract(root.is_web_root));
            }
            _ => {}
        }

        if let Some(target) = target_relative_path {
            if hidden_policy_decision(include_hidden, target) == SensitivePolicyDecision::Deny {
                return Ok(PolicyDecision::Deny(PolicyReason::HiddenFile));
            }
        }

        Ok(PolicyDecision::Allow)
    }

    fn check_root_operation(
        &self,
        operation: FileOperation,
        root: &AllowedRoot,
    ) -> Option<PolicyDecision> {
        if root.read_only && operation.is_write() {
            Some(PolicyDecision::Deny(PolicyReason::ReadOnlyRoot))
        } else {
            None
        }
    }
}

pub fn policy_denial_audit_metadata(
    action: FileOperation,
    root_id: RootId,
    relative_path: RelativePath,
    target_relative_path: Option<RelativePath>,
    reason: PolicyReason,
) -> PolicyAuditMetadata {
    PolicyAuditMetadata {
        action,
        root_id,
        relative_path,
        target_relative_path,
        reason,
        safe_message: "Operation denied by TealDrive policy.".to_owned(),
    }
}

pub fn policy_denial_error_payload(
    request_id: RequestId,
    operation: FileOperation,
    reason: PolicyReason,
) -> ErrorPayload {
    ErrorPayload::policy_denied(
        request_id,
        format!("{operation:?}"),
        reason,
        "Operation denied by TealDrive policy.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn config(root: AllowedRoot) -> AppConfig {
        AppConfig {
            roots: vec![root],
            ..AppConfig::default()
        }
    }

    fn root(read_only: bool, is_web_root: bool) -> AllowedRoot {
        AllowedRoot {
            root_id: RootId::new("root"),
            base_path: PathBuf::from("/tmp"),
            read_only,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root,
        }
    }

    #[test]
    fn read_only_allows_list() {
        let config = config(root(true, false));
        let engine = PolicyEngine::new(&config);

        assert_eq!(
            engine.check_operation(
                FileOperation::ListDirectory,
                &RootId::new("root"),
                &RelativePath::new(""),
                None,
                None,
                true
            ),
            Ok(PolicyDecision::Allow)
        );
    }

    #[test]
    fn read_only_allows_download_if_not_sensitive_denied() {
        let config = config(root(true, false));
        let engine = PolicyEngine::new(&config);

        assert_eq!(
            engine.check_operation(
                FileOperation::DownloadFile,
                &RootId::new("root"),
                &RelativePath::new("notes.txt"),
                None,
                None,
                true
            ),
            Ok(PolicyDecision::Allow)
        );
    }

    #[test]
    fn read_only_denies_write_operations() {
        let config = config(root(true, false));
        let engine = PolicyEngine::new(&config);
        for operation in [
            FileOperation::UploadFile,
            FileOperation::RenameFile,
            FileOperation::MoveToTrash,
        ] {
            assert_eq!(
                engine.check_operation(
                    operation,
                    &RootId::new("root"),
                    &RelativePath::new("file.txt"),
                    None,
                    Some(&SafeFileName("file.txt".to_owned())),
                    true
                ),
                Ok(PolicyDecision::Deny(PolicyReason::ReadOnlyRoot))
            );
        }
    }

    #[test]
    fn feature_gate_denies_disabled_operations() {
        let config = config(root(false, false));
        let engine = PolicyEngine::new(&config);

        for operation in [
            FileOperation::SaveTextFile,
            FileOperation::ChmodFile,
            FileOperation::ExtractArchive,
            FileOperation::DeletePermanently,
        ] {
            assert_eq!(
                engine.check_operation(
                    operation,
                    &RootId::new("root"),
                    &RelativePath::new("file.txt"),
                    None,
                    None,
                    true
                ),
                Ok(PolicyDecision::Deny(PolicyReason::FeatureDisabled))
            );
        }
    }

    #[test]
    fn webroot_script_upload_denied_by_policy_engine() {
        let config = config(root(false, true));
        let engine = PolicyEngine::new(&config);

        assert_eq!(
            engine.check_operation(
                FileOperation::UploadFile,
                &RootId::new("root"),
                &RelativePath::new(""),
                None,
                Some(&SafeFileName::parse("index.php").unwrap()),
                true
            ),
            Ok(PolicyDecision::Deny(PolicyReason::WebRootExecutableUpload))
        );
    }

    #[test]
    fn policy_denial_converts_to_safe_error_payload() {
        let payload = policy_denial_error_payload(
            RequestId::new(),
            FileOperation::ReadTextFile,
            PolicyReason::SensitivePath,
        );

        assert_eq!(payload.policy_reason, Some(PolicyReason::SensitivePath));
        assert!(!payload.safe_message.contains("/tmp/secret/.env"));
    }
}
