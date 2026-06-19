use std::fs;

use crate::config::{AppConfig, RelativePath, UserContext};
use crate::errors::{ErrorKind, TealDriveError};
use crate::fs::filename::SafeFileName;
use crate::fs::path::{resolve_existing_path, resolve_new_target, validate_relative_path};
use crate::helper::client::HelperProcessClient;
use crate::helper::types::{
    HelperCommand, HelperLimits, HelperPolicyContext, HelperRequest, HelperResponse,
};
use crate::policy::decision::{FileOperation, PolicyAuditMetadata, PolicyDecision, PolicyEngine};
use crate::policy::roots::require_allowed_root;
use crate::policy::sensitive::detect_sensitive;
use crate::policy::webroot::check_webroot_upload;
use crate::protocol::frame::RequestId;
use crate::protocol::payload::{OperationDonePayload, RenameFileRequest};

pub trait RenameHelper {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError>;
}

impl RenameHelper for HelperProcessClient {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        self.execute(request)
    }
}

pub fn rename_file_request<H: RenameHelper>(
    user_context: &UserContext,
    request: RenameFileRequest,
    config: &AppConfig,
    policy_engine: &PolicyEngine<'_>,
    helper_client: &H,
) -> Result<OperationDonePayload, TealDriveError> {
    let relative_path = validate_relative_path(&request.relative_path)?;
    let new_name = SafeFileName::parse(&request.new_name)?;
    let root = require_allowed_root(config.allowed_roots(), &request.root_id)?;

    let source = resolve_existing_path(root, &relative_path, false)?;
    let parent_relative_path = parent_relative_path(&source.relative_path);
    let target_relative_path = join_relative_name(&parent_relative_path, &new_name);

    if detect_sensitive(&source.relative_path, &config.sensitive_policy).is_sensitive {
        return Err(TealDriveError::PolicyDenied);
    }
    if detect_sensitive(&target_relative_path, &config.sensitive_policy).is_sensitive {
        return Err(TealDriveError::PolicyDenied);
    }
    match check_webroot_upload(root.is_web_root, &new_name, &config.webroot_policy) {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(_) => return Err(TealDriveError::WebrootExecutableDenied),
    }

    let target = resolve_new_target(root, &parent_relative_path, &new_name.0)?;
    if fs::symlink_metadata(&target.resolved_path).is_ok() {
        return Err(TealDriveError::AlreadyExists);
    }

    match policy_engine.check_operation(
        FileOperation::RenameFile,
        &request.root_id,
        &source.relative_path,
        Some(&target_relative_path),
        Some(&new_name),
        root.hidden_files_allowed,
    )? {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(reason) => {
            let _audit = PolicyAuditMetadata {
                action: FileOperation::RenameFile,
                root_id: request.root_id.clone(),
                relative_path: source.relative_path.clone(),
                target_relative_path: Some(target_relative_path),
                reason,
                safe_message: "Rename denied by TealDrive policy.".to_owned(),
            };
            return Err(TealDriveError::PolicyDenied);
        }
    }

    let helper_request = HelperRequest {
        request_id: RequestId::new(),
        username: user_context.username.clone(),
        uid: user_context.uid,
        gid: user_context.gid,
        command: HelperCommand::Rename,
        root_id: request.root_id,
        relative_path: source.relative_path,
        target_relative_path: Some(RelativePath::new(new_name.0)),
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
    };

    let response = helper_client.execute_helper(&helper_request)?;
    if !response.success {
        return Err(helper_response_to_error(&response));
    }

    Ok(OperationDonePayload {
        request_id: helper_request.request_id,
        operation: "RENAME_FILE".to_owned(),
        safe_message: Some("Renamed.".to_owned()),
    })
}

fn parent_relative_path(path: &RelativePath) -> RelativePath {
    match path.0.rsplit_once('/') {
        Some((parent, _)) => RelativePath::new(parent),
        None => RelativePath::new(""),
    }
}

fn join_relative_name(parent: &RelativePath, name: &SafeFileName) -> RelativePath {
    if parent.0.is_empty() {
        RelativePath::new(name.0.clone())
    } else {
        RelativePath::new(format!("{}/{}", parent.0, name.0))
    }
}

fn helper_response_to_error(response: &HelperResponse) -> TealDriveError {
    match response.error_kind {
        Some(ErrorKind::PermissionDenied) => TealDriveError::PermissionDenied,
        Some(ErrorKind::PolicyDenied) => TealDriveError::PolicyDenied,
        Some(ErrorKind::NotFound) => TealDriveError::NotFound,
        Some(ErrorKind::InvalidPath) => TealDriveError::InvalidPath,
        Some(ErrorKind::InvalidTarget) => TealDriveError::InvalidTarget,
        Some(ErrorKind::AlreadyExists) => TealDriveError::AlreadyExists,
        Some(ErrorKind::CrossDeviceMove) => TealDriveError::CrossDeviceMove,
        Some(ErrorKind::NameTooLong) => TealDriveError::InvalidFilename,
        Some(ErrorKind::DiskFull) => TealDriveError::DiskFull,
        _ => TealDriveError::HelperCommandNotImplemented,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AllowedRoot, RootId};
    use crate::helper::rename::rename_with_roots;
    use crate::protocol::codec::{decode_msgpack, encode_msgpack};
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    struct InProcessHelper {
        roots: Vec<AllowedRoot>,
    }

    impl RenameHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(rename_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    struct FailingIfCalled;

    impl RenameHelper for FailingIfCalled {
        fn execute_helper(
            &self,
            _request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            panic!("helper must not be called");
        }
    }

    fn temp_root() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("tealdrive-service-rename-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temp root");
        path
    }

    fn config_for(path: PathBuf, read_only: bool, is_web_root: bool) -> AppConfig {
        AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: path,
                read_only,
                uploads_allowed: true,
                hidden_files_allowed: false,
                is_web_root,
            }],
            ..AppConfig::default()
        }
    }

    fn request(path: &str, new_name: &str) -> RenameFileRequest {
        RenameFileRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(path),
            new_name: new_name.to_owned(),
        }
    }

    fn user() -> UserContext {
        UserContext {
            username: "alice".to_owned(),
            uid: 1000,
            gid: 1000,
        }
    }

    #[test]
    fn normal_file_rename_succeeds() {
        let base = temp_root();
        fs::write(base.join("old.txt"), b"hello").expect("file");
        let config = config_for(base.clone(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let done = rename_file_request(
            &user(),
            request("old.txt", "new.txt"),
            &config,
            &engine,
            &helper,
        )
        .expect("renamed");

        assert_eq!(done.operation, "RENAME_FILE");
        assert!(!base.join("old.txt").exists());
        assert!(base.join("new.txt").is_file());
    }

    #[test]
    fn normal_directory_rename_succeeds() {
        let base = temp_root();
        fs::create_dir(base.join("old-dir")).expect("dir");
        let config = config_for(base.clone(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        rename_file_request(
            &user(),
            request("old-dir", "new-dir"),
            &config,
            &engine,
            &helper,
        )
        .expect("renamed");

        assert!(base.join("new-dir").is_dir());
    }

    #[test]
    fn rename_unknown_root_rejected() {
        let config = config_for(temp_root(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let mut req = request("old.txt", "new.txt");
        req.root_id = RootId::new("missing");

        assert_eq!(
            rename_file_request(&user(), req, &config, &engine, &helper),
            Err(TealDriveError::InvalidRootId)
        );
    }

    #[test]
    fn rename_rejects_absolute_and_traversal_source() {
        let config = config_for(temp_root(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            rename_file_request(&user(), request("/etc", "new"), &config, &engine, &helper),
            Err(TealDriveError::AbsolutePathRejected)
        );
        assert_eq!(
            rename_file_request(&user(), request("../etc", "new"), &config, &engine, &helper),
            Err(TealDriveError::TraversalRejected)
        );
    }

    #[test]
    fn rename_rejects_invalid_new_names() {
        let base = temp_root();
        fs::write(base.join("old.txt"), b"hello").expect("file");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            rename_file_request(
                &user(),
                request("old.txt", "bad/name"),
                &config,
                &engine,
                &helper
            ),
            Err(TealDriveError::InvalidFilename)
        );
        assert_eq!(
            rename_file_request(
                &user(),
                request("old.txt", "bad\\name"),
                &config,
                &engine,
                &helper
            ),
            Err(TealDriveError::InvalidFilename)
        );
        assert_eq!(
            rename_file_request(
                &user(),
                request("old.txt", "bad\0name"),
                &config,
                &engine,
                &helper
            ),
            Err(TealDriveError::NullByteRejected)
        );
        assert_eq!(
            rename_file_request(&user(), request("old.txt", "."), &config, &engine, &helper),
            Err(TealDriveError::ReservedNameRejected)
        );
        assert_eq!(
            rename_file_request(&user(), request("old.txt", ".."), &config, &engine, &helper),
            Err(TealDriveError::ReservedNameRejected)
        );
    }

    #[test]
    fn read_only_root_denies_rename_before_helper() {
        let base = temp_root();
        fs::write(base.join("old.txt"), b"hello").expect("file");
        let config = config_for(base, true, false);
        let engine = PolicyEngine::new(&config);

        assert_eq!(
            rename_file_request(
                &user(),
                request("old.txt", "new.txt"),
                &config,
                &engine,
                &FailingIfCalled
            ),
            Err(TealDriveError::PolicyDenied)
        );
    }

    #[cfg(unix)]
    #[test]
    fn source_symlink_denied() {
        let base = temp_root();
        fs::write(base.join("target.txt"), b"hello").expect("file");
        symlink(base.join("target.txt"), base.join("link.txt")).expect("symlink");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            rename_file_request(
                &user(),
                request("link.txt", "renamed.txt"),
                &config,
                &engine,
                &helper
            ),
            Err(TealDriveError::SymlinkDenied)
        );
    }

    #[test]
    fn target_already_exists_maps_to_already_exists() {
        let base = temp_root();
        fs::write(base.join("old.txt"), b"hello").expect("file");
        fs::write(base.join("new.txt"), b"existing").expect("file");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            rename_file_request(
                &user(),
                request("old.txt", "new.txt"),
                &config,
                &engine,
                &helper
            ),
            Err(TealDriveError::AlreadyExists)
        );
    }

    #[test]
    fn source_missing_maps_to_not_found() {
        let config = config_for(temp_root(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            rename_file_request(
                &user(),
                request("missing.txt", "new.txt"),
                &config,
                &engine,
                &helper
            ),
            Err(TealDriveError::NotFound)
        );
    }

    #[test]
    fn source_sensitive_denied() {
        let base = temp_root();
        fs::write(base.join(".env"), b"SECRET=value").expect("file");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            rename_file_request(
                &user(),
                request(".env", "env.txt"),
                &config,
                &engine,
                &helper
            ),
            Err(TealDriveError::PolicyDenied)
        );
    }

    #[test]
    fn webroot_rename_to_php_denied() {
        let base = temp_root();
        fs::write(base.join("notes.txt"), b"hello").expect("file");
        let config = config_for(base, false, true);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            rename_file_request(
                &user(),
                request("notes.txt", "shell.php"),
                &config,
                &engine,
                &helper
            ),
            Err(TealDriveError::WebrootExecutableDenied)
        );
    }

    #[test]
    fn no_absolute_path_in_error_payload() {
        let base = temp_root();
        fs::write(base.join("old.txt"), b"hello").expect("file");
        fs::write(base.join("new.txt"), b"existing").expect("file");
        let config = config_for(base.clone(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let error = rename_file_request(
            &user(),
            request("old.txt", "new.txt"),
            &config,
            &engine,
            &helper,
        )
        .expect_err("already exists");

        assert!(!format!("{error}").contains(base.to_string_lossy().as_ref()));
    }

    #[test]
    fn rename_file_request_msgpack_roundtrip() {
        let request = request("old.txt", "new.txt");
        let bytes = encode_msgpack(&request).expect("encoded");
        let decoded: RenameFileRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, request);
    }

    #[test]
    fn operation_done_msgpack_roundtrip() {
        let payload = OperationDonePayload {
            request_id: RequestId::new(),
            operation: "RENAME_FILE".to_owned(),
            safe_message: Some("Renamed.".to_owned()),
        };
        let bytes = encode_msgpack(&payload).expect("encoded");
        let decoded: OperationDonePayload = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, payload);
    }
}
