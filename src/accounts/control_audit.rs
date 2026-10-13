//! Security/control facts are protected from the network process. Never serialize commands.
use super::{
    grants::CallerBinding,
    store::{number, AccountStore, StoreError},
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ControlAuditConfig {
    pub retention_hours: u64,
    pub max_records: u32,
}
impl Default for ControlAuditConfig {
    fn default() -> Self {
        Self {
            retention_hours: 504,
            max_records: 16384,
        }
    }
}
impl ControlAuditConfig {
    pub fn validate(&self) -> Result<(), StoreError> {
        if !(1..=876000).contains(&self.retention_hours)
            || !(100..=100000).contains(&self.max_records)
        {
            Err(StoreError::InvalidInput)
        } else {
            Ok(())
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlRecord {
    pub version: u8,
    pub id: String,
    pub action: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub caller_uid: u32,
    pub caller_pid: u32,
    pub subject_claim: Option<String>,
    pub target_claim: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
}
fn key(record: &ControlRecord) -> String {
    format!("control_v1:{:020}:{}", record.created_at, record.id)
}
impl AccountStore {
    pub fn control_records(
        &self,
        authority: &super::store::Authority,
        limit: u16,
    ) -> Result<Vec<ControlRecord>, StoreError> {
        if !matches!(authority, super::store::Authority::Root) || !(1..=200).contains(&limit) {
            return Err(StoreError::Forbidden);
        }
        let mut query=self.connection.prepare("SELECT value FROM security_state WHERE key GLOB 'control_v1:*' ORDER BY key DESC LIMIT ?1")?;
        let rows = query.query_map([limit], |row| row.get::<_, String>(0))?;
        let mut records = Vec::new();
        for raw in rows {
            records.push(serde_json::from_str(&raw?).map_err(|_| StoreError::InvalidInput)?);
        }
        Ok(records)
    }
    pub fn begin_control(
        &mut self,
        config: &ControlAuditConfig,
        caller: &CallerBinding,
        action: &str,
        subject: Option<&str>,
        target: Option<&str>,
        at: u64,
    ) -> Result<ControlRecord, StoreError> {
        config.validate()?;
        caller.validate()?;
        if action.is_empty()
            || action.len() > 64
            || !action
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
            || subject
                .into_iter()
                .chain(target)
                .any(|v| v.len() > 256 || v.chars().any(char::is_control))
        {
            return Err(StoreError::InvalidInput);
        }
        let record = ControlRecord {
            version: 1,
            id: uuid::Uuid::new_v4().to_string(),
            action: action.into(),
            created_at: at,
            updated_at: at,
            caller_uid: caller.uid,
            caller_pid: caller.pid,
            subject_claim: subject.map(str::to_owned),
            target_claim: target.map(str::to_owned),
            status: "pending".into(),
            error_code: None,
        };
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("DELETE FROM security_state WHERE key IN (SELECT key FROM security_state WHERE key GLOB 'control_v1:*' AND json_extract(value,'$.createdAt')<?1 LIMIT 500)",[number(at.saturating_sub(config.retention_hours*3600))?])?;
        let count: i64 = tx.query_row(
            "SELECT count(*) FROM security_state WHERE key GLOB 'control_v1:*'",
            [],
            |r| r.get(0),
        )?;
        if count >= i64::from(config.max_records) {
            let old:Option<String>=tx.query_row("SELECT key FROM security_state WHERE key GLOB 'control_v1:*' AND json_extract(value,'$.status') IN ('success','failed') ORDER BY key LIMIT 1",[],|r|r.get(0)).optional()?;
            let Some(old) = old else {
                return Err(StoreError::QuotaExceeded);
            };
            tx.execute("DELETE FROM security_state WHERE key=?1", [old])?;
        }
        tx.execute(
            "INSERT INTO security_state(key,value) VALUES(?1,?2)",
            params![
                key(&record),
                serde_json::to_string(&record).map_err(|_| StoreError::InvalidInput)?
            ],
        )?;
        tx.commit()?;
        Ok(record)
    }
    pub fn finish_control(
        &mut self,
        record: &ControlRecord,
        success: bool,
        error: Option<&str>,
        at: u64,
    ) -> Result<(), StoreError> {
        if error.is_some_and(|v| {
            v.len() > 64 || !v.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        }) {
            return Err(StoreError::InvalidInput);
        }
        let mut done = record.clone();
        done.status = if success { "success" } else { "failed" }.into();
        done.updated_at = at;
        done.error_code = error.map(str::to_owned);
        let changed=self.connection.execute("UPDATE security_state SET value=?1 WHERE key=?2 AND json_extract(value,'$.status')='pending'",params![serde_json::to_string(&done).map_err(|_|StoreError::InvalidInput)?,key(record)])?;
        if changed != 1 {
            return Err(StoreError::Conflict);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protected_intent_rotation_and_completion_are_bounded() {
        let mut store = AccountStore::memory();
        let caller = CallerBinding::capture().unwrap();
        let cfg = ControlAuditConfig {
            max_records: 100,
            ..Default::default()
        };
        let first = store
            .begin_control(&cfg, &caller, "Login", Some("alice"), None, 1000)
            .unwrap();
        store
            .finish_control(&first, false, Some("InvalidCredentials"), 1001)
            .unwrap();
        assert!(matches!(
            store.finish_control(&first, true, None, 1002),
            Err(StoreError::Conflict)
        ));
        for _ in 1..100 {
            store
                .begin_control(&cfg, &caller, "Login", None, None, 1000)
                .unwrap();
        }
        store
            .begin_control(&cfg, &caller, "Login", None, None, 1000)
            .unwrap();
        assert!(matches!(
            store.begin_control(&cfg, &caller, "Login", None, None, 1000),
            Err(StoreError::QuotaExceeded)
        ));
        assert!(store
            .begin_control(&cfg, &caller, "Login", Some("bad\nclaim"), None, 1000)
            .is_err());
    }
}
