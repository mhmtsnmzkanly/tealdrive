use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;

use sha2::{Digest, Sha256};

use crate::config::{AppConfig, RelativePath, RootId, UserContext};
use crate::errors::{ErrorKind, TealDriveError};
use crate::fs::filename::SafeFileName;
use crate::fs::path::{resolve_new_target, validate_relative_path};
use crate::helper::client::HelperProcessClient;
use crate::helper::types::{
    HelperCommand, HelperLimits, HelperPolicyContext, HelperRequest, HelperResponse,
    HelperWriteOptions,
};
use crate::policy::decision::{FileOperation, PolicyAuditMetadata, PolicyDecision, PolicyEngine};
use crate::policy::roots::require_allowed_root;
use crate::policy::sensitive::detect_sensitive;
use crate::policy::webroot::check_webroot_upload;
use crate::protocol::chunk::RawChunk;
use crate::protocol::frame::{RequestId, TransferId};
use crate::protocol::payload::{
    OperationDonePayload, OperationProgressPayload, UploadAbortRequest, UploadBeginRequest,
    UploadBeginResponse, UploadEndRequest, UploadEndResponse,
};
use crate::upload::temp::TempUploadName;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadState {
    pub transfer_id: TransferId,
    pub bytes_received: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveUpload {
    pub transfer_id: TransferId,
    pub root_id: RootId,
    pub parent_relative_path: RelativePath,
    pub filename: SafeFileName,
    pub total_size: u64,
    pub chunk_size: usize,
    pub sha256: Option<String>,
    pub bytes_received: u64,
    pub temp_name: TempUploadName,
    pub staging_path: PathBuf,
}

pub trait UploadHelper {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError>;
}

impl UploadHelper for HelperProcessClient {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        self.execute(request)
    }
}

pub fn upload_begin_request(
    _user_context: &UserContext,
    request: UploadBeginRequest,
    config: &AppConfig,
    policy_engine: &PolicyEngine<'_>,
) -> Result<(ActiveUpload, UploadBeginResponse), TealDriveError> {
    request.validate(&config.limits)?;

    if request.total_size > config.limits.max_in_flight_upload as u64 {
        return Err(TealDriveError::Validation);
    }

    let parent_rel = validate_relative_path(&request.relative_path)?;
    let filename = SafeFileName::parse(&request.filename)?;

    let target_rel = if parent_rel.0.is_empty() {
        RelativePath::new(filename.0.clone())
    } else {
        RelativePath::new(format!("{}/{}", parent_rel.0, filename.0))
    };

    let root = require_allowed_root(config.allowed_roots(), &request.root_id)?;

    if root.read_only || !root.uploads_allowed {
        return Err(TealDriveError::PolicyDenied);
    }

    if detect_sensitive(&target_rel, &config.sensitive_policy).is_sensitive {
        return Err(TealDriveError::PolicyDenied);
    }

    match check_webroot_upload(root.is_web_root, &filename, &config.webroot_policy) {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(_) => return Err(TealDriveError::WebrootExecutableDenied),
    }

    let target = resolve_new_target(root, &parent_rel, &filename.0)?;
    if fs::symlink_metadata(&target.resolved_path).is_ok() {
        return Err(TealDriveError::AlreadyExists);
    }

    match policy_engine.check_operation(
        FileOperation::UploadFile,
        &request.root_id,
        &parent_rel,
        Some(&target_rel),
        Some(&filename),
        root.hidden_files_allowed,
    )? {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(reason) => {
            let _audit = PolicyAuditMetadata {
                action: FileOperation::UploadFile,
                root_id: request.root_id.clone(),
                relative_path: parent_rel,
                target_relative_path: Some(target_rel),
                reason,
                safe_message: "Upload denied by TealDrive policy.".to_owned(),
            };
            return Err(TealDriveError::PolicyDenied);
        }
    }

    let transfer_id = TransferId::new();
    let temp_name = TempUploadName::new(&transfer_id);
    let negotiated_chunk_size = request
        .chunk_size
        .min(config.limits.max_chunk_size)
        .max(1);
    let staging_path = std::env::temp_dir().join(temp_name.as_str());

    fs::File::create(&staging_path).map_err(|_| TealDriveError::Internal)?;

    let active_upload = ActiveUpload {
        transfer_id,
        root_id: request.root_id,
        parent_relative_path: parent_rel,
        filename,
        total_size: request.total_size,
        chunk_size: negotiated_chunk_size,
        sha256: request.sha256,
        bytes_received: 0,
        temp_name,
        staging_path,
    };

    let response = UploadBeginResponse {
        transfer_id,
        chunk_size: negotiated_chunk_size,
    };

    Ok((active_upload, response))
}

pub fn upload_chunk_request(
    active_upload: &mut ActiveUpload,
    chunk: RawChunk,
    config: &AppConfig,
) -> Result<OperationProgressPayload, TealDriveError> {
    if chunk.transfer_id != active_upload.transfer_id {
        return Err(TealDriveError::Validation);
    }

    if chunk.chunk_bytes.len() > config.limits.max_chunk_size {
        return Err(TealDriveError::PayloadTooLarge);
    }

    if chunk.offset != active_upload.bytes_received {
        return Err(TealDriveError::Validation);
    }

    let next_bytes = active_upload.bytes_received + chunk.chunk_bytes.len() as u64;
    if next_bytes > active_upload.total_size {
        return Err(TealDriveError::PayloadTooLarge);
    }

    let mut file = OpenOptions::new()
        .write(true)
        .open(&active_upload.staging_path)
        .map_err(|_| TealDriveError::Internal)?;

    file.seek(SeekFrom::Start(chunk.offset))
        .map_err(|_| TealDriveError::Internal)?;
    file.write_all(&chunk.chunk_bytes)
        .map_err(|_| TealDriveError::Internal)?;
    file.flush().map_err(|_| TealDriveError::Internal)?;

    active_upload.bytes_received = next_bytes;

    Ok(OperationProgressPayload {
        request_id: RequestId::new(),
        operation: "UPLOAD".to_owned(),
        bytes_done: Some(active_upload.bytes_received),
        bytes_total: Some(active_upload.total_size),
        items_done: None,
        items_total: None,
    })
}

pub fn upload_end_request<H: UploadHelper>(
    user_context: &UserContext,
    active_upload: &ActiveUpload,
    request: UploadEndRequest,
    config: &AppConfig,
    helper_client: &H,
) -> Result<UploadEndResponse, TealDriveError> {
    if request.transfer_id != active_upload.transfer_id {
        return Err(TealDriveError::Validation);
    }

    if active_upload.bytes_received != active_upload.total_size
        || request.total_size != active_upload.total_size
    {
        return Err(TealDriveError::Validation);
    }

    let staged_bytes = fs::read(&active_upload.staging_path).map_err(|_| TealDriveError::Internal)?;

    let sha256_verified = if let Some(expected_hash) = &request.sha256 {
        let mut hasher = Sha256::new();
        hasher.update(&staged_bytes);
        let computed_hash = format!("{:x}", hasher.finalize());
        if !computed_hash.eq_ignore_ascii_case(expected_hash) {
            let _ = fs::remove_file(&active_upload.staging_path);
            return Err(TealDriveError::Validation);
        }
        Some(true)
    } else {
        None
    };

    let target_rel = if active_upload.parent_relative_path.0.is_empty() {
        RelativePath::new(active_upload.filename.0.clone())
    } else {
        RelativePath::new(format!(
            "{}/{}",
            active_upload.parent_relative_path.0, active_upload.filename.0
        ))
    };

    let root = require_allowed_root(config.allowed_roots(), &active_upload.root_id)?;
    let helper_request = HelperRequest {
        request_id: RequestId::new(),
        username: user_context.username.clone(),
        uid: user_context.uid,
        gid: user_context.gid,
        command: HelperCommand::WriteFile,
        root_id: active_upload.root_id.clone(),
        relative_path: target_rel,
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
        write_options: Some(HelperWriteOptions {
            data: staged_bytes,
            offset: 0,
            truncate: true,
        }),
        chmod_mode: None,
    };

    let response = helper_client.execute_helper(&helper_request)?;
    let _ = fs::remove_file(&active_upload.staging_path);

    if !response.success {
        return Err(helper_response_to_error(&response));
    }

    Ok(UploadEndResponse {
        transfer_id: active_upload.transfer_id,
        bytes_written: active_upload.bytes_received,
        sha256_verified,
    })
}

pub fn upload_abort_request(
    active_upload: &ActiveUpload,
    request: UploadAbortRequest,
) -> Result<OperationDonePayload, TealDriveError> {
    if request.transfer_id != active_upload.transfer_id {
        return Err(TealDriveError::Validation);
    }

    let _ = fs::remove_file(&active_upload.staging_path);

    Ok(OperationDonePayload {
        request_id: RequestId::new(),
        operation: "UPLOAD_ABORT".to_owned(),
        safe_message: Some("Upload aborted.".to_owned()),
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
    use crate::helper::upload::write_file_with_roots;

    struct InProcessHelper {
        roots: Vec<AllowedRoot>,
    }

    impl UploadHelper for InProcessHelper {
        fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
            Ok(write_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    fn temp_root() -> PathBuf {
        let path = std::env::temp_dir().join(format!("tealdrive-upload-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temp root");
        path
    }

    fn config_for(path: PathBuf, read_only: bool, uploads_allowed: bool, is_web_root: bool) -> AppConfig {
        AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: path,
                read_only,
                uploads_allowed,
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

    fn begin_req(filename: &str, total_size: u64, chunk_size: usize, sha256: Option<String>) -> UploadBeginRequest {
        UploadBeginRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(""),
            filename: filename.to_owned(),
            total_size,
            chunk_size,
            sha256,
        }
    }

    #[test]
    fn full_upload_lifecycle_success() {
        let base = temp_root();
        let config = config_for(base.clone(), false, true, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        let data = b"Hello, TealDrive Upload!";
        let mut hasher = Sha256::new();
        hasher.update(data);
        let sha256 = format!("{:x}", hasher.finalize());

        let req = begin_req("hello.txt", data.len() as u64, 10, Some(sha256.clone()));
        let (mut active, begin_resp) = upload_begin_request(&user(), req, &config, &engine).expect("begin ok");
        assert_eq!(begin_resp.chunk_size, 10);
        assert_eq!(begin_resp.transfer_id, active.transfer_id);

        let chunk1 = RawChunk {
            transfer_id: active.transfer_id,
            chunk_index: 0,
            offset: 0,
            chunk_bytes: data[..10].to_vec(),
        };
        let p1 = upload_chunk_request(&mut active, chunk1, &config).expect("chunk 1 ok");
        assert_eq!(p1.bytes_done, Some(10));

        let chunk2 = RawChunk {
            transfer_id: active.transfer_id,
            chunk_index: 1,
            offset: 10,
            chunk_bytes: data[10..].to_vec(),
        };
        let p2 = upload_chunk_request(&mut active, chunk2, &config).expect("chunk 2 ok");
        assert_eq!(p2.bytes_done, Some(data.len() as u64));

        let end_req = UploadEndRequest {
            transfer_id: active.transfer_id,
            total_size: data.len() as u64,
            sha256: Some(sha256),
        };
        let end_resp = upload_end_request(&user(), &active, end_req, &config, &helper).expect("end ok");
        assert_eq!(end_resp.bytes_written, data.len() as u64);
        assert_eq!(end_resp.sha256_verified, Some(true));

        assert_eq!(fs::read(base.join("hello.txt")).expect("read uploaded"), data);
    }

    #[test]
    fn upload_abort_cleans_up() {
        let base = temp_root();
        let config = config_for(base.clone(), false, true, false);
        let engine = PolicyEngine::new(&config);

        let req = begin_req("abort.txt", 100, 50, None);
        let (active, _) = upload_begin_request(&user(), req, &config, &engine).expect("begin ok");
        assert!(active.staging_path.exists());

        let abort_req = UploadAbortRequest {
            transfer_id: active.transfer_id,
            reason: Some("user cancelled".to_owned()),
        };
        let abort_resp = upload_abort_request(&active, abort_req).expect("abort ok");
        assert_eq!(abort_resp.operation, "UPLOAD_ABORT");
        assert!(!active.staging_path.exists());
        assert!(!base.join("abort.txt").exists());
    }

    #[test]
    fn upload_sha256_mismatch_rejected() {
        let base = temp_root();
        let config = config_for(base.clone(), false, true, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        let data = b"some data";
        let req = begin_req("bad_hash.txt", data.len() as u64, 100, Some("0000000000000000".to_owned()));
        let (mut active, _) = upload_begin_request(&user(), req, &config, &engine).expect("begin ok");

        let chunk = RawChunk {
            transfer_id: active.transfer_id,
            chunk_index: 0,
            offset: 0,
            chunk_bytes: data.to_vec(),
        };
        upload_chunk_request(&mut active, chunk, &config).expect("chunk ok");

        let end_req = UploadEndRequest {
            transfer_id: active.transfer_id,
            total_size: data.len() as u64,
            sha256: Some("0000000000000000".to_owned()),
        };
        let err = upload_end_request(&user(), &active, end_req, &config, &helper).expect_err("hash mismatch");
        assert_eq!(err, TealDriveError::Validation);
        assert!(!base.join("bad_hash.txt").exists());
    }

    #[test]
    fn upload_chunk_offset_mismatch_rejected() {
        let base = temp_root();
        let config = config_for(base, false, true, false);
        let engine = PolicyEngine::new(&config);

        let req = begin_req("test.txt", 100, 50, None);
        let (mut active, _) = upload_begin_request(&user(), req, &config, &engine).expect("begin ok");

        let chunk = RawChunk {
            transfer_id: active.transfer_id,
            chunk_index: 0,
            offset: 10, // should be 0
            chunk_bytes: b"12345".to_vec(),
        };
        assert_eq!(upload_chunk_request(&mut active, chunk, &config), Err(TealDriveError::Validation));
    }

    #[test]
    fn upload_target_already_exists_rejected() {
        let base = temp_root();
        fs::write(base.join("existing.txt"), b"already here").expect("write existing");
        let config = config_for(base, false, true, false);
        let engine = PolicyEngine::new(&config);

        let req = begin_req("existing.txt", 50, 50, None);
        assert_eq!(upload_begin_request(&user(), req, &config, &engine), Err(TealDriveError::AlreadyExists));
    }

    #[test]
    fn upload_to_read_only_root_rejected() {
        let base = temp_root();
        let config = config_for(base, true, true, false);
        let engine = PolicyEngine::new(&config);

        let req = begin_req("test.txt", 50, 50, None);
        assert_eq!(upload_begin_request(&user(), req, &config, &engine), Err(TealDriveError::PolicyDenied));
    }

    #[test]
    fn upload_to_disallowed_uploads_root_rejected() {
        let base = temp_root();
        let config = config_for(base, false, false, false);
        let engine = PolicyEngine::new(&config);

        let req = begin_req("test.txt", 50, 50, None);
        assert_eq!(upload_begin_request(&user(), req, &config, &engine), Err(TealDriveError::PolicyDenied));
    }

    #[test]
    fn upload_webroot_executable_rejected() {
        let base = temp_root();
        let config = config_for(base, false, true, true);
        let engine = PolicyEngine::new(&config);

        let req = begin_req("bad.php", 50, 50, None);
        assert_eq!(upload_begin_request(&user(), req, &config, &engine), Err(TealDriveError::WebrootExecutableDenied));
    }

    #[test]
    fn upload_sensitive_file_rejected() {
        let base = temp_root();
        let config = config_for(base, false, true, false);
        let engine = PolicyEngine::new(&config);

        let req = begin_req(".env", 50, 50, None);
        assert_eq!(upload_begin_request(&user(), req, &config, &engine), Err(TealDriveError::PolicyDenied));
    }
}
