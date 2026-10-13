//! Root-private undo preferences and restore fence. Neither changes login authority.
use super::{
    journal::{JournalConfig, OperationRecord},
    store::{authorized, number, AccountStore, Authority, StoreError},
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoPreferences {
    pub hours: Option<u64>,
    pub effective_hours: u64,
    pub maximum_hours: u64,
    pub revision: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoCandidate {
    pub operation: OperationRecord,
    pub available: bool,
    pub unavailable_reason: Option<String>,
    pub expires_at: u64,
    pub expected_state: Option<String>,
}
pub fn candidate(
    record: OperationRecord,
    prefs: &UndoPreferences,
    current_epoch: &str,
    at: u64,
) -> UndoCandidate {
    let expires_at = record
        .created_at
        .saturating_add(prefs.effective_hours.saturating_mul(3600));
    let reason = if record.status != "success" {
        Some("operationIncomplete")
    } else if !["move", "rename", "trash", "chmod"].contains(&record.action.as_str()) {
        Some("unsupportedOperation")
    } else if record.undo_status.is_some() {
        Some("alreadyClaimed")
    } else if record.undo_epoch.as_deref() != Some(current_epoch) {
        Some("restoredBackup")
    } else if at >= expires_at {
        Some("expired")
    } else if record.after.is_none() || record.before.is_none() {
        Some("missingExpectedState")
    } else if record.action == "chmod" && record.old_mode.is_none_or(|m| m & !0o777 != 0) {
        Some("unsupportedOriginalMode")
    } else if record.action != "chmod"
        && (record.target_path.is_none()
            || !record.before.as_ref().is_some_and(|s| s.fully_observed())
            || !record.after.as_ref().is_some_and(|s| s.fully_observed())
            || !record
                .before
                .as_ref()
                .unwrap()
                .same_inode(record.after.as_ref().unwrap()))
    {
        Some("incompleteObservation")
    } else {
        None
    };
    UndoCandidate {
        expected_state: record.after.as_ref().map(|s| s.digest.clone()),
        operation: record,
        available: reason.is_none(),
        unavailable_reason: reason.map(str::to_owned),
        expires_at,
    }
}
fn revision(db: &rusqlite::Connection, id: &str) -> Result<u64, StoreError> {
    let value: Option<String> = db
        .query_row(
            "SELECT value FROM security_state WHERE key=?1",
            [format!("undo_preferences_v1:{id}")],
            |r| r.get(0),
        )
        .optional()?;
    value.map_or(Ok(0), |v| v.parse().map_err(|_| StoreError::InvalidInput))
}
pub(super) fn epoch(db: &rusqlite::Connection) -> Result<String, StoreError> {
    let value: Option<String> = db
        .query_row(
            "SELECT value FROM security_state WHERE key='undo_epoch_v1'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(value) = value {
        uuid::Uuid::parse_str(&value).map_err(|_| StoreError::InvalidInput)?;
        return Ok(value);
    }
    let value = uuid::Uuid::new_v4().to_string();
    db.execute(
        "INSERT OR IGNORE INTO security_state(key,value) VALUES('undo_epoch_v1',?1)",
        [&value],
    )?;
    Ok(db.query_row(
        "SELECT value FROM security_state WHERE key='undo_epoch_v1'",
        [],
        |r| r.get(0),
    )?)
}
impl AccountStore {
    pub fn finish_undo(
        &mut self,
        record: &OperationRecord,
        config: &JournalConfig,
        response: Option<&crate::helper::types::HelperResponse>,
    ) -> Result<OperationRecord, StoreError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let key = Self::operation_cursor(record);
        let raw: String = tx.query_row(
            "SELECT value FROM security_state WHERE key=?1",
            [&key],
            |r| r.get(0),
        )?;
        let mut current: OperationRecord =
            serde_json::from_str(&raw).map_err(|_| StoreError::InvalidInput)?;
        if current.id != record.id
            || current.account_id != record.account_id
            || current.undo_status.as_deref() != Some("pending")
        {
            return Err(StoreError::Conflict);
        }
        current.undo_status = Some(
            match response {
                Some(r) if r.success => "success",
                Some(_) => "failed",
                None => "unknown",
            }
            .into(),
        );
        current.updated_at = super::store::now();
        current.undo_error_code = response.and_then(|response| response.error_kind);
        let value = serde_json::to_string(&current).map_err(|_| StoreError::InvalidInput)?;
        if value.len() > config.max_record_bytes {
            return Err(StoreError::InvalidInput);
        }
        tx.execute(
            "UPDATE security_state SET value=?1 WHERE key=?2",
            params![value, key],
        )?;
        tx.commit()?;
        Ok(current)
    }
    pub fn undo_candidate(
        &self,
        authority: &Authority,
        config: &JournalConfig,
        operation_id: &str,
        at: u64,
    ) -> Result<UndoCandidate, StoreError> {
        uuid::Uuid::parse_str(operation_id).map_err(|_| StoreError::InvalidInput)?;
        let actor = authorized(&self.connection, authority)?.ok_or(StoreError::Forbidden)?;
        let raw: Option<String> = self.connection.query_row("SELECT value FROM security_state WHERE key GLOB ?1 AND json_extract(value,'$.id')=?2 LIMIT 1", params![format!("operation_v1:{}:*", actor.id), operation_id], |r| r.get(0)).optional()?;
        let record: OperationRecord = serde_json::from_str(&raw.ok_or(StoreError::NotFound)?)
            .map_err(|_| StoreError::InvalidInput)?;
        if record.account_id != actor.id || record.id != operation_id || record.version != 1 {
            return Err(StoreError::InvalidInput);
        }
        Ok(candidate(
            record,
            &self.undo_preferences(authority, config)?,
            &epoch(&self.connection)?,
            at,
        ))
    }
    /// Durable single-use claim is recorded before the unprivileged worker runs.
    pub fn claim_undo(
        &mut self,
        authority: &Authority,
        config: &JournalConfig,
        operation_id: &str,
        expected_state: &str,
        at: u64,
    ) -> Result<OperationRecord, StoreError> {
        config.validate()?;
        uuid::Uuid::parse_str(operation_id).map_err(|_| StoreError::InvalidInput)?;
        if expected_state.len() != 64 || !expected_state.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(StoreError::InvalidInput);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let actor = authorized(&tx, authority)?.ok_or(StoreError::Forbidden)?;
        let row: Option<(String,String)> = tx.query_row("SELECT key,value FROM security_state WHERE key GLOB ?1 AND json_extract(value,'$.id')=?2 LIMIT 1", params![format!("operation_v1:{}:*", actor.id), operation_id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        let (key, raw) = row.ok_or(StoreError::NotFound)?;
        let mut record: OperationRecord =
            serde_json::from_str(&raw).map_err(|_| StoreError::InvalidInput)?;
        let prefs = UndoPreferences {
            hours: actor.undo_hours,
            effective_hours: actor
                .undo_hours
                .unwrap_or(config.undo_max_hours)
                .min(config.undo_max_hours),
            maximum_hours: config.undo_max_hours,
            revision: 0,
        };
        let eligibility = candidate(record.clone(), &prefs, &epoch(&tx)?, at);
        if record.version != 1
            || record.account_id != actor.id
            || record.id != operation_id
            || !eligibility.available
            || eligibility.expected_state.as_deref() != Some(expected_state)
        {
            return Err(StoreError::Conflict);
        }
        record.undo_status = Some("pending".into());
        let value = serde_json::to_string(&record).map_err(|_| StoreError::InvalidInput)?;
        if value.len() > config.max_record_bytes {
            return Err(StoreError::InvalidInput);
        }
        tx.execute(
            "UPDATE security_state SET value=?1 WHERE key=?2 AND value=?3",
            params![value, key, raw],
        )?;
        tx.commit()?;
        Ok(record)
    }
    pub fn undo_preferences(
        &self,
        authority: &Authority,
        config: &JournalConfig,
    ) -> Result<UndoPreferences, StoreError> {
        config.validate()?;
        let actor = authorized(&self.connection, authority)?.ok_or(StoreError::Forbidden)?;
        Ok(UndoPreferences {
            hours: actor.undo_hours,
            effective_hours: actor
                .undo_hours
                .unwrap_or(config.undo_max_hours)
                .min(config.undo_max_hours),
            maximum_hours: config.undo_max_hours,
            revision: revision(&self.connection, &actor.id)?,
        })
    }
    pub fn set_undo_preferences(
        &mut self,
        authority: &Authority,
        config: &JournalConfig,
        hours: Option<u64>,
        expected_revision: u64,
    ) -> Result<UndoPreferences, StoreError> {
        config.validate()?;
        if hours.is_some_and(|hours| !(1..=config.undo_max_hours).contains(&hours)) {
            return Err(StoreError::InvalidInput);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let actor = authorized(&tx, authority)?.ok_or(StoreError::Forbidden)?;
        let current = revision(&tx, &actor.id)?;
        if current != expected_revision {
            return Err(StoreError::Conflict);
        }
        let next = current
            .checked_add(1)
            .filter(|v| *v <= i64::MAX as u64)
            .ok_or(StoreError::Conflict)?;
        tx.execute(
            "UPDATE accounts SET undo_hours=?1,updated_at=?2 WHERE id=?3",
            params![
                hours.map(number).transpose()?,
                number(super::store::now())?,
                actor.id
            ],
        )?;
        tx.execute("INSERT INTO security_state(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![format!("undo_preferences_v1:{}", actor.id), next.to_string()])?;
        tx.commit()?;
        self.undo_preferences(authority, config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::store::{AccountKind, NewAccount, Role};
    #[test]
    fn preferences_inherit_root_limit_compare_and_swap_and_do_not_revoke_sessions() {
        let mut store = AccountStore::memory();
        let account = store
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
        let actor = Authority::Account {
            id: account.id.clone(),
            generation: account.generation,
        };
        let mut config = JournalConfig::default();
        assert_eq!(
            store
                .undo_preferences(&actor, &config)
                .unwrap()
                .effective_hours,
            24
        );
        assert!(matches!(
            store.set_undo_preferences(&actor, &config, Some(0), 0),
            Err(StoreError::InvalidInput)
        ));
        assert!(matches!(
            store.set_undo_preferences(&actor, &config, Some(25), 0),
            Err(StoreError::InvalidInput)
        ));
        assert_eq!(
            store
                .set_undo_preferences(&actor, &config, Some(12), 0)
                .unwrap()
                .revision,
            1
        );
        assert!(matches!(
            store.set_undo_preferences(&actor, &config, Some(6), 0),
            Err(StoreError::Conflict)
        ));
        assert_eq!(
            store.get(&account.id).unwrap().generation,
            account.generation
        );
        config.undo_max_hours = 4;
        assert_eq!(
            store
                .undo_preferences(&actor, &config)
                .unwrap()
                .effective_hours,
            4
        );
        assert_eq!(
            store
                .set_undo_preferences(&actor, &config, None, 1)
                .unwrap()
                .effective_hours,
            4
        );
        assert!(matches!(
            store.undo_preferences(&Authority::Root, &config),
            Err(StoreError::Forbidden)
        ));
    }
    #[test]
    fn claims_are_scoped_single_use_expiring_and_backup_fenced() {
        use crate::helper::types::{HelperCommand, HelperResponse};
        let mut store = AccountStore::memory();
        let account = store
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
        let authority = Authority::Account {
            id: account.id.clone(),
            generation: account.generation,
        };
        let config = JournalConfig {
            enabled: true,
            ..Default::default()
        };
        let mut request = crate::helper::ipc::tests_support::sample_request(HelperCommand::Chmod);
        request.username = account.username.clone();
        request.uid = account.uid;
        request.gid = account.gid;
        let pending = store
            .begin_operation(&account, &request, &config, 1000)
            .unwrap();
        let state = crate::helper::observation::FileState {
            digest: "a".repeat(64),
            device: "1".into(),
            inode: "2".into(),
            kind: "file".into(),
            mode: 0,
            uid: 1000,
            gid: 1000,
            size: 4,
            links: 1,
            mtime_seconds: 1,
            mtime_nanos: 0,
            ctime_seconds: 2,
            ctime_nanos: 0,
            content_digest: None,
            tree_digest: None,
        };
        let mut response = HelperResponse::operation_done(&request, "done");
        let metadata = response.audit_metadata.as_mut().unwrap();
        metadata.old_mode = Some(0o600);
        metadata.new_mode = Some(0);
        metadata.before = Some(state.clone());
        metadata.after = Some(state);
        let done = store
            .finish_operation(&pending, Some(&response), &config, 1001)
            .unwrap();
        assert!(
            store
                .undo_candidate(&authority, &config, &done.id, 1002)
                .unwrap()
                .available
        );
        assert_eq!(
            store
                .undo_candidate(&authority, &config, &done.id, 1000 + 24 * 3600)
                .unwrap()
                .unavailable_reason
                .as_deref(),
            Some("expired")
        );
        assert!(matches!(
            store.claim_undo(&authority, &config, &done.id, &"b".repeat(64), 1002),
            Err(StoreError::Conflict)
        ));
        let claimed = store
            .claim_undo(&authority, &config, &done.id, &"a".repeat(64), 1002)
            .unwrap();
        assert_eq!(claimed.undo_status.as_deref(), Some("pending"));
        assert!(matches!(
            store.claim_undo(&authority, &config, &done.id, &"a".repeat(64), 1003),
            Err(StoreError::Conflict)
        ));
        let pending = store
            .begin_operation(&account, &request, &config, 1004)
            .unwrap();
        let done = store
            .finish_operation(&pending, Some(&response), &config, 1005)
            .unwrap();
        store
            .invalidate_restored_sessions(&Authority::Root)
            .unwrap();
        let restored = store.get(&account.id).unwrap();
        let authority = Authority::Account {
            id: restored.id,
            generation: restored.generation,
        };
        assert_eq!(
            store
                .undo_candidate(&authority, &config, &done.id, 1006)
                .unwrap()
                .unavailable_reason
                .as_deref(),
            Some("restoredBackup")
        );
    }
    #[test]
    fn offline_restore_rotates_undo_authority_but_preserves_history() {
        let mut store = AccountStore::memory();
        let before = epoch(&store.connection).unwrap();
        assert_eq!(before, epoch(&store.connection).unwrap());
        store.connection.execute("INSERT INTO security_state(key,value) VALUES('operation_v1:fixture','audit evidence')", []).unwrap();
        store
            .invalidate_restored_sessions(&Authority::Root)
            .unwrap();
        assert_ne!(before, epoch(&store.connection).unwrap());
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT value FROM security_state WHERE key='operation_v1:fixture'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            "audit evidence"
        );
    }
}
