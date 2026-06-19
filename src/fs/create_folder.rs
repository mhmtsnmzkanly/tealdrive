use crate::config::{AppConfig, RelativePath, UserContext};
use crate::errors::{ErrorKind, TealDriveError};
use crate::fs::filename::SafeFileName;
use crate::fs::path::{resolve_new_target, validate_relative_path};
use crate::helper::client::HelperProcessClient;
use crate::helper::types::{
    HelperCommand, HelperLimits, HelperPolicyContext, HelperRequest, HelperResponse,
};
use crate::policy::decision::{FileOperation, PolicyAuditMetadata, PolicyDecision, PolicyEngine};
use crate::policy::roots::require_allowed_root;
use crate::policy::sensitive::detect_sensitive;
use crate::protocol::frame::RequestId;
use crate::protocol::payload::{CreateFolderRequest, OperationDonePayload};

pub trait CreateFolderHelper {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError>;
}

impl CreateFolderHelper for HelperProcessClient {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        self.execute(request)
    }
}

pub fn create_folder_request<H: CreateFolderHelper>(
    user_context: &UserContext,
    request: CreateFolderRequest,
    config: &AppConfig,
    policy_engine: &PolicyEngine<'_>,
    helper_client: &H,
) -> Result<OperationDonePayload, TealDriveError> {
    let parent_relative_path = validate_relative_path(&request.parent_relative_path)?;
    let folder_name = SafeFileName::parse(&request.folder_name)?;
    let root = require_allowed_root(config.allowed_roots(), &request.root_id)?;
    let target_relative_path = join_relative_name(&parent_relative_path, &folder_name);
    if detect_sensitive(&target_relative_path, &config.sensitive_policy).is_sensitive {
        return Err(TealDriveError::PolicyDenied);
    }
    let _target = resolve_new_target(root, &parent_relative_path, &folder_name.0)?;

    match policy_engine.check_operation(
        FileOperation::CreateFolder,
        &request.root_id,
        &parent_relative_path,
        Some(&target_relative_path),
        Some(&folder_name),
        root.hidden_files_allowed,
    )? {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(reason) => {
            let _audit = PolicyAuditMetadata {
                action: FileOperation::CreateFolder,
                root_id: request.root_id.clone(),
                relative_path: parent_relative_path.clone(),
                target_relative_path: Some(target_relative_path),
                reason,
                safe_message: "Create folder denied by TealDrive policy.".to_owned(),
            };
            return Err(TealDriveError::PolicyDenied);
        }
    }

    let helper_request = HelperRequest {
        request_id: RequestId::new(),
        username: user_context.username.clone(),
        uid: user_context.uid,
        gid: user_context.gid,
        command: HelperCommand::CreateFolder,
        root_id: request.root_id,
        relative_path: parent_relative_path,
        target_relative_path: Some(RelativePath::new(folder_name.0)),
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
        operation: "CREATE_FOLDER".to_owned(),
        safe_message: Some("Folder created.".to_owned()),
    })
}

fn join_relative_name(parent: &RelativePath, folder_name: &SafeFileName) -> RelativePath {
    if parent.0.is_empty() {
        RelativePath::new(folder_name.0.clone())
    } else {
        RelativePath::new(format!("{}/{}", parent.0, folder_name.0))
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
        Some(ErrorKind::NameTooLong) => TealDriveError::InvalidFilename,
        Some(ErrorKind::DiskFull) => TealDriveError::DiskFull,
        _ => TealDriveError::HelperCommandNotImplemented,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AllowedRoot, RootId};
    use crate::helper::create_folder::create_folder_with_roots;
    use crate::protocol::codec::{decode_msgpack, encode_msgpack};
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    struct InProcessHelper {
        roots: Vec<AllowedRoot>,
    }

    impl CreateFolderHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(create_folder_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    fn temp_root() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("tealdrive-service-create-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temp root");
        path
    }

    fn config_for(path: PathBuf, read_only: bool, hidden_allowed: bool) -> AppConfig {
        AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: path,
                read_only,
                uploads_allowed: true,
                hidden_files_allowed: hidden_allowed,
                is_web_root: false,
            }],
            ..AppConfig::default()
        }
    }

    fn request(parent: &str, name: &str) -> CreateFolderRequest {
        CreateFolderRequest {
            root_id: RootId::new("home"),
            parent_relative_path: RelativePath::new(parent),
            folder_name: name.to_owned(),
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
    fn normal_create_folder_succeeds() {
        let base = temp_root();
        let config = config_for(base.clone(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let done = create_folder_request(
            &user(),
            request("", "new-folder"),
            &config,
            &engine,
            &helper,
        )
        .expect("created");

        assert_eq!(done.operation, "CREATE_FOLDER");
        assert!(base.join("new-folder").is_dir());
    }

    #[test]
    fn create_folder_unknown_root_rejected() {
        let config = config_for(temp_root(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let mut req = request("", "new-folder");
        req.root_id = RootId::new("missing");

        assert_eq!(
            create_folder_request(&user(), req, &config, &engine, &helper),
            Err(TealDriveError::InvalidRootId)
        );
    }

    #[test]
    fn create_folder_rejects_absolute_and_traversal_parent() {
        let config = config_for(temp_root(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            create_folder_request(&user(), request("/etc", "new"), &config, &engine, &helper),
            Err(TealDriveError::AbsolutePathRejected)
        );
        assert_eq!(
            create_folder_request(&user(), request("../etc", "new"), &config, &engine, &helper),
            Err(TealDriveError::TraversalRejected)
        );
    }

    #[test]
    fn create_folder_rejects_invalid_names() {
        let config = config_for(temp_root(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            create_folder_request(&user(), request("", "bad/name"), &config, &engine, &helper),
            Err(TealDriveError::InvalidFilename)
        );
        assert_eq!(
            create_folder_request(&user(), request("", "bad\0name"), &config, &engine, &helper),
            Err(TealDriveError::NullByteRejected)
        );
        assert_eq!(
            create_folder_request(&user(), request("", "."), &config, &engine, &helper),
            Err(TealDriveError::ReservedNameRejected)
        );
        assert_eq!(
            create_folder_request(&user(), request("", ".."), &config, &engine, &helper),
            Err(TealDriveError::ReservedNameRejected)
        );
    }

    #[test]
    fn read_only_root_denies_create_before_helper() {
        let config = config_for(temp_root(), true, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper { roots: Vec::new() };

        assert_eq!(
            create_folder_request(&user(), request("", "new"), &config, &engine, &helper),
            Err(TealDriveError::PolicyDenied)
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_parent_denied() {
        let base = temp_root();
        fs::create_dir(base.join("real")).expect("real");
        symlink(base.join("real"), base.join("link")).expect("symlink");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            create_folder_request(&user(), request("link", "new"), &config, &engine, &helper),
            Err(TealDriveError::SymlinkDenied)
        );
    }

    #[test]
    fn already_exists_maps_to_already_exists() {
        let base = temp_root();
        fs::create_dir(base.join("existing")).expect("existing");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            create_folder_request(&user(), request("", "existing"), &config, &engine, &helper),
            Err(TealDriveError::AlreadyExists)
        );
    }

    #[test]
    fn missing_parent_maps_not_found() {
        let config = config_for(temp_root(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            create_folder_request(
                &user(),
                request("missing", "child"),
                &config,
                &engine,
                &helper
            ),
            Err(TealDriveError::NotFound)
        );
    }

    #[test]
    fn parent_file_maps_invalid_target() {
        let base = temp_root();
        fs::write(base.join("file"), b"not dir").expect("file");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            create_folder_request(&user(), request("file", "child"), &config, &engine, &helper),
            Err(TealDriveError::InvalidTarget)
        );
    }

    #[test]
    fn sensitive_target_denied_without_leaking_path() {
        let base = temp_root();
        let config = config_for(base.clone(), false, true);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let error = create_folder_request(&user(), request("", ".env"), &config, &engine, &helper)
            .expect_err("denied");
        let serialized = format!("{error:?}");

        assert_eq!(error, TealDriveError::PolicyDenied);
        assert!(!serialized.contains(base.to_string_lossy().as_ref()));
    }

    #[test]
    fn create_folder_request_msgpack_roundtrip() {
        let request = request("docs", "new-folder");
        let bytes = encode_msgpack(&request).expect("encoded");
        let decoded: CreateFolderRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, request);
    }

    #[test]
    fn operation_done_payload_msgpack_roundtrip() {
        let done = OperationDonePayload {
            request_id: RequestId::new(),
            operation: "CREATE_FOLDER".to_owned(),
            safe_message: Some("Folder created.".to_owned()),
        };
        let bytes = encode_msgpack(&done).expect("encoded");
        let decoded: OperationDonePayload = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, done);
    }
}
