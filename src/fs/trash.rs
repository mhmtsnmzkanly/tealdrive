use crate::config::{AppConfig, RelativePath, UserContext};
use crate::errors::{ErrorKind, TealDriveError};
use crate::fs::filename::SafeFileName;
use crate::fs::path::{resolve_existing_path, validate_relative_path};
use crate::helper::client::HelperProcessClient;
use crate::helper::types::{
    HelperCommand, HelperLimits, HelperPolicyContext, HelperRequest, HelperResponse,
    HelperResponsePayload,
};
use crate::policy::decision::{FileOperation, PolicyAuditMetadata, PolicyDecision, PolicyEngine};
use crate::policy::roots::require_allowed_root;
use crate::policy::sensitive::detect_sensitive;
use crate::protocol::frame::RequestId;
use crate::protocol::payload::{
    MoveToTrashPayload, MoveToTrashRequest, OperationDonePayload, RestoreFromTrashRequest,
};

pub trait MoveToTrashHelper {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError>;
}

impl MoveToTrashHelper for HelperProcessClient {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        self.execute(request)
    }
}

pub trait RestoreFromTrashHelper {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError>;
}

impl RestoreFromTrashHelper for HelperProcessClient {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        self.execute(request)
    }
}

pub fn move_to_trash_request<H: MoveToTrashHelper>(
    user_context: &UserContext,
    request: MoveToTrashRequest,
    config: &AppConfig,
    policy_engine: &PolicyEngine<'_>,
    helper_client: &H,
) -> Result<MoveToTrashPayload, TealDriveError> {
    let relative_path = validate_relative_path(&request.relative_path)?;
    let root = require_allowed_root(config.allowed_roots(), &request.root_id)?;
    let source = resolve_existing_path(root, &relative_path, false)?;

    if detect_sensitive(&source.relative_path, &config.sensitive_policy).is_sensitive {
        return Err(TealDriveError::PolicyDenied);
    }

    match policy_engine.check_operation(
        FileOperation::MoveToTrash,
        &request.root_id,
        &source.relative_path,
        None,
        None,
        root.hidden_files_allowed,
    )? {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(reason) => {
            let _audit = PolicyAuditMetadata {
                action: FileOperation::MoveToTrash,
                root_id: request.root_id.clone(),
                relative_path: source.relative_path.clone(),
                target_relative_path: None,
                reason,
                safe_message: "Move to trash denied by TealDrive policy.".to_owned(),
            };
            return Err(TealDriveError::PolicyDenied);
        }
    }

    let helper_request = HelperRequest {
        request_id: RequestId::new(),
        username: user_context.username.clone(),
        uid: user_context.uid,
        gid: user_context.gid,
        command: HelperCommand::MoveToTrash,
        root_id: request.root_id,
        relative_path: source.relative_path,
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
    };

    let response = helper_client.execute_helper(&helper_request)?;
    if !response.success {
        return Err(helper_response_to_error(&response));
    }

    let entry = match response.payload {
        HelperResponsePayload::TrashEntry(e) => e,
        _ => return Err(TealDriveError::HelperCommandNotImplemented),
    };

    Ok(MoveToTrashPayload {
        request_id: helper_request.request_id,
        operation: "MOVE_TO_TRASH".to_owned(),
        trash_id: entry.trash_id,
        display_name: entry.display_name,
        deleted_at: entry.deleted_at,
        original_relative_path: entry.original_relative_path,
        original_root_id: entry.original_root_id,
    })
}

pub fn restore_from_trash_request<H: RestoreFromTrashHelper>(
    user_context: &UserContext,
    request: RestoreFromTrashRequest,
    config: &AppConfig,
    policy_engine: &PolicyEngine<'_>,
    helper_client: &H,
) -> Result<OperationDonePayload, TealDriveError> {
    let safe_trash_id = SafeFileName::parse(&request.trash_id)?;

    let root_id = request
        .target_root_id
        .ok_or(TealDriveError::InvalidTarget)?;

    let root = require_allowed_root(config.allowed_roots(), &root_id)?;
    let placeholder_path = RelativePath::new(safe_trash_id.0.clone());

    match policy_engine.check_operation(
        FileOperation::RestoreFromTrash,
        &root_id,
        &placeholder_path,
        None,
        None,
        root.hidden_files_allowed,
    )? {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(reason) => {
            let _audit = PolicyAuditMetadata {
                action: FileOperation::RestoreFromTrash,
                root_id: root_id.clone(),
                relative_path: placeholder_path.clone(),
                target_relative_path: None,
                reason,
                safe_message: "Restore from trash denied by TealDrive policy.".to_owned(),
            };
            return Err(TealDriveError::PolicyDenied);
        }
    }

    let helper_request = HelperRequest {
        request_id: RequestId::new(),
        username: user_context.username.clone(),
        uid: user_context.uid,
        gid: user_context.gid,
        command: HelperCommand::RestoreFromTrash,
        root_id,
        relative_path: placeholder_path,
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
    };

    let response = helper_client.execute_helper(&helper_request)?;
    if !response.success {
        return Err(helper_response_to_error(&response));
    }

    Ok(OperationDonePayload {
        request_id: helper_request.request_id,
        operation: "RESTORE_FROM_TRASH".to_owned(),
        safe_message: Some("Restored.".to_owned()),
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
    use crate::config::{AllowedRoot, RootId};
    use crate::helper::trash::{move_file_to_trash_with_roots, restore_file_from_trash_with_roots};
    use crate::protocol::codec::{decode_msgpack, encode_msgpack};
    use crate::protocol::payload::{MoveToTrashRequest, RestoreFromTrashRequest};
    use std::fs;
    use std::path::PathBuf;

    struct InProcessHelper {
        roots: Vec<AllowedRoot>,
    }

    impl MoveToTrashHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(move_file_to_trash_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl RestoreFromTrashHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(restore_file_from_trash_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    struct FailingIfCalled;

    impl MoveToTrashHelper for FailingIfCalled {
        fn execute_helper(
            &self,
            _request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            panic!("helper must not be called");
        }
    }

    impl RestoreFromTrashHelper for FailingIfCalled {
        fn execute_helper(
            &self,
            _request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            panic!("helper must not be called");
        }
    }

    fn temp_root() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("tealdrive-service-trash-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temp root");
        path
    }

    fn config_for(path: PathBuf, read_only: bool) -> AppConfig {
        AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: path,
                read_only,
                uploads_allowed: true,
                hidden_files_allowed: false,
                is_web_root: false,
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

    fn trash_req(path: &str) -> MoveToTrashRequest {
        MoveToTrashRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(path),
        }
    }

    fn restore_req(trash_id: &str) -> RestoreFromTrashRequest {
        RestoreFromTrashRequest {
            trash_id: trash_id.to_owned(),
            target_root_id: Some(RootId::new("home")),
            target_relative_path: None,
        }
    }

    #[test]
    fn move_to_trash_success() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"hello").expect("file");
        let config = config_for(base.clone(), false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        let result =
            move_to_trash_request(&user(), trash_req("file.txt"), &config, &engine, &helper)
                .expect("trash ok");

        assert_eq!(result.operation, "MOVE_TO_TRASH");
        assert_eq!(result.display_name, "file.txt");
        assert!(!result.trash_id.is_empty());
        assert!(!base.join("file.txt").exists());
    }

    #[test]
    fn restore_success() {
        let base = temp_root();
        fs::write(base.join("orig.txt"), b"data").expect("file");
        let config = config_for(base.clone(), false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        let trash_result =
            move_to_trash_request(&user(), trash_req("orig.txt"), &config, &engine, &helper)
                .expect("trash ok");

        let restore_result = restore_from_trash_request(
            &user(),
            restore_req(&trash_result.trash_id),
            &config,
            &engine,
            &helper,
        )
        .expect("restore ok");

        assert_eq!(restore_result.operation, "RESTORE_FROM_TRASH");
        assert!(base.join("orig.txt").exists());
    }

    #[test]
    fn unknown_root_rejected() {
        let config = config_for(temp_root(), false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let mut req = trash_req("file.txt");
        req.root_id = RootId::new("missing");

        assert_eq!(
            move_to_trash_request(&user(), req, &config, &engine, &helper),
            Err(TealDriveError::InvalidRootId)
        );
    }

    #[test]
    fn traversal_rejected() {
        let config = config_for(temp_root(), false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            move_to_trash_request(
                &user(),
                trash_req("../etc/passwd"),
                &config,
                &engine,
                &helper
            ),
            Err(TealDriveError::TraversalRejected)
        );
    }

    #[test]
    fn read_only_root_denies_move_to_trash() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"hello").expect("file");
        let config = config_for(base, true);
        let engine = PolicyEngine::new(&config);

        assert_eq!(
            move_to_trash_request(
                &user(),
                trash_req("file.txt"),
                &config,
                &engine,
                &FailingIfCalled
            ),
            Err(TealDriveError::PolicyDenied)
        );
    }

    #[test]
    fn read_only_root_denies_restore() {
        let config = config_for(temp_root(), true);
        let engine = PolicyEngine::new(&config);

        assert_eq!(
            restore_from_trash_request(
                &user(),
                restore_req("00000000-0000-0000-0000-000000000000"),
                &config,
                &engine,
                &FailingIfCalled
            ),
            Err(TealDriveError::PolicyDenied)
        );
    }

    #[test]
    fn missing_file_returns_not_found() {
        let config = config_for(temp_root(), false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            move_to_trash_request(&user(), trash_req("missing.txt"), &config, &engine, &helper),
            Err(TealDriveError::NotFound)
        );
    }

    #[test]
    fn missing_trash_entry_returns_not_found() {
        let config = config_for(temp_root(), false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            restore_from_trash_request(
                &user(),
                restore_req("00000000-0000-0000-0000-000000000000"),
                &config,
                &engine,
                &helper
            ),
            Err(TealDriveError::NotFound)
        );
    }

    #[test]
    fn restore_target_exists_returns_already_exists() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"original").expect("file");
        let config = config_for(base.clone(), false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        let trash_result =
            move_to_trash_request(&user(), trash_req("file.txt"), &config, &engine, &helper)
                .expect("trash ok");

        // Re-create original
        fs::write(base.join("file.txt"), b"new content").expect("re-create");

        assert_eq!(
            restore_from_trash_request(
                &user(),
                restore_req(&trash_result.trash_id),
                &config,
                &engine,
                &helper
            ),
            Err(TealDriveError::AlreadyExists)
        );
    }

    #[test]
    fn sensitive_source_denies_move_to_trash() {
        let base = temp_root();
        fs::write(base.join(".env"), b"SECRET=x").expect("file");
        let config = config_for(base, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            move_to_trash_request(&user(), trash_req(".env"), &config, &engine, &helper),
            Err(TealDriveError::PolicyDenied)
        );
    }

    #[test]
    fn restore_without_target_root_id_returns_invalid_target() {
        let config = config_for(temp_root(), false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            restore_from_trash_request(
                &user(),
                RestoreFromTrashRequest {
                    trash_id: "00000000-0000-0000-0000-000000000000".to_owned(),
                    target_root_id: None,
                    target_relative_path: None,
                },
                &config,
                &engine,
                &helper
            ),
            Err(TealDriveError::InvalidTarget)
        );
    }

    #[test]
    fn no_absolute_path_in_error() {
        let base = temp_root();
        let config = config_for(base.clone(), false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        let err =
            move_to_trash_request(&user(), trash_req("missing.txt"), &config, &engine, &helper)
                .expect_err("not found");

        assert!(!format!("{err}").contains(base.to_string_lossy().as_ref()));
    }

    #[test]
    fn move_to_trash_request_msgpack_roundtrip() {
        let req = trash_req("docs/report.pdf");
        let bytes = encode_msgpack(&req).expect("encoded");
        let decoded: MoveToTrashRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, req);
    }

    #[test]
    fn restore_from_trash_request_msgpack_roundtrip() {
        let req = RestoreFromTrashRequest {
            trash_id: "550e8400-e29b-41d4-a716-446655440000".to_owned(),
            target_root_id: Some(RootId::new("home")),
            target_relative_path: None,
        };
        let bytes = encode_msgpack(&req).expect("encoded");
        let decoded: RestoreFromTrashRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, req);
    }

    #[test]
    fn move_to_trash_payload_msgpack_roundtrip() {
        let payload = MoveToTrashPayload {
            request_id: crate::protocol::frame::RequestId::new(),
            operation: "MOVE_TO_TRASH".to_owned(),
            trash_id: "550e8400-e29b-41d4-a716-446655440000".to_owned(),
            display_name: "file.txt".to_owned(),
            deleted_at: 1_700_000_000,
            original_relative_path: RelativePath::new("docs/file.txt"),
            original_root_id: RootId::new("home"),
        };
        let bytes = encode_msgpack(&payload).expect("encoded");
        let decoded: MoveToTrashPayload = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, payload);
    }
}
