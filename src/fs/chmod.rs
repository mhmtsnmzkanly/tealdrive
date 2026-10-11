use crate::config::{AppConfig, UserContext};
use crate::errors::{ErrorKind, TealDriveError};
use crate::fs::path::validate_relative_path;
use crate::helper::client::HelperProcessClient;
use crate::helper::types::{
    HelperCommand, HelperLimits, HelperPolicyContext, HelperRequest, HelperResponse,
};
use crate::policy::decision::{FileOperation, PolicyAuditMetadata, PolicyDecision, PolicyEngine};
use crate::policy::roots::require_allowed_root;
use crate::policy::webroot::check_webroot_chmod_executable;
use crate::protocol::frame::RequestId;
use crate::protocol::payload::{ChmodFileRequest, OperationDonePayload};

pub trait ChmodFileHelper: Send + Sync {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError>;
}

impl ChmodFileHelper for HelperProcessClient {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        self.execute(request)
    }
}

pub fn chmod_file_request<H: ChmodFileHelper>(
    user_context: &UserContext,
    request: ChmodFileRequest,
    config: &AppConfig,
    policy_engine: &PolicyEngine<'_>,
    helper_client: &H,
) -> Result<OperationDonePayload, TealDriveError> {
    let relative_path = validate_relative_path(&request.relative_path)?;
    if relative_path.0.is_empty() {
        return Err(TealDriveError::InvalidTarget);
    }
    let root = require_allowed_root(config.allowed_roots(), &request.root_id)?;
    if root.read_only {
        return Err(TealDriveError::ReadOnlyRoot);
    }

    match policy_engine.check_operation(
        FileOperation::ChmodFile,
        &request.root_id,
        &relative_path,
        None,
        None,
        root.hidden_files_allowed,
    )? {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(crate::errors::PolicyReason::FeatureDisabled) => {
            return Err(TealDriveError::FeatureDisabled);
        }
        PolicyDecision::Deny(crate::errors::PolicyReason::ReadOnlyRoot) => {
            return Err(TealDriveError::ReadOnlyRoot);
        }
        PolicyDecision::Deny(crate::errors::PolicyReason::SensitivePath) => {
            return Err(TealDriveError::PolicyDenied);
        }
        PolicyDecision::Deny(reason) => {
            let _audit = PolicyAuditMetadata {
                action: FileOperation::ChmodFile,
                root_id: request.root_id.clone(),
                relative_path: relative_path.clone(),
                target_relative_path: None,
                reason,
                safe_message: "Chmod denied by TealDrive policy.".to_owned(),
            };
            return Err(TealDriveError::PolicyDenied);
        }
    }

    match check_webroot_chmod_executable(root.is_web_root, request.mode.0) {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(_) => {
            return Err(TealDriveError::PermissionDenied);
        }
    }

    let helper_request = HelperRequest {
        request_id: RequestId::new(),
        username: user_context.username.clone(),
        uid: user_context.uid,
        gid: user_context.gid,
        command: HelperCommand::Chmod,
        root_id: request.root_id.clone(),
        relative_path,
        target_relative_path: None,
        policy_context: HelperPolicyContext {
            read_only_root: root.read_only,
            is_web_root: root.is_web_root,
        },
        limits: HelperLimits {
            max_directory_page_size: config.limits.max_directory_page_size,
            max_text_edit_size: config.limits.max_text_edit_size,
            max_chunk_size: config.limits.max_chunk_size,
            max_download_helper_bytes: crate::download::transfer::MAX_DOWNLOAD_HELPER_BYTES,
        },
        list_options: None,
        write_options: None,
        chmod_mode: Some(request.mode),
        archive_source_paths: None,
    };

    let response = helper_client.execute_helper(&helper_request)?;
    if !response.success {
        return Err(helper_response_to_error(&response));
    }

    Ok(OperationDonePayload {
        request_id: helper_request.request_id,
        operation: "CHMOD_FILE".to_owned(),
        safe_message: Some("Permissions changed.".to_owned()),
    })
}

fn helper_response_to_error(response: &HelperResponse) -> TealDriveError {
    match response.error_kind {
        Some(ErrorKind::PermissionDenied) => TealDriveError::PermissionDenied,
        Some(ErrorKind::PolicyDenied) => TealDriveError::PolicyDenied,
        Some(ErrorKind::NotFound) => TealDriveError::NotFound,
        Some(ErrorKind::InvalidPath) => TealDriveError::InvalidPath,
        Some(ErrorKind::InvalidTarget) => TealDriveError::InvalidTarget,
        Some(ErrorKind::AlreadyExists) => TealDriveError::AlreadyExists,
        Some(ErrorKind::DiskFull) => TealDriveError::DiskFull,
        Some(ErrorKind::ValidationError) => TealDriveError::InvalidFilename,
        _ => TealDriveError::HelperCommandNotImplemented,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AllowedRoot, RelativePath, RootId};
    use crate::helper::chmod::chmod_file_with_roots;
    use crate::protocol::schema::UnixMode;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    struct InProcessChmodHelper {
        roots: Vec<AllowedRoot>,
    }

    impl ChmodFileHelper for InProcessChmodHelper {
        fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
            Ok(chmod_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    fn user() -> UserContext {
        UserContext {
            username: "alice".to_owned(),
            uid: 1000,
            gid: 1000,
        }
    }

    fn temp_root() -> PathBuf {
        let path = std::env::temp_dir().join(format!("tealdrive-chmod-svc-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temp root");
        path
    }

    fn config_for(base: PathBuf, read_only: bool, is_web_root: bool, chmod_file: bool) -> AppConfig {
        AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: base,
                read_only,
                uploads_allowed: true,
                hidden_files_allowed: false,
                is_web_root,
            }],
            features: crate::policy::feature_gate::FeatureFlags {
                chmod_file,
                ..Default::default()
            },
            ..AppConfig::default()
        }
    }

    fn chmod_req(path: &str, mode: u32) -> ChmodFileRequest {
        ChmodFileRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(path),
            mode: UnixMode(mode),
        }
    }

    #[test]
    fn chmod_file_succeeds() {
        let base = temp_root();
        let file_path = base.join("script.sh");
        fs::write(&file_path, b"#!/bin/sh\necho hi").expect("write");
        fs::set_permissions(&file_path, fs::Permissions::from_mode(0o644)).expect("perm");

        let config = config_for(base.clone(), false, false, true);
        let helper = InProcessChmodHelper { roots: config.roots.clone() };
        let engine = PolicyEngine::new(&config);

        let result = chmod_file_request(
            &user(),
            chmod_req("script.sh", 0o755),
            &config,
            &engine,
            &helper,
        );

        assert!(result.is_ok());
        let meta = fs::metadata(&file_path).expect("meta");
        assert_eq!(meta.permissions().mode() & 0o777, 0o755);
    }

    #[test]
    fn chmod_file_feature_disabled_rejected() {
        let base = temp_root();
        fs::write(base.join("script.sh"), b"hi").expect("write");

        let config = config_for(base, false, false, false);
        let helper = InProcessChmodHelper { roots: config.roots.clone() };
        let engine = PolicyEngine::new(&config);

        let result = chmod_file_request(
            &user(),
            chmod_req("script.sh", 0o755),
            &config,
            &engine,
            &helper,
        );

        assert_eq!(result, Err(TealDriveError::FeatureDisabled));
    }

    #[test]
    fn chmod_file_read_only_root_rejected() {
        let base = temp_root();
        fs::write(base.join("readonly.txt"), b"hi").expect("write");

        let config = config_for(base, true, false, true);
        let helper = InProcessChmodHelper { roots: config.roots.clone() };
        let engine = PolicyEngine::new(&config);

        let result = chmod_file_request(
            &user(),
            chmod_req("readonly.txt", 0o644),
            &config,
            &engine,
            &helper,
        );

        assert_eq!(result, Err(TealDriveError::ReadOnlyRoot));
    }

    #[test]
    fn chmod_file_sensitive_file_rejected() {
        let base = temp_root();
        fs::write(base.join(".env"), b"SECRET=1").expect("write");

        let config = config_for(base, false, false, true);
        let helper = InProcessChmodHelper { roots: config.roots.clone() };
        let engine = PolicyEngine::new(&config);

        let result = chmod_file_request(
            &user(),
            chmod_req(".env", 0o600),
            &config,
            &engine,
            &helper,
        );

        assert_eq!(result, Err(TealDriveError::PolicyDenied));
    }

    #[test]
    fn chmod_file_webroot_executable_rejected() {
        let base = temp_root();
        fs::write(base.join("script.php"), b"<?php").expect("write");

        let config = config_for(base, false, true, true);
        let helper = InProcessChmodHelper { roots: config.roots.clone() };
        let engine = PolicyEngine::new(&config);

        let result = chmod_file_request(
            &user(),
            chmod_req("script.php", 0o755),
            &config,
            &engine,
            &helper,
        );

        assert_eq!(result, Err(TealDriveError::PermissionDenied));
    }

    #[test]
    fn chmod_file_root_itself_rejected() {
        let base = temp_root();
        let config = config_for(base, false, false, true);
        let helper = InProcessChmodHelper { roots: config.roots.clone() };
        let engine = PolicyEngine::new(&config);

        let result = chmod_file_request(
            &user(),
            chmod_req("", 0o755),
            &config,
            &engine,
            &helper,
        );

        assert_eq!(result, Err(TealDriveError::InvalidTarget));
    }
}
