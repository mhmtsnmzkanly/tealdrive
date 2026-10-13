//! Root-generated staging tokens; no filename glob or user-provided cleanup path.
use super::{
    lifecycle::{stage_target, staging_record},
    store::{AccountStatus, AccountStore, StoreError},
};
use crate::config::AppConfig;
use rusqlite::params;
use std::{
    os::unix::fs::MetadataExt,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
#[derive(Default, serde::Serialize)]
pub struct StagingResult {
    pub staging_cleaned: usize,
    pub staging_skipped: usize,
}
pub(crate) fn cleanup(store: &AccountStore, cfg: &AppConfig) -> Result<StagingResult, StoreError> {
    if unsafe { libc::getuid() } != 0 || unsafe { libc::geteuid() } != 0 {
        return Err(StoreError::Forbidden);
    }
    cfg.lifecycle.validate()?;
    let mut result = StagingResult::default();
    if !cfg.journal.enabled || !cfg.lifecycle.staging_cleanup_enabled {
        return Ok(result);
    }
    let at = super::store::now();
    let rows: Vec<(String, String)> = {
        let mut query=store.connection.prepare("SELECT json_extract(value,'$.accountId'),json_extract(value,'$.id') FROM security_state WHERE key GLOB 'staging_expiry_v1:*' AND json_extract(value,'$.stagingGcStatus') IS NULL AND json_extract(value,'$.createdAt')<=?1 ORDER BY key LIMIT ?2")?;
        let rows = query.query_map(
            params![
                super::store::number(
                    at.saturating_sub(cfg.lifecycle.staging_retention_hours * 3600)
                )?,
                cfg.lifecycle.max_cleanup_per_run
            ],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        rows.collect::<Result<_, _>>()?
    };
    let exe = std::env::current_exe()?;
    let meta = std::fs::symlink_metadata(&exe)?;
    if !meta.is_file() || meta.uid() != 0 || meta.mode() & 0o022 != 0 {
        return Err(StoreError::Forbidden);
    }
    let started = Instant::now();
    for (account_id, operation_id) in rows {
        if started.elapsed() >= Duration::from_secs(60) {
            break;
        }
        let operation = staging_record(store, &account_id, &operation_id)?;
        let actor = store.get(&account_id)?;
        let token = format!("staging_expiry_v1:{account_id}:{operation_id}");
        if actor.status == AccountStatus::Deleted {
            let purged: bool = store.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM security_state WHERE key=?1)",
                [format!("virtual_purged_v1:{account_id}")],
                |r| r.get(0),
            )?;
            if purged {
                store
                    .connection
                    .execute("DELETE FROM security_state WHERE key=?1", [token])?;
                continue;
            }
        }
        if actor.status != AccountStatus::Active {
            result.staging_skipped += 1;
            continue;
        }
        let root = crate::policy::home::resolve(cfg, &actor, cfg.home_template.as_ref())
            .map_err(|_| StoreError::Forbidden)?
            .roots
            .into_iter()
            .find(|r| r.root_id == operation.root_id && r.base_path == actor.home)
            .ok_or(StoreError::Forbidden)?;
        let _lock = crate::helper::quota::Reservation::acquire(&root)?;
        let mut child = Command::new(&exe)
            .args([
                "__staging-expiry-worker",
                &account_id,
                &operation_id,
                &at.to_string(),
            ])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let timer = Instant::now();
        let success = loop {
            if let Some(status) = child.try_wait()? {
                break status.success();
            }
            if timer.elapsed() >= Duration::from_secs(30) {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
            std::thread::sleep(Duration::from_millis(25));
        };
        if success {
            result.staging_cleaned += 1;
            store
                .connection
                .execute("DELETE FROM security_state WHERE key=?1", [token])?;
        } else {
            result.staging_skipped += 1;
            let mut record = operation;
            record.staging_gc_status = Some("needsReview".into());
            let raw = serde_json::to_string(&record).map_err(|_| StoreError::InvalidInput)?;
            if raw.len() > cfg.journal.max_record_bytes {
                return Err(StoreError::InvalidInput);
            }
            store.connection.execute(
                "UPDATE security_state SET value=?1 WHERE key=?2",
                params![raw, token],
            )?;
        }
    }
    Ok(result)
}
pub(crate) fn worker(account_id: &str, operation_id: &str, as_of: u64) -> Result<(), StoreError> {
    if unsafe { libc::getuid() } != 0 || unsafe { libc::geteuid() } != 0 {
        return Err(StoreError::Forbidden);
    }
    let parent = unsafe { libc::getppid() };
    let cfg = crate::helper::security::trusted_config().map_err(|_| StoreError::Forbidden)?;
    if !cfg.journal.enabled || !cfg.lifecycle.staging_cleanup_enabled {
        return Err(StoreError::Forbidden);
    }
    let store = AccountStore::open(std::path::Path::new(crate::layout::ACCOUNTS))?;
    let operation = staging_record(&store, account_id, operation_id)?;
    let (path, destination) = stage_target(&operation)?;
    let cutoff = as_of.saturating_sub(cfg.lifecycle.staging_retention_hours * 3600);
    let actor = store.get(account_id)?;
    if actor.status != AccountStatus::Active || operation.created_at > cutoff {
        return Err(StoreError::Forbidden);
    }
    crate::helper::account_gateway::validate_account(&cfg, &actor)
        .map_err(|_| StoreError::Forbidden)?;
    let mut view = crate::policy::home::resolve(&cfg, &actor, cfg.home_template.as_ref())
        .map_err(|_| StoreError::Forbidden)?;
    crate::helper::account_gateway::apply_persisted_policy(&mut view, &actor)
        .map_err(|_| StoreError::Forbidden)?;
    let root = view
        .roots
        .iter()
        .find(|r| r.root_id == operation.root_id && r.base_path == actor.home)
        .cloned()
        .ok_or(StoreError::Forbidden)?;
    if root.read_only {
        return Err(StoreError::Forbidden);
    }
    super::virtuals::no_nested_mounts(&root.base_path.join(&path.0))?;
    drop(store);
    crate::helper::security::enter_identity(&crate::config::UserContext {
        username: actor.username.clone(),
        uid: actor.uid,
        gid: actor.gid,
    })
    .map_err(|_| StoreError::Forbidden)?;
    crate::helper::sandbox::confine(&root)?;
    if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM, 0, 0, 0) } != 0
        || unsafe { libc::getppid() } != parent
    {
        return Err(StoreError::Forbidden);
    }
    crate::helper::maintenance::expire_staging(
        &view,
        &actor,
        &root,
        &operation,
        &path,
        &destination,
        cutoff,
    )
}
