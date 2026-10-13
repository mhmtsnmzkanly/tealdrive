//! Protected operation intent/result records. User contents and credential values never enter here.
use super::store::{authorized, number, Account, AccountStore, Authority, StoreError};
use crate::helper::types::{HelperCommand, HelperRequest, HelperResponse};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JournalConfig {
    pub enabled: bool,
    pub retention_hours: u64,
    pub undo_max_hours: u64,
    pub max_entries_per_account: u32,
    pub max_record_bytes: usize,
}
impl Default for JournalConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            retention_hours: 504,
            undo_max_hours: 24,
            max_entries_per_account: 2048,
            max_record_bytes: 8192,
        }
    }
}
impl JournalConfig {
    pub fn validate(&self) -> Result<(), StoreError> {
        if !(1..=876000).contains(&self.retention_hours)
            || !(1..=self.retention_hours).contains(&self.undo_max_hours)
            || !(100..=100000).contains(&self.max_entries_per_account)
            || !(4096..=32768).contains(&self.max_record_bytes)
        {
            return Err(StoreError::InvalidInput);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationRecord {
    pub version: u8,
    pub id: String,
    pub request_id: crate::protocol::RequestId,
    pub account_id: String,
    pub generation: u64,
    pub created_at: u64,
    pub updated_at: u64,
    pub action: String,
    pub root_id: crate::config::RootId,
    pub source_path: crate::config::RelativePath,
    pub target_path: Option<crate::config::RelativePath>,
    pub requested_mode: Option<u32>,
    pub status: String,
    pub error_code: Option<crate::errors::ErrorKind>,
    pub bytes_read: u64,
    pub bytes_written: u64,
}
fn prefix(id: &str) -> String {
    format!("operation_v1:{id}:")
}
fn key(record: &OperationRecord) -> String {
    format!(
        "{}{:020}:{}",
        prefix(&record.account_id),
        record.created_at,
        record.id
    )
}
pub fn action(command: HelperCommand) -> &'static str {
    use HelperCommand::*;
    match command {
        WriteFile => "write",
        UploadCommit => "upload",
        CreateFolder => "createFolder",
        Rename => "rename",
        Move => "move",
        Copy => "copy",
        MoveToTrash => "trash",
        RestoreFromTrash => "restore",
        DeletePermanently => "delete",
        Chmod => "chmod",
        CompressZip | CompressTarGz => "archive",
        ExtractArchive => "extract",
        ReadTextFile => "readText",
        DownloadFile => "download",
        _ => "read",
    }
}
pub fn tracked(command: HelperCommand) -> bool {
    matches!(
        command,
        HelperCommand::WriteFile
            | HelperCommand::UploadCommit
            | HelperCommand::CreateFolder
            | HelperCommand::Rename
            | HelperCommand::Move
            | HelperCommand::Copy
            | HelperCommand::MoveToTrash
            | HelperCommand::RestoreFromTrash
            | HelperCommand::DeletePermanently
            | HelperCommand::Chmod
            | HelperCommand::CompressZip
            | HelperCommand::CompressTarGz
            | HelperCommand::ExtractArchive
            | HelperCommand::ReadTextFile
            | HelperCommand::DownloadFile
    )
}
fn encode(record: &OperationRecord, config: &JournalConfig) -> Result<String, StoreError> {
    let data = serde_json::to_string(record).map_err(|_| StoreError::InvalidInput)?;
    if data.len() > config.max_record_bytes {
        return Err(StoreError::InvalidInput);
    }
    Ok(data)
}
impl AccountStore {
    pub fn begin_operation(
        &mut self,
        account: &Account,
        request: &HelperRequest,
        config: &JournalConfig,
        at: u64,
    ) -> Result<OperationRecord, StoreError> {
        config.validate()?;
        if !config.enabled
            || !tracked(request.command)
            || request.uid != account.uid
            || request.gid != account.gid
            || request.username != account.username
            || request.relative_path.0.len() > 4096
            || request
                .target_relative_path
                .as_ref()
                .is_some_and(|v| v.0.len() > 4096)
        {
            return Err(StoreError::InvalidInput);
        }
        let authority = Authority::Account {
            id: account.id.clone(),
            generation: account.generation,
        };
        let actor = authorized(&self.connection, &authority)?.ok_or(StoreError::Forbidden)?;
        let record = OperationRecord {
            version: 1,
            id: uuid::Uuid::new_v4().to_string(),
            request_id: request.request_id,
            account_id: actor.id.clone(),
            generation: actor.generation,
            created_at: at,
            updated_at: at,
            action: action(request.command).into(),
            root_id: request.root_id.clone(),
            source_path: request.relative_path.clone(),
            target_path: if request.command == HelperCommand::Rename {
                request.target_relative_path.as_ref().map(|target| {
                    crate::config::RelativePath::new(
                        match request.relative_path.0.rsplit_once('/') {
                            Some((parent, _)) if !parent.is_empty() => {
                                format!("{parent}/{}", target.0)
                            }
                            _ => target.0.clone(),
                        },
                    )
                })
            } else {
                request.target_relative_path.clone()
            },
            requested_mode: request.chmod_mode.map(|m| m.0),
            status: "pending".into(),
            error_code: None,
            bytes_read: 0,
            bytes_written: 0,
        };
        let data = encode(&record, config)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let pattern = format!("{}%", prefix(&actor.id));
        tx.execute("DELETE FROM security_state WHERE key IN (SELECT key FROM security_state WHERE key LIKE ?1 AND json_extract(value,'$.createdAt')<?2 LIMIT 2000)",params![pattern,number(at.saturating_sub(config.retention_hours*3600))?])?;
        let count: i64 = tx.query_row(
            "SELECT count(*) FROM security_state WHERE key LIKE ?1",
            [&pattern],
            |r| r.get(0),
        )?;
        if count >= i64::from(config.max_entries_per_account) {
            return Err(StoreError::QuotaExceeded);
        }
        tx.execute(
            "INSERT INTO security_state(key,value) VALUES(?1,?2)",
            params![key(&record), data],
        )?;
        tx.commit()?;
        Ok(record)
    }
    pub fn finish_operation(
        &mut self,
        record: &OperationRecord,
        response: Option<&HelperResponse>,
        config: &JournalConfig,
        at: u64,
    ) -> Result<OperationRecord, StoreError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let data: Option<String> = tx
            .query_row(
                "SELECT value FROM security_state WHERE key=?1",
                [key(record)],
                |r| r.get(0),
            )
            .optional()?;
        let mut current: OperationRecord = serde_json::from_str(&data.ok_or(StoreError::NotFound)?)
            .map_err(|_| StoreError::InvalidInput)?;
        if current.version != 1
            || current.id != record.id
            || current.account_id != record.account_id
            || current.status != "pending"
        {
            return Err(StoreError::Conflict);
        }
        current.updated_at = at;
        current.status = match response {
            Some(r) if r.success => "success",
            Some(_) => "failed",
            None => "unknown",
        }
        .into();
        if let Some(r) = response {
            current.error_code = r.error_kind;
            current.bytes_read = r.bytes_read;
            current.bytes_written = r.bytes_written;
        }
        tx.execute(
            "UPDATE security_state SET value=?1 WHERE key=?2",
            params![encode(&current, config)?, key(record)],
        )?;
        tx.commit()?;
        Ok(current)
    }
    pub fn operation_records(
        &self,
        authority: &Authority,
        limit: u16,
        cursor: Option<&str>,
        filter: Option<&str>,
    ) -> Result<Vec<OperationRecord>, StoreError> {
        if !(1..=200).contains(&limit)
            || cursor.is_some_and(|v| v.len() > 160 || v.chars().any(char::is_control))
            || filter.is_some_and(|v| v.len() > 32 || !v.chars().all(|c| c.is_ascii_alphanumeric()))
        {
            return Err(StoreError::InvalidInput);
        }
        let actor = authorized(&self.connection, authority)?.ok_or(StoreError::Forbidden)?;
        let pattern = format!("{}%", prefix(&actor.id));
        let mut statement=self.connection.prepare("SELECT value FROM security_state WHERE key LIKE ?1 AND (?2 IS NULL OR key<?2) AND (?3 IS NULL OR json_extract(value,'$.action')=?3) ORDER BY key DESC LIMIT ?4")?;
        let records = statement
            .query_map(params![pattern, cursor, filter, i64::from(limit)], |r| {
                r.get::<_, String>(0)
            })?;
        records
            .map(|data| {
                let record: OperationRecord =
                    serde_json::from_str(&data?).map_err(|_| StoreError::InvalidInput)?;
                if record.version != 1 || record.account_id != actor.id {
                    return Err(StoreError::Forbidden);
                }
                Ok(record)
            })
            .collect()
    }
    pub fn operation_cursor(record: &OperationRecord) -> String {
        key(record)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::store::{AccountKind, NewAccount, Role};
    fn account(store: &mut AccountStore, name: &str, uid: u32) -> Account {
        store
            .create(
                &Authority::Root,
                NewAccount {
                    username: name.into(),
                    uid,
                    gid: uid,
                    home: format!("/home/{name}").into(),
                    kind: AccountKind::Linux,
                    role: Role::User,
                    owner_admin_id: None,
                    quota_bytes: None,
                },
            )
            .unwrap()
    }
    #[test]
    fn intent_result_scope_and_secret_exclusion() {
        let mut store = AccountStore::memory();
        let alice = account(&mut store, "alice", 1000);
        let other = account(&mut store, "bob", 1001);
        let config = JournalConfig {
            enabled: true,
            ..Default::default()
        };
        let mut request =
            crate::helper::ipc::tests_support::sample_request(HelperCommand::WriteFile);
        request.write_options = Some(crate::helper::types::HelperWriteOptions {
            data: b"PRIVATE_CONTENT_NEVER_LOG".to_vec(),
            expected_revision: None,
            offset: 0,
            truncate: true,
        });
        let pending = store
            .begin_operation(&alice, &request, &config, 1000)
            .unwrap();
        let owner = Authority::Account {
            id: alice.id.clone(),
            generation: alice.generation,
        };
        assert_eq!(
            store.operation_records(&owner, 10, None, None).unwrap()[0].status,
            "pending"
        );
        let response = HelperResponse::operation_done(&request, "done");
        let done = store
            .finish_operation(&pending, Some(&response), &config, 1001)
            .unwrap();
        assert_eq!(done.status, "success");
        assert!(!serde_json::to_string(&done)
            .unwrap()
            .contains("PRIVATE_CONTENT"));
        assert!(matches!(
            store.finish_operation(&pending, Some(&response), &config, 1002),
            Err(StoreError::Conflict)
        ));
        let stranger = Authority::Account {
            id: other.id,
            generation: other.generation,
        };
        assert!(store
            .operation_records(
                &stranger,
                10,
                Some(&AccountStore::operation_cursor(&done)),
                None
            )
            .unwrap()
            .is_empty());
        assert!(store
            .operation_records(&owner, 10, None, Some("move"))
            .unwrap()
            .is_empty());
    }
    #[test]
    fn interrupted_result_is_unknown_and_full_journal_refuses_new_intent() {
        let mut store = AccountStore::memory();
        let alice = account(&mut store, "alice", 1000);
        let config = JournalConfig {
            enabled: true,
            max_entries_per_account: 100,
            ..Default::default()
        };
        let request = crate::helper::ipc::tests_support::sample_request(HelperCommand::Rename);
        let first = store
            .begin_operation(&alice, &request, &config, 1000)
            .unwrap();
        assert_eq!(
            store
                .finish_operation(&first, None, &config, 1001)
                .unwrap()
                .status,
            "unknown"
        );
        for _ in 1..100 {
            store
                .begin_operation(&alice, &request, &config, 1001)
                .unwrap();
        }
        assert!(matches!(
            store.begin_operation(&alice, &request, &config, 1002),
            Err(StoreError::QuotaExceeded)
        ));
        store
            .begin_operation(
                &alice,
                &request,
                &config,
                1000 + config.retention_hours * 3600 + 1,
            )
            .unwrap();
    }
}
