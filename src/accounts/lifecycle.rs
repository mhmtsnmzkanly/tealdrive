//! Root schedules bounded maintenance; a fresh Wizard worker drops IDs before user data access.
use super::{
    journal::OperationRecord,
    store::{AccountStatus, AccountStore, StoreError},
};
use crate::config::{AppConfig, RelativePath};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::{
    os::unix::fs::MetadataExt,
    process::{Command, Stdio},
};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LifecycleConfig {
    pub trash_cleanup_enabled: bool,
    pub trash_retention_hours: u64,
    pub staging_retention_hours: u64,
    pub max_cleanup_per_run: u16,
}
impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            trash_cleanup_enabled: true,
            trash_retention_hours: 504,
            staging_retention_hours: 24,
            max_cleanup_per_run: 100,
        }
    }
}
impl LifecycleConfig {
    pub fn validate(&self) -> Result<(), StoreError> {
        if !(1..=876000).contains(&self.trash_retention_hours)
            || !(1..=876000).contains(&self.staging_retention_hours)
            || !(1..=1000).contains(&self.max_cleanup_per_run)
        {
            return Err(StoreError::InvalidInput);
        }
        Ok(())
    }
}
#[derive(Debug, Default, Serialize)]
pub struct CleanupResult {
    pub trash_cleaned: usize,
    pub trash_skipped: usize,
}
pub(crate) fn record(
    store: &AccountStore,
    account_id: &str,
    operation_id: &str,
) -> Result<OperationRecord, StoreError> {
    uuid::Uuid::parse_str(account_id).map_err(|_| StoreError::InvalidInput)?;
    uuid::Uuid::parse_str(operation_id).map_err(|_| StoreError::InvalidInput)?;
    let raw: String = store.connection.query_row(
        "SELECT value FROM security_state WHERE key=?1",
        params![format!("trash_expiry_v1:{account_id}:{operation_id}")],
        |r| r.get(0),
    )?;
    let record: OperationRecord =
        serde_json::from_str(&raw).map_err(|_| StoreError::InvalidInput)?;
    if record.version != 1 || record.account_id != account_id || record.id != operation_id {
        return Err(StoreError::InvalidInput);
    }
    Ok(record)
}
fn target(record: &OperationRecord) -> Result<RelativePath, StoreError> {
    let path = record
        .target_path
        .as_ref()
        .ok_or(StoreError::InvalidInput)?;
    let id = path
        .0
        .strip_prefix(".tealdrive-trash/files/")
        .ok_or(StoreError::InvalidInput)?;
    uuid::Uuid::parse_str(id).map_err(|_| StoreError::InvalidInput)?;
    Ok(path.clone())
}
fn eligible(record: &OperationRecord, cfg: &AppConfig, at: u64) -> bool {
    cfg.lifecycle.trash_cleanup_enabled
        && record.action == "trash"
        && record.status == "success"
        && (record.undo_status.is_none() || record.undo_status.as_deref() == Some("failed"))
        && record.created_at
            <= at.saturating_sub(cfg.lifecycle.trash_retention_hours.saturating_mul(3600))
        && record
            .after
            .as_ref()
            .is_some_and(|state| state.kind == "file" || state.fully_observed())
        && target(record).is_ok()
}
pub(crate) fn cleanup(store: &AccountStore, cfg: &AppConfig) -> Result<CleanupResult, StoreError> {
    if unsafe { libc::getuid() } != 0 || unsafe { libc::geteuid() } != 0 {
        return Err(StoreError::Forbidden);
    }
    cfg.lifecycle.validate()?;
    let mut result = CleanupResult::default();
    if !cfg.journal.enabled || !cfg.lifecycle.trash_cleanup_enabled {
        return Ok(result);
    }
    let at = super::store::now();
    let rows: Vec<(String, String)> = {
        let mut query = store.connection.prepare("SELECT json_extract(value,'$.accountId'),json_extract(value,'$.id') FROM security_state WHERE key GLOB 'trash_expiry_v1:*' AND json_extract(value,'$.trashGcStatus') IS NULL AND json_extract(value,'$.action')='trash' AND json_extract(value,'$.status')='success' AND json_extract(value,'$.createdAt')<=?1 ORDER BY key LIMIT ?2")?;
        let rows = query.query_map(
            params![
                super::store::number(
                    at.saturating_sub(cfg.lifecycle.trash_retention_hours * 3600)
                )?,
                cfg.lifecycle.max_cleanup_per_run
            ],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        rows.collect::<Result<_, _>>()?
    };
    let executable = std::env::current_exe()?;
    let metadata = std::fs::symlink_metadata(&executable)?;
    if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
        return Err(StoreError::Forbidden);
    }
    let run_started = std::time::Instant::now();
    for (account_id, operation_id) in rows {
        if run_started.elapsed() >= std::time::Duration::from_secs(60) {
            break;
        }
        let operation = record(store, &account_id, &operation_id)?;
        let actor = store.get(&account_id)?;
        if actor.status == AccountStatus::Deleted {
            let purged: bool = store.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM security_state WHERE key=?1)",
                [format!("virtual_purged_v1:{account_id}")],
                |r| r.get(0),
            )?;
            if purged {
                store.connection.execute(
                    "DELETE FROM security_state WHERE key=?1",
                    [format!("trash_expiry_v1:{account_id}:{operation_id}")],
                )?;
                continue;
            }
        }
        if !eligible(&operation, cfg, at) || actor.status != AccountStatus::Active {
            result.trash_skipped += 1;
            continue;
        }
        let root = crate::policy::home::resolve(cfg, &actor, cfg.home_template.as_ref())
            .map_err(|_| StoreError::Forbidden)?
            .roots
            .into_iter()
            .find(|root| root.root_id == operation.root_id && root.base_path == actor.home)
            .ok_or(StoreError::Forbidden)?;
        let _lock = crate::helper::quota::Reservation::acquire(&root)?;
        let mut child = Command::new(&executable)
            .args(["__trash-expiry-worker", &account_id, &operation_id])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let started = std::time::Instant::now();
        let success = loop {
            if let Some(status) = child.try_wait()? {
                break status.success();
            }
            if started.elapsed() >= std::time::Duration::from_secs(30) {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        };
        if success {
            result.trash_cleaned += 1;
            store.connection.execute(
                "DELETE FROM security_state WHERE key=?1",
                [format!("trash_expiry_v1:{account_id}:{operation_id}")],
            )?;
        } else {
            result.trash_skipped += 1;
            let mut retained = operation;
            retained.trash_gc_status = Some("needsReview".into());
            let raw = serde_json::to_string(&retained).map_err(|_| StoreError::InvalidInput)?;
            if raw.len() > cfg.journal.max_record_bytes {
                return Err(StoreError::InvalidInput);
            }
            store.connection.execute(
                "UPDATE security_state SET value=?1 WHERE key=?2",
                params![raw, format!("trash_expiry_v1:{account_id}:{operation_id}")],
            )?;
        }
    }
    Ok(result)
}
pub(crate) fn worker(account_id: &str, operation_id: &str) -> Result<(), StoreError> {
    let parent_pid = unsafe { libc::getppid() };
    if unsafe { libc::getuid() } != 0 || unsafe { libc::geteuid() } != 0 {
        return Err(StoreError::Forbidden);
    }
    let cfg = crate::helper::security::trusted_config().map_err(|_| StoreError::Forbidden)?;
    let store = AccountStore::open(std::path::Path::new(crate::layout::ACCOUNTS))?;
    let operation = record(&store, account_id, operation_id)?;
    let actor = store.get(account_id)?;
    if actor.status != AccountStatus::Active || !eligible(&operation, &cfg, super::store::now()) {
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
        .find(|root| root.root_id == operation.root_id && root.base_path == actor.home)
        .cloned()
        .ok_or(StoreError::Forbidden)?;
    if root.read_only {
        return Err(StoreError::Forbidden);
    }
    let path = target(&operation)?;
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
        || unsafe { libc::getppid() } != parent_pid
    {
        return Err(StoreError::Forbidden);
    }
    crate::helper::maintenance::expire_trash(&view, &actor, &root, &operation, &path)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::{
        journal::JournalConfig,
        store::{AccountKind, Authority, NewAccount, Role},
    };
    use crate::helper::types::{
        HelperCommand, HelperResponse, HelperResponsePayload, HelperTrashEntry,
    };
    #[test]
    fn root_cleanup_token_survives_audit_retention_and_stays_bounded() {
        let mut store = AccountStore::memory();
        let actor = store
            .create(
                &Authority::Root,
                NewAccount {
                    username: "alice".into(),
                    uid: 1000,
                    gid: 1000,
                    home: "/home/alice".into(),
                    kind: AccountKind::Linux,
                    role: Role::User,
                    owner_admin_id: None,
                    quota_bytes: None,
                },
            )
            .unwrap();
        let cfg = JournalConfig {
            enabled: true,
            retention_hours: 1,
            undo_max_hours: 1,
            max_entries_per_account: 100,
            max_total_entries: 100,
            ..Default::default()
        };
        let mut request =
            crate::helper::ipc::tests_support::sample_request(HelperCommand::MoveToTrash);
        request.username = actor.username.clone();
        request.uid = actor.uid;
        request.gid = actor.gid;
        let pending = store.begin_operation(&actor, &request, &cfg, 1000).unwrap();
        assert_eq!(
            target(&pending).unwrap().0,
            format!(".tealdrive-trash/files/{}", pending.id)
        );
        let mut reply = HelperResponse::operation_done(&request, "done");
        reply.payload = HelperResponsePayload::TrashEntry(HelperTrashEntry {
            trash_id: pending.id.clone(),
            display_name: "file".into(),
            deleted_at: 1000,
            original_relative_path: request.relative_path.clone(),
            original_root_id: request.root_id.clone(),
        });
        store
            .finish_operation(&pending, Some(&reply), &cfg, 1001)
            .unwrap();
        store.gc_operations(&cfg, 4601).unwrap();
        assert_eq!(
            record(&store, &actor.id, &pending.id).unwrap().status,
            "success"
        );
        for _ in 1..100 {
            store.begin_operation(&actor, &request, &cfg, 5000).unwrap();
        }
        store.gc_operations(&cfg, 8601).unwrap();
        assert!(matches!(
            store.begin_operation(&actor, &request, &cfg, 9000),
            Err(StoreError::QuotaExceeded)
        ));
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT count(*) FROM security_state WHERE key GLOB 'operation_v1:*'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
    }
    #[test]
    fn lifecycle_hours_and_work_bounds_validate_independently_from_audit_ttl() {
        let mut cfg = AppConfig::default();
        cfg.lifecycle.trash_retention_hours = 0;
        assert!(cfg.validate().is_err());
        cfg.lifecycle = Default::default();
        cfg.lifecycle.max_cleanup_per_run = 1001;
        assert!(cfg.validate().is_err());
        cfg.lifecycle = Default::default();
        assert_eq!(cfg.lifecycle.trash_retention_hours, 504);
        assert_eq!(cfg.lifecycle.staging_retention_hours, 24);
        assert!(cfg.validate().is_ok());
    }
}
