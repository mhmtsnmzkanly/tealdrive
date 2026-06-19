use crate::config::{AppConfig, RelativePath, UserContext};
use crate::errors::{ErrorKind, TealDriveError};
use crate::fs::listing::helper_entry_to_payload;
use crate::fs::path::validate_relative_path;
use crate::helper::client::HelperProcessClient;
use crate::helper::types::{
    HelperCommand, HelperLimits, HelperPolicyContext, HelperRequest, HelperResponse,
    HelperResponsePayload,
};
use crate::policy::decision::{FileOperation, PolicyAuditMetadata, PolicyDecision, PolicyEngine};
use crate::policy::roots::require_allowed_root;
use crate::protocol::frame::RequestId;
use crate::protocol::payload::{FileMetadataPayload, FileMetadataRequest};

pub trait MetadataHelper {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError>;
}

impl MetadataHelper for HelperProcessClient {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        self.execute(request)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMetadataAudit {
    pub request_id: RequestId,
    pub username: String,
    pub root_id: crate::config::RootId,
    pub relative_path: RelativePath,
    pub status: String,
    pub reason: Option<String>,
}

pub fn file_metadata_request<H: MetadataHelper>(
    user_context: &UserContext,
    request: FileMetadataRequest,
    config: &AppConfig,
    policy_engine: &PolicyEngine<'_>,
    helper_client: &H,
) -> Result<FileMetadataPayload, TealDriveError> {
    let relative_path = validate_relative_path(&request.relative_path)?;
    let root = require_allowed_root(config.allowed_roots(), &request.root_id)?;
    match policy_engine.check_operation(
        FileOperation::FileMetadata,
        &request.root_id,
        &relative_path,
        None,
        None,
        root.hidden_files_allowed,
    )? {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(reason) => {
            let _audit = PolicyAuditMetadata {
                action: FileOperation::FileMetadata,
                root_id: request.root_id.clone(),
                relative_path: relative_path.clone(),
                target_relative_path: None,
                reason,
                safe_message: "File metadata denied by TealDrive policy.".to_owned(),
            };
            return Err(TealDriveError::PolicyDenied);
        }
    }

    let helper_request = HelperRequest {
        request_id: RequestId::new(),
        username: user_context.username.clone(),
        uid: user_context.uid,
        gid: user_context.gid,
        command: HelperCommand::FileMetadata,
        root_id: request.root_id.clone(),
        relative_path: relative_path.clone(),
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

    let HelperResponsePayload::FileMetadata(entry) = response.payload else {
        return Err(TealDriveError::HelperMalformedResponse);
    };
    Ok(FileMetadataPayload {
        root_id: request.root_id,
        relative_path,
        entry: helper_entry_to_payload(entry),
    })
}

fn helper_response_to_error(response: &HelperResponse) -> TealDriveError {
    match response.error_kind {
        Some(ErrorKind::PermissionDenied) => TealDriveError::PermissionDenied,
        Some(ErrorKind::NotFound) => TealDriveError::NotFound,
        Some(ErrorKind::InvalidPath) => TealDriveError::InvalidPath,
        Some(ErrorKind::PolicyDenied) => TealDriveError::PolicyDenied,
        _ => TealDriveError::HelperCommandNotImplemented,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AllowedRoot, RootId};
    use crate::helper::metadata::file_metadata_with_roots;
    use crate::policy::decision::PolicyEngine;
    use crate::protocol::codec::{decode_msgpack, encode_msgpack};
    use crate::protocol::schema::FileKind;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    struct InProcessHelper {
        roots: Vec<AllowedRoot>,
    }

    impl MetadataHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(file_metadata_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    fn temp_root() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("tealdrive-service-meta-{}", uuid::Uuid::new_v4()));
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

    fn request(path: &str) -> FileMetadataRequest {
        FileMetadataRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(path),
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
    fn file_metadata_unknown_root_rejected() {
        let config = config_for(temp_root(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let mut req = request("file.txt");
        req.root_id = RootId::new("missing");

        assert_eq!(
            file_metadata_request(&user(), req, &config, &engine, &helper),
            Err(TealDriveError::InvalidRootId)
        );
    }

    #[test]
    fn file_metadata_rejects_absolute_and_traversal() {
        let config = config_for(temp_root(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            file_metadata_request(&user(), request("/etc"), &config, &engine, &helper),
            Err(TealDriveError::AbsolutePathRejected)
        );
        assert_eq!(
            file_metadata_request(&user(), request("../etc"), &config, &engine, &helper),
            Err(TealDriveError::TraversalRejected)
        );
    }

    #[test]
    fn file_metadata_normal_file_succeeds() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"hello").expect("file");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let payload =
            file_metadata_request(&user(), request("file.txt"), &config, &engine, &helper)
                .expect("metadata");

        assert_eq!(payload.entry.name, "file.txt");
        assert_eq!(payload.entry.file_type, FileKind::File);
        assert_eq!(payload.entry.size, 5);
    }

    #[test]
    fn file_metadata_directory_succeeds() {
        let base = temp_root();
        fs::create_dir(base.join("folder")).expect("folder");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let payload = file_metadata_request(&user(), request("folder"), &config, &engine, &helper)
            .expect("metadata");

        assert_eq!(payload.entry.file_type, FileKind::Directory);
    }

    #[test]
    fn read_only_root_allows_metadata() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"hello").expect("file");
        let config = config_for(base, true, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert!(
            file_metadata_request(&user(), request("file.txt"), &config, &engine, &helper).is_ok()
        );
    }

    #[test]
    fn hidden_file_metadata_respects_policy() {
        let base = temp_root();
        fs::write(base.join(".hidden"), b"hidden").expect("hidden");
        let denied_config = config_for(base.clone(), false, false);
        let denied_engine = PolicyEngine::new(&denied_config);
        let denied_helper = InProcessHelper {
            roots: denied_config.roots.clone(),
        };
        let allowed_config = config_for(base, false, true);
        let allowed_engine = PolicyEngine::new(&allowed_config);
        let allowed_helper = InProcessHelper {
            roots: allowed_config.roots.clone(),
        };

        assert_eq!(
            file_metadata_request(
                &user(),
                request(".hidden"),
                &denied_config,
                &denied_engine,
                &denied_helper
            ),
            Err(TealDriveError::PolicyDenied)
        );
        assert!(file_metadata_request(
            &user(),
            request(".hidden"),
            &allowed_config,
            &allowed_engine,
            &allowed_helper
        )
        .is_ok());
    }

    #[test]
    fn sensitive_file_flagged_without_content_read() {
        let base = temp_root();
        fs::write(base.join(".env"), b"SECRET=1").expect("env");
        let config = config_for(base, false, true);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let payload = file_metadata_request(&user(), request(".env"), &config, &engine, &helper)
            .expect("metadata");
        let serialized = format!("{payload:?}");

        assert!(payload.entry.is_sensitive);
        assert!(!serialized.contains("SECRET=1"));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_file_metadata_flagged_not_followed() {
        let base = temp_root();
        fs::write(base.join("target"), b"hello").expect("target");
        symlink(base.join("target"), base.join("link")).expect("symlink");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let payload = file_metadata_request(&user(), request("link"), &config, &engine, &helper)
            .expect("metadata");

        assert!(payload.entry.is_symlink);
        assert_eq!(payload.entry.file_type, FileKind::Symlink);
        assert!(payload.entry.symlink_target.is_none());
    }

    #[test]
    fn file_metadata_payload_msgpack_roundtrip() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"hello").expect("file");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let payload =
            file_metadata_request(&user(), request("file.txt"), &config, &engine, &helper)
                .expect("metadata");
        let bytes = encode_msgpack(&payload).expect("encoded");
        let decoded: FileMetadataPayload = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, payload);
    }

    #[test]
    fn file_metadata_request_msgpack_roundtrip() {
        let request = request("file.txt");
        let bytes = encode_msgpack(&request).expect("encoded");
        let decoded: FileMetadataRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, request);
    }
}
