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
    pub max_total_entries: u32,
    pub max_record_bytes: usize,
}
impl Default for JournalConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            retention_hours: 504,
            undo_max_hours: 24,
            max_entries_per_account: 2048,
            max_total_entries: 32768,
            max_record_bytes: 8192,
        }
    }
}
impl JournalConfig {
    pub fn validate(&self) -> Result<(), StoreError> {
        if !(1..=876000).contains(&self.retention_hours)
            || !(1..=self.retention_hours).contains(&self.undo_max_hours)
            || !(100..=100000).contains(&self.max_entries_per_account)
            || !(self.max_entries_per_account..=1000000).contains(&self.max_total_entries)
            || !(4096..=32768).contains(&self.max_record_bytes)
        {
            return Err(StoreError::InvalidInput);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryQuery {
    pub limit: u16,
    pub cursor: Option<String>,
    pub action: Option<String>,
    pub request_id: Option<String>,
    pub status: Option<String>,
    pub root_id: Option<String>,
    pub path: Option<String>,
    pub from: Option<u64>,
    pub to: Option<u64>,
}
impl Default for HistoryQuery {
    fn default() -> Self {
        Self {
            limit: 100,
            cursor: None,
            action: None,
            request_id: None,
            status: None,
            root_id: None,
            path: None,
            from: None,
            to: None,
        }
    }
}
impl HistoryQuery {
    pub fn validate(&self) -> Result<(), StoreError> {
        if !(1..=200).contains(&self.limit)
            || self
                .cursor
                .as_ref()
                .is_some_and(|v| v.len() > 160 || v.chars().any(char::is_control))
            || self
                .action
                .as_ref()
                .is_some_and(|v| v.len() > 32 || !v.chars().all(|c| c.is_ascii_alphanumeric()))
            || self
                .request_id
                .as_ref()
                .is_some_and(|value| uuid::Uuid::parse_str(value).is_err())
            || self
                .status
                .as_ref()
                .is_some_and(|v| !["pending", "success", "failed", "unknown"].contains(&v.as_str()))
            || self
                .root_id
                .as_ref()
                .is_some_and(|v| v.len() > 128 || v.chars().any(char::is_control))
            || self
                .path
                .as_ref()
                .is_some_and(|v| v.len() > 256 || v.chars().any(char::is_control))
            || self.from.is_some_and(|v| v > i64::MAX as u64)
            || self.to.is_some_and(|v| v > i64::MAX as u64)
            || self.from.zip(self.to).is_some_and(|(from, to)| from > to)
        {
            return Err(StoreError::InvalidInput);
        }
        Ok(())
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub items: Vec<OperationRecord>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
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
    #[serde(default)]
    pub old_mode: Option<u32>,
    #[serde(default)]
    pub new_mode: Option<u32>,
    #[serde(default)]
    pub before: Option<crate::helper::observation::FileState>,
    #[serde(default)]
    pub after: Option<crate::helper::observation::FileState>,
    pub status: String,
    pub error_code: Option<crate::errors::ErrorKind>,
    pub bytes_read: u64,
    pub bytes_written: u64,
    #[serde(default)]
    pub log_reserve_bytes: u64,
    #[serde(default)]
    pub log_storage: Option<String>,
    #[serde(default)]
    pub undo_epoch: Option<String>,
    #[serde(default)]
    pub undo_status: Option<String>,
    #[serde(default)]
    pub undo_error_code: Option<crate::errors::ErrorKind>,
    #[serde(default)]
    pub staging_path: Option<crate::config::RelativePath>,
    #[serde(default)]
    pub trash_gc_status: Option<String>,
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
        let operation_id = uuid::Uuid::new_v4().to_string();
        let destination = if matches!(
            request.command,
            HelperCommand::WriteFile | HelperCommand::UploadCommit
        ) {
            Some(&request.relative_path)
        } else {
            request.target_relative_path.as_ref()
        };
        let staging_path = if matches!(
            request.command,
            HelperCommand::WriteFile
                | HelperCommand::UploadCommit
                | HelperCommand::Copy
                | HelperCommand::CompressZip
                | HelperCommand::CompressTarGz
                | HelperCommand::ExtractArchive
        ) {
            destination.map(|path| {
                let parent = path.0.rsplit_once('/').map_or("", |(parent, _)| parent);
                let suffix = match request.command {
                    HelperCommand::CompressZip => ".zip",
                    HelperCommand::CompressTarGz => ".tar.gz",
                    _ => "",
                };
                crate::config::RelativePath::new(format!(
                    "{}.tdrv-transaction-{operation_id}{suffix}",
                    if parent.is_empty() {
                        String::new()
                    } else {
                        format!("{parent}/")
                    }
                ))
            })
        } else {
            None
        };
        let record = OperationRecord {
            version: 1,
            id: operation_id.clone(),
            request_id: request.request_id,
            account_id: actor.id.clone(),
            generation: actor.generation,
            created_at: at,
            updated_at: at,
            action: action(request.command).into(),
            root_id: request.root_id.clone(),
            source_path: request.relative_path.clone(),
            target_path: if request.command == HelperCommand::MoveToTrash {
                Some(crate::config::RelativePath::new(format!(
                    ".tealdrive-trash/files/{operation_id}"
                )))
            } else if request.command == HelperCommand::Rename {
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
            old_mode: None,
            new_mode: None,
            before: None,
            after: None,
            status: "pending".into(),
            error_code: None,
            bytes_read: 0,
            bytes_written: 0,
            log_reserve_bytes: 0,
            log_storage: Some("systemBudget".into()),
            undo_epoch: Some(super::undo::epoch(&self.connection)?),
            undo_status: None,
            undo_error_code: None,
            staging_path,
            trash_gc_status: None,
        };
        let data = encode(&record, config)?;
        // Reserve bounded after-state metadata before the filesystem mutation begins.
        if data.len().saturating_add(2048) > config.max_record_bytes {
            return Err(StoreError::InvalidInput);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let pattern = format!("{}*", prefix(&actor.id));
        let expired: Vec<(String, String)> = {
            let mut query=tx.prepare("SELECT key,value FROM security_state WHERE key GLOB ?1 AND json_extract(value,'$.createdAt')<?2 LIMIT 2000")?;
            let rows = query.query_map(
                params![
                    pattern,
                    number(at.saturating_sub(config.retention_hours * 3600))?
                ],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        for (key, value) in expired {
            let old: OperationRecord =
                serde_json::from_str(&value).map_err(|_| StoreError::InvalidInput)?;
            super::ledger::remove_record(&actor, &old)?;
            tx.execute("DELETE FROM security_state WHERE key=?1", [key])?;
        }
        let count: i64 = tx.query_row(
            "SELECT count(*) FROM security_state WHERE key GLOB ?1",
            [&pattern],
            |r| r.get(0),
        )?;
        let global: i64 = tx.query_row(
            "SELECT count(*) FROM security_state WHERE key GLOB 'operation_v1:*'",
            [],
            |r| r.get(0),
        )?;
        if count >= i64::from(config.max_entries_per_account)
            || global >= i64::from(config.max_total_entries)
        {
            let oldest:Option<(String,String)>=tx.query_row("SELECT key,value FROM security_state WHERE key GLOB ?1 AND json_extract(value,'$.status') IN ('success','failed') AND (json_extract(value,'$.action') NOT IN ('move','rename','trash','chmod') OR json_extract(value,'$.createdAt')<?2) ORDER BY key LIMIT 1",params![pattern,number(at.saturating_sub(config.undo_max_hours*3600))?],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            let Some((old_key, value)) = oldest else {
                return Err(StoreError::QuotaExceeded);
            };
            let old: OperationRecord =
                serde_json::from_str(&value).map_err(|_| StoreError::InvalidInput)?;
            super::ledger::remove_record(&actor, &old)?;
            tx.execute("DELETE FROM security_state WHERE key=?1", [old_key])?;
        }
        tx.execute(
            "INSERT INTO security_state(key,value) VALUES(?1,?2)",
            params![key(&record), data],
        )?;
        if record.action == "trash" {
            let count: i64 = tx.query_row(
                "SELECT count(*) FROM security_state WHERE key GLOB ?1",
                [format!("trash_expiry_v1:{}:*", actor.id)],
                |r| r.get(0),
            )?;
            let global: i64 = tx.query_row(
                "SELECT count(*) FROM security_state WHERE key GLOB 'trash_expiry_v1:*'",
                [],
                |r| r.get(0),
            )?;
            if count >= i64::from(config.max_entries_per_account)
                || global >= i64::from(config.max_total_entries)
            {
                return Err(StoreError::QuotaExceeded);
            }
            tx.execute(
                "INSERT INTO security_state(key,value) VALUES(?1,?2)",
                params![
                    format!("trash_expiry_v1:{}:{}", actor.id, record.id),
                    encode(&record, config)?
                ],
            )?;
        }
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
        current.log_reserve_bytes = record.log_reserve_bytes;
        current.log_storage = record.log_storage.clone();
        current.status = match response {
            Some(r) if r.success => "success",
            Some(_) => "failed",
            None => "unknown",
        }
        .into();
        if let Some(r) = response {
            if current.action == "trash" && r.success {
                if let crate::helper::types::HelperResponsePayload::TrashEntry(entry) = &r.payload {
                    if entry.trash_id != current.id {
                        return Err(StoreError::InvalidInput);
                    }
                    current.target_path = Some(crate::config::RelativePath::new(format!(
                        ".tealdrive-trash/files/{}",
                        entry.trash_id
                    )));
                }
            }
            current.error_code = r.error_kind;
            current.bytes_read = r.bytes_read;
            current.bytes_written = r.bytes_written;
            if let Some(metadata) = r.audit_metadata.as_ref() {
                current.old_mode = metadata.old_mode;
                current.new_mode = metadata.new_mode;
                current.before = metadata.before.clone();
                current.after = metadata.after.clone();
            }
        }
        tx.execute(
            "UPDATE security_state SET value=?1 WHERE key=?2",
            params![encode(&current, config)?, key(record)],
        )?;
        if current.action == "trash" {
            let token = format!("trash_expiry_v1:{}:{}", current.account_id, current.id);
            if current.status == "failed" {
                tx.execute("DELETE FROM security_state WHERE key=?1", [token])?;
            } else {
                tx.execute(
                    "UPDATE security_state SET value=?1 WHERE key=?2",
                    params![encode(&current, config)?, token],
                )?;
            }
        }
        if current.status == "success" && ["delete", "restore"].contains(&current.action.as_str()) {
            // An explicit successful trash operation retires the corresponding maintenance token.
            let supplied = record
                .source_path
                .0
                .strip_prefix(".tealdrive-trash/files/")
                .unwrap_or(&record.source_path.0);
            if let Ok(id) = uuid::Uuid::parse_str(supplied) {
                tx.execute(
                    "DELETE FROM security_state WHERE key=?1",
                    [format!("trash_expiry_v1:{}:{id}", current.account_id)],
                )?;
            }
        }
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
        self.operation_records_filtered(
            authority,
            &HistoryQuery {
                limit,
                cursor: cursor.map(str::to_owned),
                action: filter.map(str::to_owned),
                ..Default::default()
            },
        )
    }
    pub fn operation_records_filtered(
        &self,
        authority: &Authority,
        query: &HistoryQuery,
    ) -> Result<Vec<OperationRecord>, StoreError> {
        query.validate()?;
        let actor = authorized(&self.connection, authority)?.ok_or(StoreError::Forbidden)?;
        let pattern = format!("{}*", prefix(&actor.id));
        let mut statement=self.connection.prepare("SELECT value FROM security_state WHERE key GLOB ?1 AND (?2 IS NULL OR key<?2) AND (?3 IS NULL OR json_extract(value,'$.action')=?3) AND (?5 IS NULL OR json_extract(value,'$.status')=?5) AND (?6 IS NULL OR json_extract(value,'$.rootId')=?6) AND (?7 IS NULL OR instr(json_extract(value,'$.sourcePath'),?7)>0 OR instr(json_extract(value,'$.targetPath'),?7)>0) AND (?8 IS NULL OR json_extract(value,'$.createdAt')>=?8) AND (?9 IS NULL OR json_extract(value,'$.createdAt')<=?9) AND (?10 IS NULL OR json_extract(value,'$.requestId')=?10) ORDER BY key DESC LIMIT ?4")?;
        let records = statement.query_map(
            params![
                pattern,
                query.cursor,
                query.action,
                i64::from(query.limit),
                query.status,
                query.root_id,
                query.path,
                query.from.map(number).transpose()?,
                query.to.map(number).transpose()?,
                query.request_id
            ],
            |r| r.get::<_, String>(0),
        )?;
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
    pub fn gc_operations(&mut self, config: &JournalConfig, at: u64) -> Result<usize, StoreError> {
        if unsafe { libc::geteuid() } != 0 {
            return Err(StoreError::Forbidden);
        }
        config.validate()?;
        let expired: Vec<(String, String)> = {
            let mut statement=self.connection.prepare("SELECT key,value FROM security_state WHERE key GLOB 'operation_v1:*' AND json_extract(value,'$.createdAt')<?1 LIMIT 500")?;
            let rows = statement.query_map(
                [number(at.saturating_sub(config.retention_hours * 3600))?],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        let mut removed = 0;
        for (old_key, value) in expired {
            let record: OperationRecord =
                serde_json::from_str(&value).map_err(|_| StoreError::InvalidInput)?;
            if record.version != 1 || key(&record) != old_key {
                return Err(StoreError::InvalidInput);
            }
            let account = self.get(&record.account_id)?;
            super::ledger::remove_record(&account, &record)?;
            removed += self.connection.execute(
                "DELETE FROM security_state WHERE key=?1 AND value=?2",
                params![old_key, value],
            )?;
        }
        Ok(removed)
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
    #[test]
    fn status_path_root_and_time_filters_run_before_limit() {
        let mut store = AccountStore::memory();
        let alice = account(&mut store, "alice", 1000);
        let config = JournalConfig {
            enabled: true,
            ..Default::default()
        };
        let request = crate::helper::ipc::tests_support::sample_request(HelperCommand::WriteFile);
        let first = store
            .begin_operation(&alice, &request, &config, 1000)
            .unwrap();
        store
            .finish_operation(
                &first,
                Some(&HelperResponse::operation_done(&request, "ok")),
                &config,
                1001,
            )
            .unwrap();
        let second = store
            .begin_operation(&alice, &request, &config, 1050)
            .unwrap();
        store
            .finish_operation(
                &second,
                Some(&HelperResponse::permission_denied(&request, "no")),
                &config,
                1051,
            )
            .unwrap();
        let authority = Authority::Account {
            id: alice.id,
            generation: alice.generation,
        };
        let query = HistoryQuery {
            limit: 1,
            status: Some("success".into()),
            root_id: Some("home".into()),
            path: Some("docs".into()),
            from: Some(999),
            to: Some(1051),
            ..Default::default()
        };
        let records = store
            .operation_records_filtered(&authority, &query)
            .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, first.id);
        assert!(HistoryQuery {
            from: Some(10),
            to: Some(1),
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(HistoryQuery {
            status: Some("fabricated".into()),
            ..Default::default()
        }
        .validate()
        .is_err());
    }
    #[test]
    fn rotation_reclaims_completed_nonundo_events_and_keeps_other_accounts() {
        let mut store = AccountStore::memory();
        let alice = account(&mut store, "alice", 1000);
        let bob = account(&mut store, "bob", 1001);
        let config = JournalConfig {
            enabled: true,
            max_entries_per_account: 100,
            max_total_entries: 100,
            ..Default::default()
        };
        let mut request =
            crate::helper::ipc::tests_support::sample_request(HelperCommand::WriteFile);
        let response = HelperResponse::operation_done(&request, "ok");
        for i in 0..99 {
            let record = store
                .begin_operation(&alice, &request, &config, 1000 + i)
                .unwrap();
            store
                .finish_operation(&record, Some(&response), &config, 1000 + i)
                .unwrap();
        }
        request.username = "bob".into();
        request.uid = 1001;
        request.gid = 1001;
        let foreign = store
            .begin_operation(&bob, &request, &config, 1200)
            .unwrap();
        store
            .finish_operation(&foreign, Some(&response), &config, 1200)
            .unwrap();
        request.username = "alice".into();
        request.uid = 1000;
        request.gid = 1000;
        store
            .begin_operation(&alice, &request, &config, 1201)
            .unwrap();
        let own = Authority::Account {
            id: alice.id,
            generation: alice.generation,
        };
        let other = Authority::Account {
            id: bob.id,
            generation: bob.generation,
        };
        assert_eq!(
            store
                .operation_records(&own, 200, None, None)
                .unwrap()
                .len(),
            99
        );
        assert_eq!(
            store.operation_records(&other, 200, None, None).unwrap()[0].id,
            foreign.id
        );
    }
}
