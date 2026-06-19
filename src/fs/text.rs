use crate::config::{AppConfig, UserContext};
use crate::errors::{ErrorKind, TealDriveError};
use crate::fs::metadata::{file_metadata_request, MetadataHelper};
use crate::fs::path::validate_relative_path;
use crate::helper::client::HelperProcessClient;
use crate::helper::types::{
    HelperCommand, HelperLimits, HelperPolicyContext, HelperRequest, HelperResponse,
    HelperResponsePayload,
};
use crate::policy::decision::{FileOperation, PolicyAuditMetadata, PolicyDecision, PolicyEngine};
use crate::policy::roots::require_allowed_root;
use crate::protocol::frame::RequestId;
use crate::protocol::payload::{FileMetadataRequest, ReadTextFileRequest, TextFileContentPayload};
use crate::protocol::schema::FileKind;

pub trait TextPreviewHelper {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError>;
}

impl TextPreviewHelper for HelperProcessClient {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        self.execute(request)
    }
}

pub fn read_text_file_request<H: TextPreviewHelper + MetadataHelper>(
    user_context: &UserContext,
    request: ReadTextFileRequest,
    config: &AppConfig,
    policy_engine: &PolicyEngine<'_>,
    helper_client: &H,
) -> Result<TextFileContentPayload, TealDriveError> {
    request.validate(&config.limits)?;
    let relative_path = validate_relative_path(&request.relative_path)?;
    let root = require_allowed_root(config.allowed_roots(), &request.root_id)?;
    match policy_engine.check_operation(
        FileOperation::ReadTextFile,
        &request.root_id,
        &relative_path,
        None,
        None,
        root.hidden_files_allowed,
    )? {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(reason) => {
            let _audit = PolicyAuditMetadata {
                action: FileOperation::ReadTextFile,
                root_id: request.root_id.clone(),
                relative_path: relative_path.clone(),
                target_relative_path: None,
                reason,
                safe_message: "Text preview denied by TealDrive policy.".to_owned(),
            };
            return Err(TealDriveError::PolicyDenied);
        }
    }

    let metadata = file_metadata_request(
        user_context,
        FileMetadataRequest {
            root_id: request.root_id.clone(),
            relative_path: relative_path.clone(),
        },
        config,
        policy_engine,
        helper_client,
    )?;
    if metadata.entry.file_type == FileKind::Directory {
        return Err(TealDriveError::InvalidTarget);
    }
    if metadata.entry.is_symlink || metadata.entry.is_sensitive {
        return Err(TealDriveError::PolicyDenied);
    }
    if metadata.entry.size > config.limits.max_text_edit_size as u64 {
        return Err(TealDriveError::FileTooLarge);
    }
    if request
        .max_bytes
        .is_some_and(|max_bytes| metadata.entry.size > max_bytes as u64)
    {
        return Err(TealDriveError::FileTooLarge);
    }

    let helper_request = HelperRequest {
        request_id: RequestId::new(),
        username: user_context.username.clone(),
        uid: user_context.uid,
        gid: user_context.gid,
        command: HelperCommand::ReadTextFile,
        root_id: request.root_id.clone(),
        relative_path: relative_path.clone(),
        target_relative_path: None,
        policy_context: HelperPolicyContext {
            read_only_root: root.read_only,
            is_web_root: root.is_web_root,
        },
        limits: HelperLimits {
            max_directory_page_size: config.limits.max_directory_page_size,
            max_text_edit_size: request
                .max_bytes
                .unwrap_or(config.limits.max_text_edit_size)
                .min(config.limits.max_text_edit_size),
            max_chunk_size: config.limits.max_chunk_size,
            max_download_helper_bytes: crate::download::transfer::MAX_DOWNLOAD_HELPER_BYTES,
        },
        list_options: None,
    };

    let response = TextPreviewHelper::execute_helper(helper_client, &helper_request)?;
    if !response.success {
        return Err(helper_response_to_error(&response));
    }
    let HelperResponsePayload::TextFileContent(content) = response.payload else {
        return Err(TealDriveError::HelperMalformedResponse);
    };

    Ok(TextFileContentPayload {
        root_id: request.root_id,
        relative_path,
        name: content.name,
        content: content.content,
        encoding: content.encoding,
        size: content.size,
        modified: content.modified,
        owner: content.owner,
        group: content.group,
        mode: content.mode,
        permissions: content.permissions,
        is_sensitive: content.is_sensitive,
        is_hidden: content.is_hidden,
        is_read_only: content.is_read_only,
        truncated: content.truncated,
        line_count: content.line_count,
        language_hint: content.language_hint,
    })
}

fn helper_response_to_error(response: &HelperResponse) -> TealDriveError {
    match response.error_kind {
        Some(ErrorKind::PermissionDenied) => TealDriveError::PermissionDenied,
        Some(ErrorKind::PolicyDenied) => TealDriveError::PolicyDenied,
        Some(ErrorKind::NotFound) => TealDriveError::NotFound,
        Some(ErrorKind::InvalidPath) => TealDriveError::InvalidPath,
        Some(ErrorKind::InvalidTarget) => TealDriveError::InvalidTarget,
        Some(ErrorKind::FileTooLarge) => TealDriveError::FileTooLarge,
        Some(ErrorKind::UnsupportedBinaryFile) => TealDriveError::UnsupportedBinaryFile,
        Some(ErrorKind::InvalidTextEncoding) => TealDriveError::InvalidTextEncoding,
        _ => TealDriveError::HelperCommandNotImplemented,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AllowedRoot, RelativePath, RootId};
    use crate::helper::metadata::file_metadata_with_roots;
    use crate::helper::text::read_text_file_with_roots;
    use crate::protocol::codec::{decode_msgpack, encode_msgpack};
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    struct InProcessHelper {
        roots: Vec<AllowedRoot>,
    }

    impl TextPreviewHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(read_text_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
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
            std::env::temp_dir().join(format!("tealdrive-service-text-{}", uuid::Uuid::new_v4()));
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

    fn request(path: &str) -> ReadTextFileRequest {
        ReadTextFileRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(path),
            max_bytes: None,
            encoding: None,
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
    fn normal_text_file_preview_succeeds() {
        let base = temp_root();
        fs::write(base.join("notes.txt"), b"hello\nworld\n").expect("file");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let payload =
            read_text_file_request(&user(), request("notes.txt"), &config, &engine, &helper)
                .expect("preview");

        assert_eq!(payload.content, "hello\nworld\n");
        assert_eq!(payload.encoding, "utf-8");
        assert_eq!(payload.line_count, Some(2));
    }

    #[test]
    fn directory_preview_rejected() {
        let base = temp_root();
        fs::create_dir(base.join("folder")).expect("folder");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            read_text_file_request(&user(), request("folder"), &config, &engine, &helper),
            Err(TealDriveError::InvalidTarget)
        );
    }

    #[test]
    fn missing_preview_maps_not_found() {
        let config = config_for(temp_root(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            read_text_file_request(&user(), request("missing.txt"), &config, &engine, &helper),
            Err(TealDriveError::NotFound)
        );
    }

    #[test]
    fn sensitive_text_preview_denied_and_content_not_returned() {
        let base = temp_root();
        fs::write(base.join(".env"), b"SECRET=1").expect("env");
        let config = config_for(base.clone(), false, true);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let error = read_text_file_request(&user(), request(".env"), &config, &engine, &helper)
            .expect_err("denied");
        let serialized = format!("{error:?}");

        assert_eq!(error, TealDriveError::PolicyDenied);
        assert!(!serialized.contains("SECRET=1"));
        assert!(!serialized.contains(base.to_string_lossy().as_ref()));
    }

    #[test]
    fn hidden_file_policy_respected() {
        let base = temp_root();
        fs::write(base.join(".hidden"), b"hidden").expect("hidden");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            read_text_file_request(&user(), request(".hidden"), &config, &engine, &helper),
            Err(TealDriveError::PolicyDenied)
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_preview_denied() {
        let base = temp_root();
        fs::write(base.join("target"), b"hello").expect("target");
        symlink(base.join("target"), base.join("link")).expect("symlink");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            read_text_file_request(&user(), request("link"), &config, &engine, &helper),
            Err(TealDriveError::PolicyDenied)
        );
    }

    #[test]
    fn read_only_root_allows_preview() {
        let base = temp_root();
        fs::write(base.join("notes.txt"), b"hello").expect("file");
        let config = config_for(base, true, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert!(
            read_text_file_request(&user(), request("notes.txt"), &config, &engine, &helper)
                .is_ok()
        );
    }

    #[test]
    fn file_over_size_limit_rejected() {
        let base = temp_root();
        fs::write(base.join("notes.txt"), b"12345").expect("file");
        let mut config = config_for(base, false, false);
        config.limits.max_text_edit_size = 4;
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            read_text_file_request(&user(), request("notes.txt"), &config, &engine, &helper),
            Err(TealDriveError::FileTooLarge)
        );
    }

    #[test]
    fn binary_file_rejected() {
        let base = temp_root();
        fs::write(base.join("binary.bin"), b"a\0b").expect("file");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            read_text_file_request(&user(), request("binary.bin"), &config, &engine, &helper),
            Err(TealDriveError::UnsupportedBinaryFile)
        );
    }

    #[test]
    fn invalid_utf8_rejected() {
        let base = temp_root();
        fs::write(base.join("bad.txt"), [0xff, 0xfe]).expect("file");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            read_text_file_request(&user(), request("bad.txt"), &config, &engine, &helper),
            Err(TealDriveError::InvalidTextEncoding)
        );
    }

    #[test]
    fn traversal_rejected_before_helper() {
        let config = config_for(temp_root(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            read_text_file_request(&user(), request("../secret"), &config, &engine, &helper),
            Err(TealDriveError::TraversalRejected)
        );
    }

    #[test]
    fn read_text_request_msgpack_roundtrip() {
        let request = ReadTextFileRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new("notes.txt"),
            max_bytes: Some(1024),
            encoding: Some("utf-8".to_owned()),
        };
        let bytes = encode_msgpack(&request).expect("encoded");
        let decoded: ReadTextFileRequest = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, request);
    }

    #[test]
    fn text_file_content_msgpack_roundtrip() {
        let base = temp_root();
        fs::write(base.join("notes.txt"), b"hello").expect("file");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let payload =
            read_text_file_request(&user(), request("notes.txt"), &config, &engine, &helper)
                .expect("preview");
        let bytes = encode_msgpack(&payload).expect("encoded");
        let decoded: TextFileContentPayload = decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, payload);
    }
}
