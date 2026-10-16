use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use sha2::{Digest, Sha256};

use crate::config::{AppConfig, RelativePath, RootId, UserContext};
use crate::errors::{ErrorKind, TealDriveError};
use crate::fs::filename::SafeFileName;
use crate::fs::path::{authorize_new_target as resolve_new_target, validate_relative_path};
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

// Keep system metadata/control writes possible while staging user uploads.
static STAGING_IO: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[derive(Debug, Default)]
struct StageUsage {
    bytes: u64,
    transfers: u32,
}
static STAGING_RESERVED: std::sync::LazyLock<std::sync::Arc<std::sync::Mutex<StageUsage>>> =
    std::sync::LazyLock::new(|| std::sync::Arc::new(std::sync::Mutex::new(StageUsage::default())));
#[derive(Debug)]
pub(crate) struct StageReservation {
    bytes: u64,
    counter: std::sync::Arc<std::sync::Mutex<StageUsage>>,
    _disk: Option<crate::upload::budget::DiskReservation>,
}
impl PartialEq for StageReservation {
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes && std::sync::Arc::ptr_eq(&self.counter, &other.counter)
    }
}
impl Eq for StageReservation {}
impl Drop for StageReservation {
    fn drop(&mut self) {
        let mut value = self.counter.lock().unwrap_or_else(|e| e.into_inner());
        value.bytes = value.bytes.saturating_sub(self.bytes);
        value.transfers = value.transfers.saturating_sub(1);
    }
}
fn reserve_staging(
    counter: std::sync::Arc<std::sync::Mutex<StageUsage>>,
    bytes: u64,
    maximum: u64,
    max_transfers: u32,
) -> Result<std::sync::Arc<StageReservation>, TealDriveError> {
    {
        let mut held = counter.lock().map_err(|_| TealDriveError::Internal)?;
        let total = held
            .bytes
            .checked_add(bytes)
            .ok_or(TealDriveError::DiskFull)?;
        if total > maximum || held.transfers >= max_transfers {
            return Err(TealDriveError::DiskFull);
        }
        held.bytes = total;
        held.transfers += 1;
    }
    Ok(std::sync::Arc::new(StageReservation {
        bytes,
        counter,
        _disk: None,
    }))
}

fn check_staging_capacity(
    file: &fs::File,
    bytes: u64,
    minimum_free: u64,
) -> Result<(), TealDriveError> {
    use std::os::fd::AsRawFd;
    let mut status: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstatvfs(file.as_raw_fd(), &mut status) } != 0 {
        return Err(TealDriveError::Internal);
    }
    let free = status.f_bavail.saturating_mul(status.f_frsize);
    if bytes
        .checked_add(minimum_free)
        .is_none_or(|required| free < required)
    {
        return Err(TealDriveError::DiskFull);
    }
    Ok(())
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadState {
    pub transfer_id: TransferId,
    pub bytes_received: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveUpload {
    pub retain_on_disconnect: bool,
    _staging_reservation: std::sync::Arc<StageReservation>,
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
    /// Production passes an already-open read-only descriptor; small in-process
    /// adapters retain the old interface without granting a filesystem path.
    fn execute_upload_source(
        &self,
        request: &HelperRequest,
        source: &mut fs::File,
    ) -> Result<HelperResponse, TealDriveError> {
        let size = request
            .write_options
            .as_ref()
            .and_then(|v| v.stream_size)
            .ok_or(TealDriveError::Validation)?;
        if size > 3 * crate::limits::MIB as u64 {
            return Err(TealDriveError::HelperRequestTooLarge);
        }
        let mut request = request.clone();
        let options = request
            .write_options
            .as_mut()
            .ok_or(TealDriveError::Validation)?;
        source
            .seek(SeekFrom::Start(0))
            .map_err(|_| TealDriveError::Internal)?;
        source
            .take(size)
            .read_to_end(&mut options.data)
            .map_err(|_| TealDriveError::Internal)?;
        if options.data.len() as u64 != size {
            return Err(TealDriveError::Validation);
        }
        options.stream_size = None;
        self.execute_helper(&request)
    }
}

impl UploadHelper for HelperProcessClient {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        self.execute(request)
    }
    fn execute_upload_source(
        &self,
        request: &HelperRequest,
        source: &mut fs::File,
    ) -> Result<HelperResponse, TealDriveError> {
        self.execute_upload_stream(request, source)
    }
}

pub(crate) fn validate_upload(
    user_context: &UserContext,
    request: &UploadBeginRequest,
    config: &AppConfig,
    policy_engine: &PolicyEngine<'_>,
) -> Result<(RelativePath, SafeFileName), TealDriveError> {
    let policy_engine = &policy_engine.for_user(&user_context.username);
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

    let _target = resolve_new_target(root, &parent_rel, &filename.0)?;

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

    if root
        .quota_bytes
        .is_some_and(|limit| request.total_size > limit)
    {
        return Err(TealDriveError::DiskFull);
    }
    Ok((parent_rel, filename))
}
pub fn upload_begin_request(
    user_context: &UserContext,
    request: UploadBeginRequest,
    config: &AppConfig,
    policy_engine: &PolicyEngine<'_>,
) -> Result<(ActiveUpload, UploadBeginResponse), TealDriveError> {
    let (parent_rel, filename) = validate_upload(user_context, &request, config, policy_engine)?;
    let transfer_id = TransferId::new();
    let temp_name = TempUploadName::new(&transfer_id);
    let negotiated_chunk_size = request.chunk_size.min(config.limits.max_chunk_size).max(1);
    let staging_path = std::env::temp_dir().join(temp_name.as_str());

    let permit = reserve_staging(
        STAGING_RESERVED.clone(),
        request.total_size,
        config.system_storage.max_staging_bytes,
        config.system_storage.max_staging_transfers,
    )?;
    let disk = crate::upload::budget::reserve(
        &staging_path,
        request.total_size,
        config.system_storage.max_staging_bytes,
        config.system_storage.max_staging_transfers,
    )?;
    let mut permit = permit;
    std::sync::Arc::get_mut(&mut permit)
        .ok_or(TealDriveError::Internal)?
        ._disk = Some(disk);
    let staging = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&staging_path)
        .map_err(|_| TealDriveError::Internal)?;
    if let Err(error) = check_staging_capacity(
        &staging,
        request.total_size,
        config.system_storage.staging_min_free_bytes,
    ) {
        drop(staging);
        let _ = fs::remove_file(&staging_path);
        return Err(error);
    }

    let active_upload = ActiveUpload {
        retain_on_disconnect: request.retain_on_disconnect,
        _staging_reservation: permit,
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
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&active_upload.staging_path)
        .map_err(|_| TealDriveError::Internal)?;

    let _disk_lock = STAGING_IO.lock().map_err(|_| TealDriveError::Internal)?;
    check_staging_capacity(
        &file,
        chunk.chunk_bytes.len() as u64,
        config.system_storage.staging_min_free_bytes,
    )?;
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
    struct Cleanup<'a>(&'a std::path::Path);
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = fs::remove_file(self.0);
        }
    }
    let _cleanup = Cleanup(&active_upload.staging_path);
    if request.transfer_id != active_upload.transfer_id {
        return Err(TealDriveError::Validation);
    }

    if active_upload.bytes_received != active_upload.total_size
        || request.total_size != active_upload.total_size
    {
        return Err(TealDriveError::Validation);
    }

    let mut source = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&active_upload.staging_path)
        .map_err(|_| TealDriveError::Internal)?;
    if source
        .metadata()
        .map_err(|_| TealDriveError::Internal)?
        .len()
        != active_upload.total_size
    {
        return Err(TealDriveError::Validation);
    }

    if let (Some(begin), Some(end)) = (&active_upload.sha256, &request.sha256) {
        if !begin.eq_ignore_ascii_case(end) {
            return Err(TealDriveError::Validation);
        }
    }
    let sha256_verified =
        if let Some(expected_hash) = active_upload.sha256.as_ref().or(request.sha256.as_ref()) {
            let mut hasher = Sha256::new();
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let count = source
                    .read(&mut buffer)
                    .map_err(|_| TealDriveError::Internal)?;
                if count == 0 {
                    break;
                }
                hasher.update(&buffer[..count]);
            }
            source
                .seek(SeekFrom::Start(0))
                .map_err(|_| TealDriveError::Internal)?;
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
        command: HelperCommand::UploadCommit,
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
            stream_size: Some(active_upload.total_size),
            expected_revision: None,
            data: Vec::new(),
            offset: 0,
            truncate: true,
        }),
        chmod_mode: None,
        archive_source_paths: None,
        search_options: None,
    };

    let response = helper_client.execute_upload_source(&helper_request, &mut source)?;
    let _ = fs::remove_file(&active_upload.staging_path);

    if !response.success {
        return Err(helper_response_to_error(&response));
    }
    if response.bytes_written != active_upload.total_size {
        return Err(TealDriveError::HelperMalformedResponse);
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
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(write_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    fn temp_root() -> PathBuf {
        let path = crate::test_support::scratch_dir()
            .join(format!("tealdrive-upload-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temp root");
        path
    }

    fn config_for(
        path: PathBuf,
        read_only: bool,
        uploads_allowed: bool,
        is_web_root: bool,
    ) -> AppConfig {
        AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: path,
                read_only,
                uploads_allowed,
                hidden_files_allowed: false,
                is_web_root,
                quota_bytes: None,
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

    fn begin_req(
        filename: &str,
        total_size: u64,
        chunk_size: usize,
        sha256: Option<String>,
    ) -> UploadBeginRequest {
        UploadBeginRequest {
            retain_on_disconnect: false,
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(""),
            filename: filename.to_owned(),
            total_size,
            chunk_size,
            sha256,
        }
    }

    #[test]
    fn staging_budget_tracks_last_clone_and_bounds_zero_byte_transfers() {
        let counter = std::sync::Arc::new(std::sync::Mutex::new(StageUsage::default()));
        let first = reserve_staging(counter.clone(), 8, 10, 2).unwrap();
        let clone = first.clone();
        drop(first);
        assert!(reserve_staging(counter.clone(), 3, 10, 2).is_err());
        let zero = reserve_staging(counter.clone(), 0, 10, 2).unwrap();
        assert!(reserve_staging(counter.clone(), 0, 10, 2).is_err());
        drop(clone);
        drop(zero);
        let permit = reserve_staging(counter.clone(), 10, 10, 2).unwrap();
        assert_eq!(counter.lock().unwrap().bytes, 10);
        drop(permit);
        assert_eq!(counter.lock().unwrap().bytes, 0);
        assert_eq!(counter.lock().unwrap().transfers, 0);
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
        let (mut active, begin_resp) =
            upload_begin_request(&user(), req, &config, &engine).expect("begin ok");
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&active.staging_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600,
            "staged user content must be private independently of the service umask"
        );
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
        let end_resp =
            upload_end_request(&user(), &active, end_req, &config, &helper).expect("end ok");
        assert_eq!(end_resp.bytes_written, data.len() as u64);
        assert_eq!(end_resp.sha256_verified, Some(true));

        assert_eq!(
            fs::read(base.join("hello.txt")).expect("read uploaded"),
            data
        );
    }

    #[test]
    fn begin_hash_cannot_be_bypassed_and_end_failures_remove_staging() {
        let base = temp_root();
        let config = config_for(base.clone(), false, true, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        for (name, begin_hash, total_size) in
            [("bad-hash", Some("0".repeat(64)), 3), ("bad-size", None, 4)]
        {
            let (mut active, _) =
                upload_begin_request(&user(), begin_req(name, 3, 3, begin_hash), &config, &engine)
                    .unwrap();
            let transfer_id = active.transfer_id;
            upload_chunk_request(
                &mut active,
                RawChunk {
                    transfer_id,
                    chunk_index: 0,
                    offset: 0,
                    chunk_bytes: b"abc".to_vec(),
                },
                &config,
            )
            .unwrap();
            assert!(upload_end_request(
                &user(),
                &active,
                UploadEndRequest {
                    transfer_id: active.transfer_id,
                    total_size,
                    sha256: None
                },
                &config,
                &helper
            )
            .is_err());
            assert!(!active.staging_path.exists());
            assert!(!base.join(name).exists());
        }
        fs::remove_dir_all(base).unwrap();
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
        let req = begin_req(
            "bad_hash.txt",
            data.len() as u64,
            100,
            Some("0000000000000000".to_owned()),
        );
        let (mut active, _) =
            upload_begin_request(&user(), req, &config, &engine).expect("begin ok");

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
        let err = upload_end_request(&user(), &active, end_req, &config, &helper)
            .expect_err("hash mismatch");
        assert_eq!(err, TealDriveError::Validation);
        assert!(!base.join("bad_hash.txt").exists());
    }

    #[test]
    fn upload_chunk_offset_mismatch_rejected() {
        let base = temp_root();
        let config = config_for(base, false, true, false);
        let engine = PolicyEngine::new(&config);

        let req = begin_req("test.txt", 100, 50, None);
        let (mut active, _) =
            upload_begin_request(&user(), req, &config, &engine).expect("begin ok");

        let chunk = RawChunk {
            transfer_id: active.transfer_id,
            chunk_index: 0,
            offset: 10, // should be 0
            chunk_bytes: b"12345".to_vec(),
        };
        assert_eq!(
            upload_chunk_request(&mut active, chunk, &config),
            Err(TealDriveError::Validation)
        );
    }

    #[test]
    fn upload_target_already_exists_rejected() {
        let base = temp_root();
        fs::write(base.join("existing.txt"), b"already here").expect("write existing");
        let config = config_for(base, false, true, false);
        let engine = PolicyEngine::new(&config);

        let req = begin_req("existing.txt", 50, 50, None);
        let (mut active, _) = upload_begin_request(&user(), req, &config, &engine).unwrap();
        let id = active.transfer_id;
        upload_chunk_request(
            &mut active,
            RawChunk {
                transfer_id: id,
                chunk_index: 0,
                offset: 0,
                chunk_bytes: vec![0; 50],
            },
            &config,
        )
        .unwrap();
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        assert_eq!(
            upload_end_request(
                &user(),
                &active,
                UploadEndRequest {
                    transfer_id: active.transfer_id,
                    total_size: 50,
                    sha256: None
                },
                &config,
                &helper
            ),
            Err(TealDriveError::AlreadyExists)
        );
    }

    #[test]
    fn upload_to_read_only_root_rejected() {
        let base = temp_root();
        let config = config_for(base, true, true, false);
        let engine = PolicyEngine::new(&config);

        let req = begin_req("test.txt", 50, 50, None);
        assert_eq!(
            upload_begin_request(&user(), req, &config, &engine),
            Err(TealDriveError::PolicyDenied)
        );
    }

    #[test]
    fn upload_to_disallowed_uploads_root_rejected() {
        let base = temp_root();
        let config = config_for(base, false, false, false);
        let engine = PolicyEngine::new(&config);

        let req = begin_req("test.txt", 50, 50, None);
        assert_eq!(
            upload_begin_request(&user(), req, &config, &engine),
            Err(TealDriveError::PolicyDenied)
        );
    }

    #[test]
    fn upload_webroot_executable_rejected() {
        let base = temp_root();
        let config = config_for(base, false, true, true);
        let engine = PolicyEngine::new(&config);

        let req = begin_req("bad.php", 50, 50, None);
        assert_eq!(
            upload_begin_request(&user(), req, &config, &engine),
            Err(TealDriveError::WebrootExecutableDenied)
        );
    }

    #[test]
    fn upload_sensitive_file_rejected() {
        let base = temp_root();
        let config = config_for(base, false, true, false);
        let engine = PolicyEngine::new(&config);

        let req = begin_req(".env", 50, 50, None);
        assert_eq!(
            upload_begin_request(&user(), req, &config, &engine),
            Err(TealDriveError::PolicyDenied)
        );
    }

    #[test]
    fn upload_exceeding_quota_rejected() {
        let base = temp_root();
        std::fs::write(base.join("existing.dat"), vec![0u8; 80]).unwrap();
        let mut config = config_for(base, false, true, false);
        config.roots[0].quota_bytes = Some(100);
        let engine = PolicyEngine::new(&config);

        // 80 existing + 30 new > 100 quota -> DiskFull
        let req = begin_req("toolarge.txt", 30, 30, None);
        let (mut active, _) = upload_begin_request(&user(), req, &config, &engine).unwrap();
        let id = active.transfer_id;
        upload_chunk_request(
            &mut active,
            RawChunk {
                transfer_id: id,
                chunk_index: 0,
                offset: 0,
                chunk_bytes: vec![0; 30],
            },
            &config,
        )
        .unwrap();
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        assert_eq!(
            upload_end_request(
                &user(),
                &active,
                UploadEndRequest {
                    transfer_id: active.transfer_id,
                    total_size: 30,
                    sha256: None
                },
                &config,
                &helper
            ),
            Err(TealDriveError::DiskFull)
        );

        // 80 existing + 15 new <= 100 quota -> Allowed to begin
        let req_ok = begin_req("fits.txt", 15, 15, None);
        assert!(upload_begin_request(&user(), req_ok, &config, &engine).is_ok());
    }
}
