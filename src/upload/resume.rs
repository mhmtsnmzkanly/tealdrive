//! Short-lived reconnect recovery, bound to the authenticated browser session.
//! Server restart deliberately does not restore authority from service-writable files.
use super::transfer::ActiveUpload;
use crate::{
    config::AppConfig,
    errors::TealDriveError,
    protocol::{
        frame::TransferId,
        payload::{UploadBeginRequest, UploadResumeResponse},
    },
    ws::connection::WsConnectionContext,
};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    sync::{LazyLock, Mutex},
};
struct Suspended {
    session: crate::session::SessionId,
    username: String,
    uid: u32,
    gid: u32,
    expires: u64,
    upload: Option<ActiveUpload>,
}
impl Drop for Suspended {
    fn drop(&mut self) {
        if let Some(upload) = &self.upload {
            let _ = std::fs::remove_file(&upload.staging_path);
        }
    }
}
static PENDING: LazyLock<Mutex<HashMap<TransferId, Suspended>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
fn now() -> u64 {
    crate::session::store::InMemorySessionStore::now_epoch_seconds()
}
pub(crate) fn suspend(context: &WsConnectionContext, upload: ActiveUpload, seconds: u64) {
    let id = upload.transfer_id;
    let record = Suspended {
        session: context.session_id,
        username: context.username.clone(),
        uid: context.uid,
        gid: context.gid,
        expires: now().saturating_add(seconds),
        upload: Some(upload),
    };
    if let Ok(mut pending) = PENDING.lock() {
        pending.retain(|_, v| v.expires > now());
        if pending.len() < 4096 {
            pending.insert(id, record);
        }
    }
}
pub(crate) fn purge(session: Option<crate::session::SessionId>) {
    if let Ok(mut pending) = PENDING.lock() {
        pending.retain(|_, r| r.expires > now() && session != Some(r.session));
    }
}
pub(crate) fn clear() {
    if let Ok(mut pending) = PENDING.lock() {
        pending.clear();
    }
}
pub(crate) fn take(
    context: &WsConnectionContext,
    id: TransferId,
    config: &AppConfig,
) -> Result<(ActiveUpload, UploadResumeResponse), TealDriveError> {
    if config.system_storage.upload_resume_seconds == 0 {
        return Err(TealDriveError::PermissionDenied);
    }
    let mut record = {
        let mut pending = PENDING.lock().map_err(|_| TealDriveError::Internal)?;
        pending.retain(|_, v| v.expires > now());
        let record = pending.get(&id).ok_or(TealDriveError::NotFound)?;
        if record.session != context.session_id
            || record.username != context.username
            || record.uid != context.uid
            || record.gid != context.gid
        {
            return Err(TealDriveError::NotFound);
        }
        pending.remove(&id).ok_or(TealDriveError::NotFound)?
    };
    let upload = record.upload.as_ref().ok_or(TealDriveError::NotFound)?;
    let request = UploadBeginRequest {
        retain_on_disconnect: true,
        root_id: upload.root_id.clone(),
        relative_path: upload.parent_relative_path.clone(),
        filename: upload.filename.0.clone(),
        total_size: upload.total_size,
        chunk_size: upload.chunk_size,
        sha256: upload.sha256.clone(),
    };
    super::transfer::validate_upload(
        &context.user_context(),
        &request,
        config,
        &crate::policy::decision::PolicyEngine::new(config),
    )?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&upload.staging_path)
        .map_err(|_| TealDriveError::NotFound)?;
    let metadata = file.metadata().map_err(|_| TealDriveError::Internal)?;
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
        || metadata.nlink() != 1
        || metadata.len() != upload.bytes_received
    {
        return Err(TealDriveError::PermissionDenied);
    }
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|_| TealDriveError::Internal)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    let response = UploadResumeResponse {
        transfer_id: id,
        root_id: upload.root_id.clone(),
        relative_path: upload.parent_relative_path.clone(),
        filename: upload.filename.0.clone(),
        total_size: upload.total_size,
        bytes_received: upload.bytes_received,
        chunk_size: upload.chunk_size,
        prefix_sha256: format!("{:x}", digest.finalize()),
    };
    Ok((
        record.upload.take().ok_or(TealDriveError::NotFound)?,
        response,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{RelativePath, RootId},
        protocol::chunk::RawChunk,
        session::SessionId,
    };
    fn fixture() -> (
        std::path::PathBuf,
        AppConfig,
        WsConnectionContext,
        ActiveUpload,
    ) {
        let path = std::env::temp_dir().join(format!("resume-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        let mut cfg = AppConfig::default();
        cfg.roots[0].root_id = RootId::new("home");
        cfg.roots[0].base_path = path.clone();
        cfg.roots[0].uploads_allowed = true;
        let context = WsConnectionContext {
            session_id: SessionId(uuid::Uuid::new_v4()),
            username: "alice".into(),
            uid: 1000,
            gid: 1000,
            user_agent: None,
            ip: None,
            connected_at: 0,
            protocol_ready: true,
            selected_protocol: None,
            features: cfg.features.clone(),
            active_uploads: HashMap::new(),
        };
        let (mut upload, _) = super::super::transfer::upload_begin_request(
            &context.user_context(),
            UploadBeginRequest {
                retain_on_disconnect: true,
                root_id: RootId::new("home"),
                relative_path: RelativePath::new(""),
                filename: "resumed.txt".into(),
                total_size: 10,
                chunk_size: 4,
                sha256: None,
            },
            &cfg,
            &crate::policy::decision::PolicyEngine::new(&cfg),
        )
        .unwrap();
        let id = upload.transfer_id;
        super::super::transfer::upload_chunk_request(
            &mut upload,
            RawChunk {
                transfer_id: id,
                chunk_index: 0,
                offset: 0,
                chunk_bytes: b"same".to_vec(),
            },
            &cfg,
        )
        .unwrap();
        (path, cfg, context, upload)
    }
    #[test]
    fn reconnect_requires_same_session_and_rechecks_current_policy() {
        let (path, mut cfg, context, upload) = fixture();
        let id = upload.transfer_id;
        let stage = upload.staging_path.clone();
        suspend(&context, upload, 60);
        let mut other = context.clone();
        other.session_id = SessionId(uuid::Uuid::new_v4());
        assert!(matches!(
            take(&other, id, &cfg),
            Err(TealDriveError::NotFound)
        ));
        other.session_id = context.session_id;
        other.uid += 1;
        assert!(matches!(
            take(&other, id, &cfg),
            Err(TealDriveError::NotFound)
        ));
        let (upload, response) = take(&context, id, &cfg).unwrap();
        assert_eq!(response.bytes_received, 4);
        assert_eq!(
            response.prefix_sha256,
            format!("{:x}", Sha256::digest(b"same"))
        );
        assert!(take(&context, id, &cfg).is_err());
        suspend(&context, upload, 60);
        cfg.roots[0].read_only = true;
        assert!(take(&context, id, &cfg).is_err());
        assert!(!stage.exists());
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn expiration_and_revoked_session_release_staging() {
        let (path, cfg, context, upload) = fixture();
        let id = upload.transfer_id;
        let stage = upload.staging_path.clone();
        suspend(&context, upload, 0);
        purge(None);
        assert!(!stage.exists());
        assert!(take(&context, id, &cfg).is_err());
        std::fs::remove_dir_all(path).unwrap();
        let (path, _, context, upload) = fixture();
        let stage = upload.staging_path.clone();
        suspend(&context, upload, 60);
        purge(Some(context.session_id));
        assert!(!stage.exists());
        std::fs::remove_dir_all(path).unwrap();
    }
}
