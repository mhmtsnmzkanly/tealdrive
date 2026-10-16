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
use crate::protocol::chunk::RawChunk;
use crate::protocol::codec::encode_msgpack;
use crate::protocol::frame::{RequestId, TdrvFrame, TransferId};
use crate::protocol::header::{Encoding, TdrvHeader};
use crate::protocol::message_type::MessageType;
use crate::protocol::payload::{DownloadBeginRequest, DownloadEndPayload, FileMetadataRequest};
use crate::protocol::schema::FileKind;

pub const MAX_DOWNLOAD_HELPER_BYTES: usize = 768 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadState {
    pub transfer_id: TransferId,
    pub bytes_sent: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadFrameSet {
    pub transfer_id: TransferId,
    pub frames: Vec<TdrvFrame>,
    pub total_bytes: u64,
    pub chunk_count: u32,
}

pub trait DownloadHelper {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError>;
}

impl DownloadHelper for HelperProcessClient {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        self.execute(request)
    }
}

pub fn download_file_request<H: DownloadHelper + MetadataHelper>(
    user_context: &UserContext,
    request: DownloadBeginRequest,
    config: &AppConfig,
    policy_engine: &PolicyEngine<'_>,
    helper_client: &H,
) -> Result<DownloadFrameSet, TealDriveError> {
    let policy_engine = &policy_engine.for_user(&user_context.username);
    request.validate(&config.limits)?;
    let relative_path = validate_relative_path(&request.relative_path)?;
    policy_engine.ensure_visible(&request.root_id, &relative_path)?;
    let root = require_allowed_root(config.allowed_roots(), &request.root_id)?;
    match policy_engine.check_operation(
        FileOperation::DownloadFile,
        &request.root_id,
        &relative_path,
        None,
        None,
        root.hidden_files_allowed,
    )? {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(reason) => {
            let _audit = PolicyAuditMetadata {
                action: FileOperation::DownloadFile,
                root_id: request.root_id.clone(),
                relative_path: relative_path.clone(),
                target_relative_path: None,
                reason,
                safe_message: "Download denied by TealDrive policy.".to_owned(),
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
    if metadata.entry.is_symlink {
        return Err(TealDriveError::PolicyDenied);
    }
    if metadata.entry.is_sensitive {
        return Err(TealDriveError::PolicyDenied);
    }

    let chunk_size = request
        .requested_chunk_size
        .unwrap_or(config.limits.max_chunk_size)
        .min(config.limits.max_chunk_size)
        .max(1);
    let helper_request = HelperRequest {
        request_id: RequestId::new(),
        username: user_context.username.clone(),
        uid: user_context.uid,
        gid: user_context.gid,
        command: HelperCommand::DownloadFile,
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
            max_chunk_size: chunk_size,
            max_download_helper_bytes: MAX_DOWNLOAD_HELPER_BYTES,
        },
        list_options: None,
        write_options: None,
        chmod_mode: None,
        archive_source_paths: None,
        search_options: None,
    };

    let response = DownloadHelper::execute_helper(helper_client, &helper_request)?;
    if !response.success {
        return Err(helper_response_to_error(&response));
    }

    let HelperResponsePayload::DownloadFile(download) = response.payload else {
        return Err(TealDriveError::HelperMalformedResponse);
    };
    let mut frames = Vec::with_capacity(download.chunks.len() + 1);
    for chunk in &download.chunks {
        let raw = RawChunk {
            transfer_id: download.transfer_id,
            chunk_index: chunk.chunk_index,
            offset: chunk.offset,
            chunk_bytes: chunk.chunk_bytes.clone(),
        };
        let mut header = TdrvHeader::new(MessageType::DownloadChunk, RequestId::new(), 0);
        header.encoding = Encoding::RawBinary;
        frames.push(TdrvFrame::new(header, raw.encode()?)?);
    }
    let chunk_count = u32::try_from(download.chunks.len()).map_err(|_| TealDriveError::Internal)?;
    let end_payload = DownloadEndPayload {
        transfer_id: download.transfer_id,
        bytes_sent: download.total_bytes,
        chunk_count,
        sha256: None,
    };
    let mut end_header = TdrvHeader::new(MessageType::DownloadEnd, RequestId::new(), 0);
    end_header.encoding = Encoding::MessagePack;
    frames.push(TdrvFrame::new(end_header, encode_msgpack(&end_payload)?)?);

    Ok(DownloadFrameSet {
        transfer_id: download.transfer_id,
        frames,
        total_bytes: download.total_bytes,
        chunk_count,
    })
}

pub(crate) fn helper_response_to_error(response: &HelperResponse) -> TealDriveError {
    match response.error_kind {
        Some(ErrorKind::PermissionDenied) => TealDriveError::PermissionDenied,
        Some(ErrorKind::PolicyDenied) => TealDriveError::PolicyDenied,
        Some(ErrorKind::NotFound) => TealDriveError::NotFound,
        Some(ErrorKind::InvalidPath) => TealDriveError::InvalidPath,
        Some(ErrorKind::InvalidTarget) => TealDriveError::InvalidTarget,
        Some(ErrorKind::FileTooLarge) => TealDriveError::FileTooLarge,
        _ => TealDriveError::HelperCommandNotImplemented,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AllowedRoot, RelativePath, RootId};
    use crate::helper::download::download_file_with_roots;
    use crate::helper::metadata::file_metadata_with_roots;
    use crate::protocol::chunk::RawChunk;
    use crate::protocol::codec::decode_msgpack;
    use crate::protocol::payload::DownloadEndPayload;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    struct InProcessHelper {
        roots: Vec<AllowedRoot>,
    }

    impl DownloadHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(download_file_with_roots(
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
        let path = crate::test_support::scratch_dir().join(format!(
            "tealdrive-service-download-{}",
            uuid::Uuid::new_v4()
        ));
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
                quota_bytes: None,
            }],
            ..AppConfig::default()
        }
    }

    fn request(path: &str, chunk_size: Option<usize>) -> DownloadBeginRequest {
        DownloadBeginRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(path),
            requested_chunk_size: chunk_size,
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
    fn normal_file_download_produces_chunks() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"hello world").expect("file");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let result = download_file_request(
            &user(),
            request("file.txt", Some(5)),
            &config,
            &engine,
            &helper,
        )
        .expect("download");

        assert_eq!(result.total_bytes, 11);
        assert_eq!(result.chunk_count, 3);
        assert_eq!(
            result.frames[0].header.message_type,
            MessageType::DownloadChunk
        );
        let chunk = RawChunk::decode(&result.frames[0].payload).expect("chunk");
        assert_eq!(chunk.offset, 0);
        assert_eq!(chunk.transfer_id, result.transfer_id);
    }

    #[test]
    fn directory_download_rejected() {
        let base = temp_root();
        fs::create_dir(base.join("folder")).expect("folder");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            download_file_request(&user(), request("folder", None), &config, &engine, &helper),
            Err(TealDriveError::InvalidTarget)
        );
    }

    #[test]
    fn missing_file_maps_not_found() {
        let config = config_for(temp_root(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            download_file_request(&user(), request("missing", None), &config, &engine, &helper),
            Err(TealDriveError::NotFound)
        );
    }

    #[test]
    fn sensitive_file_download_denied() {
        let base = temp_root();
        fs::write(base.join(".env"), b"SECRET=1").expect("env");
        let config = config_for(base, false, true);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            download_file_request(&user(), request(".env", None), &config, &engine, &helper),
            Err(TealDriveError::PolicyDenied)
        );
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
            download_file_request(&user(), request(".hidden", None), &config, &engine, &helper),
            Err(TealDriveError::PolicyDenied)
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_download_denied() {
        let base = temp_root();
        fs::write(base.join("target"), b"hello").expect("target");
        symlink(base.join("target"), base.join("link")).expect("symlink");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            download_file_request(&user(), request("link", None), &config, &engine, &helper),
            Err(TealDriveError::PolicyDenied)
        );
    }

    #[test]
    fn read_only_root_allows_download() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"hello").expect("file");
        let config = config_for(base, true, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert!(download_file_request(
            &user(),
            request("file.txt", None),
            &config,
            &engine,
            &helper
        )
        .is_ok());
    }

    #[test]
    fn chunk_size_over_max_rejected() {
        let config = config_for(temp_root(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            download_file_request(
                &user(),
                request("file.txt", Some(config.limits.max_chunk_size + 1)),
                &config,
                &engine,
                &helper
            ),
            Err(TealDriveError::Validation)
        );
    }

    #[test]
    fn download_end_payload_roundtrip_and_offsets_correct() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"abcdef").expect("file");
        let config = config_for(base, false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let result = download_file_request(
            &user(),
            request("file.txt", Some(2)),
            &config,
            &engine,
            &helper,
        )
        .expect("download");
        let chunks: Vec<_> = result.frames[..result.frames.len() - 1]
            .iter()
            .map(|frame| RawChunk::decode(&frame.payload).expect("chunk"))
            .collect();
        let end: DownloadEndPayload =
            decode_msgpack(&result.frames.last().expect("end").payload).expect("end");

        assert_eq!(chunks[0].offset, 0);
        assert_eq!(chunks[1].offset, 2);
        assert!(chunks
            .iter()
            .all(|chunk| chunk.transfer_id == result.transfer_id));
        assert_eq!(end.transfer_id, result.transfer_id);
        assert_eq!(end.bytes_sent, 6);
        assert_eq!(end.chunk_count, 3);
    }

    #[test]
    fn traversal_rejected_before_helper() {
        let config = config_for(temp_root(), false, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        assert_eq!(
            download_file_request(
                &user(),
                request("../secret", None),
                &config,
                &engine,
                &helper
            ),
            Err(TealDriveError::TraversalRejected)
        );
    }
}
