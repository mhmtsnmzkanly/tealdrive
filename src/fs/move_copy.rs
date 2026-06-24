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
use crate::protocol::payload::{CopyFileRequest, MoveFileRequest, OperationDonePayload};

pub trait MoveHelper {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError>;
}

impl MoveHelper for HelperProcessClient {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        self.execute(request)
    }
}

pub trait CopyHelper {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError>;
}

impl CopyHelper for HelperProcessClient {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        self.execute(request)
    }
}

pub fn move_file_request<H: MoveHelper>(
    user_context: &UserContext,
    request: MoveFileRequest,
    config: &AppConfig,
    policy_engine: &PolicyEngine<'_>,
    helper_client: &H,
) -> Result<OperationDonePayload, TealDriveError> {
    let source_relative_path = validate_relative_path(&request.relative_path)?;
    let target_relative_path = validate_relative_path(&request.target_relative_path)?;

    if request.root_id != request.target_root_id {
        return Err(TealDriveError::InvalidTarget);
    }

    let root = require_allowed_root(config.allowed_roots(), &request.root_id)?;
    let source = resolve_existing_path(root, &source_relative_path, false)?;

    let (target_parent_str, target_name_str) = split_target_path(&target_relative_path);
    let target_name = SafeFileName::parse(target_name_str)?;
    let target_parent = RelativePath::new(target_parent_str);
    let target = resolve_new_target(root, &target_parent, &target_name.0)?;

    if fs::symlink_metadata(&target.resolved_path).is_ok() {
        return Err(TealDriveError::AlreadyExists);
    }

    if detect_sensitive(&source.relative_path, &config.sensitive_policy).is_sensitive {
        return Err(TealDriveError::PolicyDenied);
    }
    if detect_sensitive(&target_relative_path, &config.sensitive_policy).is_sensitive {
        return Err(TealDriveError::PolicyDenied);
    }
    match check_webroot_upload(root.is_web_root, &target_name, &config.webroot_policy) {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(_) => return Err(TealDriveError::WebrootExecutableDenied),
    }

    match policy_engine.check_operation(
        FileOperation::MoveFile,
        &request.root_id,
        &source.relative_path,
        Some(&target_relative_path),
        Some(&target_name),
        root.hidden_files_allowed,
    )? {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(reason) => {
            let _audit = PolicyAuditMetadata {
                action: FileOperation::MoveFile,
                root_id: request.root_id.clone(),
                relative_path: source.relative_path.clone(),
                target_relative_path: Some(target_relative_path.clone()),
                reason,
                safe_message: "Move denied by TealDrive policy.".to_owned(),
            };
            return Err(TealDriveError::PolicyDenied);
        }
    }

    let helper_request = HelperRequest {
        request_id: RequestId::new(),
        username: user_context.username.clone(),
        uid: user_context.uid,
        gid: user_context.gid,
        command: HelperCommand::Move,
        root_id: request.root_id,
        relative_path: source.relative_path,
        target_relative_path: Some(target_relative_path),
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
        operation: "MOVE_FILE".to_owned(),
        safe_message: Some("Moved.".to_owned()),
    })
}

pub fn copy_file_request<H: CopyHelper>(
    user_context: &UserContext,
    request: CopyFileRequest,
    config: &AppConfig,
    policy_engine: &PolicyEngine<'_>,
    helper_client: &H,
) -> Result<OperationDonePayload, TealDriveError> {
    let source_relative_path = validate_relative_path(&request.relative_path)?;
    let target_relative_path = validate_relative_path(&request.target_relative_path)?;

    if request.root_id != request.target_root_id {
        return Err(TealDriveError::InvalidTarget);
    }

    let root = require_allowed_root(config.allowed_roots(), &request.root_id)?;
    let source = resolve_existing_path(root, &source_relative_path, false)?;

    let (target_parent_str, target_name_str) = split_target_path(&target_relative_path);
    let target_name = SafeFileName::parse(target_name_str)?;
    let target_parent = RelativePath::new(target_parent_str);
    let target = resolve_new_target(root, &target_parent, &target_name.0)?;

    if fs::symlink_metadata(&target.resolved_path).is_ok() {
        return Err(TealDriveError::AlreadyExists);
    }

    if detect_sensitive(&source.relative_path, &config.sensitive_policy).is_sensitive {
        return Err(TealDriveError::PolicyDenied);
    }
    if detect_sensitive(&target_relative_path, &config.sensitive_policy).is_sensitive {
        return Err(TealDriveError::PolicyDenied);
    }
    match check_webroot_upload(root.is_web_root, &target_name, &config.webroot_policy) {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(_) => return Err(TealDriveError::WebrootExecutableDenied),
    }

    match policy_engine.check_operation(
        FileOperation::CopyFile,
        &request.root_id,
        &source.relative_path,
        Some(&target_relative_path),
        Some(&target_name),
        root.hidden_files_allowed,
    )? {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(reason) => {
            let _audit = PolicyAuditMetadata {
                action: FileOperation::CopyFile,
                root_id: request.root_id.clone(),
                relative_path: source.relative_path.clone(),
                target_relative_path: Some(target_relative_path.clone()),
                reason,
                safe_message: "Copy denied by TealDrive policy.".to_owned(),
            };
            return Err(TealDriveError::PolicyDenied);
        }
    }

    let helper_request = HelperRequest {
        request_id: RequestId::new(),
        username: user_context.username.clone(),
        uid: user_context.uid,
        gid: user_context.gid,
        command: HelperCommand::Copy,
        root_id: request.root_id,
        relative_path: source.relative_path,
        target_relative_path: Some(target_relative_path),
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
        operation: "COPY_FILE".to_owned(),
        safe_message: Some("Copied.".to_owned()),
    })
}

fn split_target_path(target: &RelativePath) -> (&str, &str) {
    match target.0.rsplit_once('/') {
        Some((parent, name)) => (parent, name),
        None => ("", &target.0),
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
    use crate::helper::move_copy::{copy_file_with_roots, move_file_with_roots};
    use crate::protocol::codec::{decode_msgpack, encode_msgpack};
    use std::path::PathBuf;

    struct InProcessMoveHelper {
        roots: Vec<AllowedRoot>,
    }

    impl MoveHelper for InProcessMoveHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(move_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    struct InProcessCopyHelper {
        roots: Vec<AllowedRoot>,
    }

    impl CopyHelper for InProcessCopyHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(copy_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    struct FailingMoveHelper;

    impl MoveHelper for FailingMoveHelper {
        fn execute_helper(
            &self,
            _request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            panic!("move helper must not be called");
        }
    }

    struct FailingCopyHelper;

    impl CopyHelper for FailingCopyHelper {
        fn execute_helper(
            &self,
            _request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            panic!("copy helper must not be called");
        }
    }

    fn temp_root() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("tealdrive-service-move-{}", uuid::Uuid::new_v4()));
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

    fn user() -> UserContext {
        UserContext {
            username: "alice".to_owned(),
            uid: 1000,
            gid: 1000,
        }
    }

    fn move_req(src: &str, dst: &str) -> MoveFileRequest {
        MoveFileRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(src),
            target_root_id: RootId::new("home"),
            target_relative_path: RelativePath::new(dst),
        }
    }

    fn copy_req(src: &str, dst: &str) -> CopyFileRequest {
        CopyFileRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(src),
            target_root_id: RootId::new("home"),
            target_relative_path: RelativePath::new(dst),
        }
    }

    #[test]
    fn normal_file_move_succeeds() {
        let base = temp_root();
        fs::write(base.join("old.txt"), b"hello").expect("file");
        let config = config_for(base.clone(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessMoveHelper {
            roots: config.roots.clone(),
        };
        let done = move_file_request(
            &user(),
            move_req("old.txt", "new.txt"),
            &config,
            &engine,
            &helper,
        )
        .expect("moved");

        assert_eq!(done.operation, "MOVE_FILE");
        assert!(!base.join("old.txt").exists());
        assert!(base.join("new.txt").is_file());
    }

    #[test]
    fn move_into_subdirectory_succeeds() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"hello").expect("file");
        fs::create_dir(base.join("archive")).expect("dir");
        let config = config_for(base.clone(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessMoveHelper {
            roots: config.roots.clone(),
        };

        move_file_request(
            &user(),
            move_req("file.txt", "archive/file.txt"),
            &config,
            &engine,
            &helper,
        )
        .expect("moved");

        assert!(!base.join("file.txt").exists());
        assert!(base.join("archive/file.txt").is_file());
    }

    #[test]
    fn cross_root_move_denied() {
        let config = config_for(temp_root(), false, false);
        let engine = PolicyEngine::new(&config);
        let mut req = move_req("old.txt", "new.txt");
        req.target_root_id = RootId::new("other");

        assert_eq!(
            move_file_request(&user(), req, &config, &engine, &FailingMoveHelper),
            Err(TealDriveError::InvalidTarget)
        );
    }

    #[test]
    fn move_unknown_root_rejected() {
        let config = config_for(temp_root(), false, false);
        let engine = PolicyEngine::new(&config);
        let mut req = move_req("old.txt", "new.txt");
        req.root_id = RootId::new("missing");
        req.target_root_id = RootId::new("missing");

        assert_eq!(
            move_file_request(&user(), req, &config, &engine, &FailingMoveHelper),
            Err(TealDriveError::InvalidRootId)
        );
    }

    #[test]
    fn read_only_root_denies_move_before_helper() {
        let base = temp_root();
        fs::write(base.join("old.txt"), b"hello").expect("file");
        let config = config_for(base, true, false);
        let engine = PolicyEngine::new(&config);

        assert_eq!(
            move_file_request(
                &user(),
                move_req("old.txt", "new.txt"),
                &config,
                &engine,
                &FailingMoveHelper,
            ),
            Err(TealDriveError::PolicyDenied)
        );
    }

    #[test]
    fn move_target_already_exists() {
        let base = temp_root();
        fs::write(base.join("old.txt"), b"hello").expect("file");
        fs::write(base.join("new.txt"), b"existing").expect("file");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessMoveHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            move_file_request(
                &user(),
                move_req("old.txt", "new.txt"),
                &config,
                &engine,
                &helper
            ),
            Err(TealDriveError::AlreadyExists)
        );
    }

    #[test]
    fn source_sensitive_denies_move() {
        let base = temp_root();
        fs::write(base.join(".env"), b"SECRET=value").expect("file");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessMoveHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            move_file_request(
                &user(),
                move_req(".env", "env.txt"),
                &config,
                &engine,
                &helper
            ),
            Err(TealDriveError::PolicyDenied)
        );
    }

    #[test]
    fn webroot_move_to_php_denied() {
        let base = temp_root();
        fs::write(base.join("notes.txt"), b"hello").expect("file");
        let config = config_for(base, false, true);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessMoveHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            move_file_request(
                &user(),
                move_req("notes.txt", "shell.php"),
                &config,
                &engine,
                &helper,
            ),
            Err(TealDriveError::WebrootExecutableDenied)
        );
    }

    #[test]
    fn move_source_missing_maps_to_not_found() {
        let config = config_for(temp_root(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessMoveHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            move_file_request(
                &user(),
                move_req("missing.txt", "new.txt"),
                &config,
                &engine,
                &helper,
            ),
            Err(TealDriveError::NotFound)
        );
    }

    #[test]
    fn no_absolute_path_in_move_error() {
        let base = temp_root();
        fs::write(base.join("old.txt"), b"hello").expect("file");
        fs::write(base.join("new.txt"), b"existing").expect("file");
        let config = config_for(base.clone(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessMoveHelper {
            roots: config.roots.clone(),
        };
        let error = move_file_request(
            &user(),
            move_req("old.txt", "new.txt"),
            &config,
            &engine,
            &helper,
        )
        .expect_err("already exists");

        assert!(!format!("{error}").contains(base.to_string_lossy().as_ref()));
    }

    #[test]
    fn normal_file_copy_succeeds() {
        let base = temp_root();
        fs::write(base.join("orig.txt"), b"hello").expect("file");
        let config = config_for(base.clone(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessCopyHelper {
            roots: config.roots.clone(),
        };
        let done = copy_file_request(
            &user(),
            copy_req("orig.txt", "copy.txt"),
            &config,
            &engine,
            &helper,
        )
        .expect("copied");

        assert_eq!(done.operation, "COPY_FILE");
        assert!(base.join("orig.txt").is_file());
        assert!(base.join("copy.txt").is_file());
    }

    #[test]
    fn copy_directory_rejected() {
        let base = temp_root();
        fs::create_dir(base.join("mydir")).expect("dir");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessCopyHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            copy_file_request(
                &user(),
                copy_req("mydir", "mydir-copy"),
                &config,
                &engine,
                &helper,
            ),
            Err(TealDriveError::InvalidTarget)
        );
    }

    #[test]
    fn cross_root_copy_denied() {
        let config = config_for(temp_root(), false, false);
        let engine = PolicyEngine::new(&config);
        let mut req = copy_req("orig.txt", "copy.txt");
        req.target_root_id = RootId::new("other");

        assert_eq!(
            copy_file_request(&user(), req, &config, &engine, &FailingCopyHelper),
            Err(TealDriveError::InvalidTarget)
        );
    }

    #[test]
    fn read_only_root_denies_copy_before_helper() {
        let base = temp_root();
        fs::write(base.join("orig.txt"), b"hello").expect("file");
        let config = config_for(base, true, false);
        let engine = PolicyEngine::new(&config);

        assert_eq!(
            copy_file_request(
                &user(),
                copy_req("orig.txt", "copy.txt"),
                &config,
                &engine,
                &FailingCopyHelper,
            ),
            Err(TealDriveError::PolicyDenied)
        );
    }

    #[test]
    fn copy_target_already_exists() {
        let base = temp_root();
        fs::write(base.join("orig.txt"), b"hello").expect("file");
        fs::write(base.join("copy.txt"), b"existing").expect("file");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessCopyHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            copy_file_request(
                &user(),
                copy_req("orig.txt", "copy.txt"),
                &config,
                &engine,
                &helper,
            ),
            Err(TealDriveError::AlreadyExists)
        );
    }

    #[test]
    fn source_sensitive_denies_copy() {
        let base = temp_root();
        fs::write(base.join(".env"), b"SECRET=value").expect("file");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessCopyHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            copy_file_request(
                &user(),
                copy_req(".env", "env.txt"),
                &config,
                &engine,
                &helper
            ),
            Err(TealDriveError::PolicyDenied)
        );
    }

    #[test]
    fn move_file_request_msgpack_roundtrip() {
        let request = move_req("old.txt", "archive/old.txt");
        let bytes = encode_msgpack(&request).expect("encoded");
        let decoded: MoveFileRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, request);
    }

    #[test]
    fn copy_file_request_msgpack_roundtrip() {
        let request = copy_req("orig.txt", "copies/orig.txt");
        let bytes = encode_msgpack(&request).expect("encoded");
        let decoded: CopyFileRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, request);
    }
}
