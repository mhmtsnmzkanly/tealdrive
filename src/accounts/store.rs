//! Each helper operation opens a root-only database and uses short immediate transactions.
//! No network request chooses this path. Do not serialize the connection or password hashes.
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("login rate limited")]
    RateLimited,
    #[error("invalid account input")]
    InvalidInput,
    #[error("account state conflict")]
    Conflict,
    #[error("account not found")]
    NotFound,
    #[error("operation forbidden")]
    Forbidden,
    #[error("quota allocation exceeds available budget")]
    QuotaExceeded,
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("account store ownership or path is untrusted")]
    UntrustedStore,
    #[error("unsupported account schema")]
    UnsupportedSchema,
    #[error("password hash unavailable or invalid")]
    PasswordHash,
    #[error("account database unavailable")]
    Database(#[from] rusqlite::Error),
    #[error("account store I/O failed")]
    Io(#[from] std::io::Error),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    Linux,
    Virtual,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Admin,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountStatus {
    Active,
    Suspended,
    Revoked,
    Deleted,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub username: String,
    pub uid: u32,
    pub gid: u32,
    pub home: PathBuf,
    pub kind: AccountKind,
    pub role: Role,
    #[serde(default)]
    pub global_admin: bool,
    #[serde(default)]
    pub password_change_required: bool,
    pub owner_admin_id: Option<String>,
    pub status: AccountStatus,
    pub login_enabled: bool,
    pub generation: u64,
    pub password_enrolled: bool,
    pub quota_bytes: Option<u64>,
    pub undo_hours: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
    pub deleted_at: Option<u64>,
    pub deny_enrollment: bool,
}
#[derive(Debug, Clone)]
pub struct NewAccount {
    pub username: String,
    pub uid: u32,
    pub gid: u32,
    pub home: PathBuf,
    pub kind: AccountKind,
    pub role: Role,
    pub owner_admin_id: Option<String>,
    pub quota_bytes: Option<u64>,
}
// The gateway derives this from the caller's verified identity/ticket; never from JSON role claims.
#[derive(Debug, Clone)]
pub enum Authority {
    Root,
    Account { id: String, generation: u64 },
}
pub struct AccountStore {
    pub(super) connection: Connection,
}
pub(super) fn now() -> u64 {
    crate::session::store::InMemorySessionStore::now_epoch_seconds()
}
pub(super) fn number(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::InvalidInput)
}
pub(super) fn pending_reserved(
    connection: &Connection,
    owner: &str,
    exclude: &str,
) -> Result<i64, StoreError> {
    Ok(connection.query_row("SELECT coalesce(sum(CASE WHEN key LIKE 'virtual_create_v1:%' THEN CASE WHEN json_extract(value,'$.owner')=?1 AND json_extract(value,'$.account_id') IS NULL AND key!=?2 THEN json_extract(value,'$.quota') ELSE 0 END ELSE 0 END),0) FROM security_state",params![owner,exclude],|r|r.get(0))?)
}
/// Protected fixed reservation; absent keys preserve legacy remainder quotas.
pub(super) fn fixed_own(connection: &Connection, id: &str) -> Result<Option<i64>, StoreError> {
    let raw: Option<String> = connection
        .query_row(
            "SELECT value FROM security_state WHERE key=?1",
            [format!("admin_own_quota_v1:{id}")],
            |r| r.get(0),
        )
        .optional()?;
    raw.map(|raw| {
        raw.parse::<i64>()
            .ok()
            .filter(|v| *v > 0)
            .ok_or(StoreError::InvalidInput)
    })
    .transpose()
}
pub(super) fn root_parent(path: &Path) -> Result<(), StoreError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(StoreError::UntrustedStore);
    }
    let mut parent = path.parent();
    while let Some(dir) = parent {
        let m = std::fs::symlink_metadata(dir)?;
        if !m.is_dir() || m.uid() != 0 || m.mode() & 0o022 != 0 {
            return Err(StoreError::UntrustedStore);
        }
        parent = dir.parent();
    }
    Ok(())
}
fn row_u64(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    u64::try_from(row.get::<_, i64>(index)?).map_err(|_| rusqlite::Error::InvalidQuery)
}
fn row_optional_u64(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    row.get::<_, Option<i64>>(index)?
        .map(|value| u64::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery))
        .transpose()
}
pub(super) fn row_account(row: &rusqlite::Row<'_>) -> rusqlite::Result<Account> {
    let kind: String = row.get(5)?;
    let role: String = row.get(6)?;
    let status: String = row.get(8)?;
    let conversion = || rusqlite::Error::InvalidQuery;
    Ok(Account {
        id: row.get(0)?,
        username: row.get(1)?,
        uid: row.get(2)?,
        gid: row.get(3)?,
        home: PathBuf::from(row.get::<_, String>(4)?),
        kind: match kind.as_str() {
            "linux" => AccountKind::Linux,
            "virtual" => AccountKind::Virtual,
            _ => return Err(conversion()),
        },
        role: match role.as_str() {
            "user" => Role::User,
            "admin" => Role::Admin,
            _ => return Err(conversion()),
        },
        owner_admin_id: row.get(7)?,
        status: match status.as_str() {
            "active" => AccountStatus::Active,
            "suspended" => AccountStatus::Suspended,
            "revoked" => AccountStatus::Revoked,
            "deleted" => AccountStatus::Deleted,
            _ => return Err(conversion()),
        },
        login_enabled: row.get(9)?,
        generation: row_u64(row, 10)?,
        password_enrolled: row.get(11)?,
        quota_bytes: row_optional_u64(row, 12)?,
        undo_hours: row_optional_u64(row, 13)?,
        created_at: row_u64(row, 14)?,
        updated_at: row_u64(row, 15)?,
        deleted_at: row_optional_u64(row, 16)?,
        deny_enrollment: row.get(17)?,
        global_admin: row.get(18)?,
        password_change_required: row.get(19)?,
    })
}
pub(super) const COLUMNS:&str="id,username,uid,gid,home,kind,role,owner_admin_id,status,login_enabled,generation,web_password_hash IS NOT NULL,quota_bytes,undo_hours,created_at,updated_at,deleted_at,deny_enrollment,(role='admin' AND coalesce((SELECT value='global' FROM security_state WHERE key='panel_scope:'||accounts.id),kind='linux')),EXISTS(SELECT 1 FROM security_state WHERE key='password_change_required:'||accounts.id)";
fn apply_metadata_budget(connection: &Connection, budget: u64) -> Result<(), StoreError> {
    let page_size: i64 = connection.pragma_query_value(None, "page_size", |row| row.get(0))?;
    let page_size = u64::try_from(page_size).map_err(|_| StoreError::InvalidInput)?;
    if page_size == 0 {
        return Err(StoreError::InvalidInput);
    }
    let maximum = budget / page_size;
    let observed: i64 =
        connection.query_row(&format!("PRAGMA max_page_count={maximum}"), [], |row| {
            row.get(0)
        })?;
    if observed > number(maximum)? {
        return Err(StoreError::QuotaExceeded);
    }
    connection.execute_batch("PRAGMA journal_size_limit=16777216;")?;
    Ok(())
}
pub(super) fn account(connection: &Connection, id: &str) -> Result<Account, StoreError> {
    connection
        .query_row(
            &format!("SELECT {COLUMNS} FROM accounts WHERE id=?1"),
            [id],
            row_account,
        )
        .optional()?
        .ok_or(StoreError::NotFound)
}
pub(super) fn authorized(
    connection: &Connection,
    actor: &Authority,
) -> Result<Option<Account>, StoreError> {
    match actor {
        Authority::Root => Ok(None),
        Authority::Account { id, generation } => {
            let account = account(connection, id)?;
            if account.generation != *generation
                || !account.login_enabled
                || account.status != AccountStatus::Active
            {
                return Err(StoreError::Forbidden);
            }
            Ok(Some(account))
        }
    }
}
pub(super) fn manage(
    connection: &Connection,
    actor: &Authority,
    target: &Account,
) -> Result<(), StoreError> {
    if let Some(actor) = authorized(connection, actor)? {
        if actor.role != Role::Admin
            || target.owner_admin_id.as_deref() != Some(&actor.id)
            || target.kind != AccountKind::Virtual
        {
            return Err(StoreError::Forbidden);
        }
    }
    Ok(())
}
impl AccountStore {
    pub fn initialize(path: &Path) -> Result<Self, StoreError> {
        if unsafe { libc::geteuid() } != 0 {
            return Err(StoreError::UntrustedStore);
        }
        root_parent(path)?;
        // Never overwrite or silently adopt an existing file as a new store.
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)?;
        file.sync_all()?;
        let connection = Self::connect(path)?;
        let mut store = Self { connection };
        store.initialize_schema()?;
        Ok(store)
    }
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        Self::open_version(path, false)
    }
    /// Explicit root migration; normal startup never silently upgrades an account database.
    pub fn migrate(path: &Path) -> Result<Self, StoreError> {
        let mut store = Self::open_version(path, true)?;
        let version: i64 = store
            .connection
            .pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version == 1 {
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or(StoreError::UntrustedStore)?;
            let backup = path.with_file_name(format!("{name}.v1-{}.backup", uuid::Uuid::new_v4()));
            store.backup(&Authority::Root, &backup)?;
            let tx = store
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute_batch(include_str!("migrations/002.sql"))?;
            tx.commit()?;
        }
        Ok(store)
    }
    fn open_version(path: &Path, allow_previous: bool) -> Result<Self, StoreError> {
        if unsafe { libc::geteuid() } != 0 {
            return Err(StoreError::UntrustedStore);
        }
        root_parent(path)?;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)?;
        let m = file.metadata()?;
        if !m.is_file() || m.uid() != 0 || m.mode() & 0o077 != 0 {
            return Err(StoreError::UntrustedStore);
        }
        for suffix in ["-wal", "-shm", "-journal"] {
            let sidecar = PathBuf::from(format!("{}{suffix}", path.to_string_lossy()));
            match std::fs::symlink_metadata(sidecar) {
                Ok(m) if !m.is_file() || m.uid() != 0 || m.mode() & 0o077 != 0 => {
                    return Err(StoreError::UntrustedStore)
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        let connection = Self::connect(path)?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let application: i64 =
            connection.pragma_query_value(None, "application_id", |r| r.get(0))?;
        if (version != 2 && !(allow_previous && version == 1)) || application != 1413763670 {
            return Err(StoreError::UnsupportedSchema);
        }
        Ok(Self { connection })
    }
    fn connect(path: &Path) -> Result<Connection, StoreError> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NOFOLLOW
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        connection.busy_timeout(std::time::Duration::from_secs(3))?;
        connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF; PRAGMA secure_delete=ON; PRAGMA synchronous=FULL;")?;
        let budget = crate::helper::security::trusted_config()
            .map(|cfg| cfg.system_storage.max_metadata_db_bytes)
            .unwrap_or_else(|_| {
                crate::config::SystemStorageConfig::default().max_metadata_db_bytes
            });
        apply_metadata_budget(&connection, budget)?;
        Ok(connection)
    }
    fn initialize_schema(&mut self) -> Result<(), StoreError> {
        self.connection.execute_batch("PRAGMA journal_mode=WAL;")?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(include_str!("schema.sql"))?;
        let dummy = super::password::hash(&uuid::Uuid::new_v4().to_string())?;
        tx.execute(
            "INSERT INTO security_state(key,value) VALUES('dummy_password_hash',?1)",
            [dummy.as_str()],
        )?;
        tx.commit()?;
        Ok(())
    }
    /// Root-only online backup includes committed WAL data, without raw file copying.
    pub fn backup(&self, actor: &Authority, path: &Path) -> Result<(), StoreError> {
        if !matches!(actor, Authority::Root) || unsafe { libc::geteuid() } != 0 {
            return Err(StoreError::Forbidden);
        }
        root_parent(path)?;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)?;
        file.sync_all()?;
        self.connection.backup(rusqlite::MAIN_DB, path, None)?;
        file.sync_all()?;
        std::fs::File::open(path.parent().ok_or(StoreError::UntrustedStore)?)?.sync_all()?;
        Ok(())
    }
    pub fn get(&self, id: &str) -> Result<Account, StoreError> {
        account(&self.connection, id)
    }
    pub fn find_uid(&self, uid: u32) -> Result<Option<Account>, StoreError> {
        Ok(self
            .connection
            .query_row(
                &format!("SELECT {COLUMNS} FROM accounts WHERE uid=?1"),
                [uid],
                row_account,
            )
            .optional()?)
    }
    /// Root-only scope assignment. Storage identity and tenant-admin role stay intact.
    pub fn set_panel_scope(
        &mut self,
        actor: &Authority,
        id: &str,
        global: bool,
    ) -> Result<Account, StoreError> {
        if !matches!(actor, Authority::Root) {
            return Err(StoreError::Forbidden);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let target = account(&tx, id)?;
        if target.role != Role::Admin || target.status != AccountStatus::Active {
            return Err(StoreError::Forbidden);
        }
        tx.execute("INSERT INTO security_state(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![format!("panel_scope:{id}"),if global {"global"} else {"tenant"}])?;
        tx.execute(
            "UPDATE accounts SET generation=generation+1,updated_at=?1 WHERE id=?2",
            params![number(now())?, id],
        )?;
        tx.execute(
            "DELETE FROM enrollment WHERE username=?1",
            [target.username],
        )?;
        let result = account(&tx, id)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn set_role(
        &mut self,
        actor: &Authority,
        id: &str,
        role: Role,
    ) -> Result<Account, StoreError> {
        if !matches!(actor, Authority::Root) {
            return Err(StoreError::Forbidden);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let target = account(&tx, id)?;
        if target.kind != AccountKind::Linux || target.status != AccountStatus::Active {
            return Err(StoreError::Forbidden);
        }
        tx.execute(
            "UPDATE accounts SET role=?1,generation=generation+1,updated_at=?2 WHERE id=?3",
            params![
                if role == Role::Admin { "admin" } else { "user" },
                number(now())?,
                id
            ],
        )?;
        tx.execute("INSERT INTO security_state(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![format!("panel_scope:{id}"),if role == Role::Admin {"global"} else {"tenant"}])?;
        tx.execute(
            "DELETE FROM enrollment WHERE username=?1",
            [target.username],
        )?;
        let result = account(&tx, id)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn find_username(&self, username: &str) -> Result<Option<Account>, StoreError> {
        Ok(self
            .connection
            .query_row(
                &format!("SELECT {COLUMNS} FROM accounts WHERE username=?1"),
                [username],
                row_account,
            )
            .optional()?)
    }
    pub fn list(&self, actor: &Authority, kind: AccountKind) -> Result<Vec<Account>, StoreError> {
        let actor = authorized(&self.connection, actor)?;
        if actor.as_ref().is_some_and(|a| a.role != Role::Admin) {
            return Err(StoreError::Forbidden);
        }
        if actor.is_some() && kind == AccountKind::Linux {
            return Err(StoreError::Forbidden);
        }
        let kind = if kind == AccountKind::Linux {
            "linux"
        } else {
            "virtual"
        };
        let mut statement=self.connection.prepare(&format!("SELECT {COLUMNS} FROM accounts WHERE kind=?1 AND (?2 IS NULL OR owner_admin_id=?2) ORDER BY username LIMIT 1025"))?;
        let rows = statement.query_map(
            params![kind, actor.as_ref().map(|a| a.id.as_str())],
            row_account,
        )?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
    pub fn create(&mut self, actor: &Authority, new: NewAccount) -> Result<Account, StoreError> {
        self.create_record(actor, new, None, None)
    }
    pub(crate) fn create_enrolled(
        &mut self,
        actor: &Authority,
        new: NewAccount,
        password: &str,
        request_state: Option<(&str, &str)>,
    ) -> Result<Account, StoreError> {
        let hash = super::password::hash(password)?;
        self.create_record(actor, new, Some(hash), request_state)
    }
    fn create_record(
        &mut self,
        actor: &Authority,
        new: NewAccount,
        hash: Option<zeroize::Zeroizing<String>>,
        request_state: Option<(&str, &str)>,
    ) -> Result<Account, StoreError> {
        if new.username.is_empty()
            || new.username.len() > 256
            || new.username.contains(['/', '\\', '\0'])
            || new.uid == 0
            || new.gid == 0
            || !new.home.is_absolute()
            || new.home == Path::new("/")
            || new
                .home
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(StoreError::InvalidInput);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let caller = authorized(&tx, actor)?;
        if caller.is_some()
            && (new.kind != AccountKind::Virtual
                || new.role != Role::User
                || new.owner_admin_id.as_deref() != caller.as_ref().map(|a| a.id.as_str())
                || caller.as_ref().is_none_or(|a| a.role != Role::Admin))
        {
            return Err(StoreError::Forbidden);
        }
        if new.kind == AccountKind::Virtual && new.role == Role::Admin {
            if caller.is_some() || new.owner_admin_id.is_some() || new.quota_bytes.is_some() {
                return Err(StoreError::Forbidden);
            }
        } else if new.kind == AccountKind::Virtual {
            let quota = new
                .quota_bytes
                .filter(|q| *q > 0)
                .ok_or(StoreError::InvalidInput)?;
            let owner = new
                .owner_admin_id
                .as_deref()
                .ok_or(StoreError::InvalidInput)?;
            let owner_account = account(&tx, owner)?;
            if owner_account.role != Role::Admin || owner_account.status != AccountStatus::Active {
                return Err(StoreError::Forbidden);
            }
            let budget: i64 = tx
                .query_row(
                    "SELECT limit_bytes FROM admin_budgets WHERE account_id=?1",
                    [owner],
                    |r| r.get(0),
                )
                .optional()?
                .ok_or(StoreError::QuotaExceeded)?;
            let allocated: i64 = tx.query_row(
                "SELECT coalesce(sum(quota_bytes),0) FROM accounts WHERE owner_admin_id=?1",
                [owner],
                |r| r.get(0),
            )?;
            let pending = pending_reserved(&tx, owner, request_state.map_or("", |(key, _)| key))?;
            if number(quota)? > budget - allocated - pending - fixed_own(&tx, owner)?.unwrap_or(0) {
                return Err(StoreError::QuotaExceeded);
            }
        } else if new.owner_admin_id.is_some() || new.quota_bytes.is_some() {
            return Err(StoreError::InvalidInput);
        }
        let root_budget = if new.kind == AccountKind::Virtual && new.role == Role::Admin {
            if let Some((key, state)) = request_state {
                let state: serde_json::Value =
                    serde_json::from_str(state).map_err(|_| StoreError::InvalidInput)?;
                if state["owner"] != "__root__" || !key.starts_with("virtual_create_v1:__root__:") {
                    return Err(StoreError::Forbidden);
                }
                let budget = state["quota"].as_u64().ok_or(StoreError::InvalidInput)?;
                let pool: i64 =
                    tx.query_row("SELECT limit_bytes FROM quota_pool WHERE id=1", [], |r| {
                        r.get(0)
                    })?;
                let allocated: i64 = tx.query_row(
                    "SELECT coalesce(sum(limit_bytes),0) FROM admin_budgets",
                    [],
                    |r| r.get(0),
                )?;
                if number(budget)? > pool - allocated - pending_reserved(&tx, "__root__", key)? {
                    return Err(StoreError::QuotaExceeded);
                }
                Some(budget)
            } else {
                None
            }
        } else {
            None
        };
        let id = uuid::Uuid::new_v4().to_string();
        let time = now();
        tx.execute("INSERT INTO accounts(id,username,uid,gid,home,kind,role,owner_admin_id,quota_bytes,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10)",params![id,new.username,new.uid,new.gid,new.home.to_str().ok_or(StoreError::InvalidInput)?,if new.kind==AccountKind::Linux{"linux"}else{"virtual"},if new.role==Role::Admin{"admin"}else{"user"},new.owner_admin_id,new.quota_bytes.map(number).transpose()?,number(time)?])?;
        if let Some(budget) = root_budget {
            tx.execute(
                "INSERT INTO admin_budgets(account_id,limit_bytes) VALUES(?1,?2)",
                params![id, number(budget)?],
            )?;
        }
        if let Some(hash) = hash {
            tx.execute(
                "UPDATE accounts SET web_password_hash=?1 WHERE id=?2",
                params![hash.as_str(), id],
            )?;
            // An admin chose this password and knows it; the user must replace it.
            if caller.is_some() {
                tx.execute(
                    "INSERT INTO security_state(key,value) VALUES(?1,'1')",
                    [format!("password_change_required:{id}")],
                )?;
            }
        }
        if let Some((key, state)) = request_state {
            let mut state: serde_json::Value =
                serde_json::from_str(state).map_err(|_| StoreError::InvalidInput)?;
            state["account_id"] = serde_json::Value::String(id.clone());
            tx.execute("INSERT INTO security_state(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,state.to_string()])?;
        }
        let result = account(&tx, &id)?;
        tx.commit()?;
        Ok(result)
    }
    /// Allocation only. Actual logical usage is always measured by the helper.
    pub fn effective_quota(&self, id: &str) -> Result<Option<u64>, StoreError> {
        let target = self.get(id)?;
        if target.kind == AccountKind::Linux {
            return Ok(None);
        }
        if target.role == Role::User {
            return Ok(target.quota_bytes);
        }
        // One statement observes one SQLite snapshot: concurrent budget/allocation
        // updates cannot combine an old budget with a newer child reservation.
        let (budget, allocated, pending, own): (i64, i64, i64, Option<String>) = self.connection.query_row(
            "SELECT coalesce((SELECT limit_bytes FROM admin_budgets WHERE account_id=?1),0), coalesce((SELECT sum(quota_bytes) FROM accounts WHERE owner_admin_id=?1),0), (SELECT coalesce(sum(CASE WHEN key LIKE 'virtual_create_v1:%' THEN CASE WHEN json_extract(value,'$.owner')=?1 AND json_extract(value,'$.account_id') IS NULL THEN json_extract(value,'$.quota') ELSE 0 END ELSE 0 END),0) FROM security_state), (SELECT value FROM security_state WHERE key='admin_own_quota_v1:'||?1)",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
        let allocated = allocated
            .checked_add(pending)
            .ok_or(StoreError::QuotaExceeded)?;
        if allocated > budget {
            return Err(StoreError::QuotaExceeded);
        }
        let own = own
            .map(|v| {
                v.parse::<i64>()
                    .ok()
                    .filter(|v| *v > 0)
                    .ok_or(StoreError::InvalidInput)
            })
            .transpose()?;
        if own.is_some_and(|v| v > budget - allocated) {
            return Err(StoreError::QuotaExceeded);
        }
        Ok(Some(own.unwrap_or(budget - allocated) as u64))
    }
    pub fn set_pool(&mut self, actor: &Authority, bytes: u64) -> Result<(), StoreError> {
        if !matches!(actor, Authority::Root) {
            return Err(StoreError::Forbidden);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let allocated: i64 = tx.query_row(
            "SELECT coalesce(sum(limit_bytes),0) FROM admin_budgets",
            [],
            |r| r.get(0),
        )?;
        let allocated = allocated
            .checked_add(pending_reserved(&tx, "__root__", "")?)
            .ok_or(StoreError::QuotaExceeded)?;
        let bytes = number(bytes)?;
        if bytes < allocated {
            return Err(StoreError::QuotaExceeded);
        }
        tx.execute("UPDATE quota_pool SET limit_bytes=?1 WHERE id=1", [bytes])?;
        tx.commit()?;
        Ok(())
    }
    pub fn set_admin_budget(
        &mut self,
        actor: &Authority,
        id: &str,
        bytes: u64,
    ) -> Result<(), StoreError> {
        self.set_admin_budget_with(actor, id, bytes, |_| Ok(()))
    }
    pub(super) fn set_admin_budget_with(
        &mut self,
        actor: &Authority,
        id: &str,
        bytes: u64,
        mut before_commit: impl FnMut(u64) -> Result<(), StoreError>,
    ) -> Result<(), StoreError> {
        if !matches!(actor, Authority::Root) {
            return Err(StoreError::Forbidden);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let target = account(&tx, id)?;
        if target.role != Role::Admin || target.status != AccountStatus::Active {
            return Err(StoreError::Forbidden);
        }
        let current: i64 = tx
            .query_row(
                "SELECT limit_bytes FROM admin_budgets WHERE account_id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0);
        let pool: i64 = tx.query_row("SELECT limit_bytes FROM quota_pool WHERE id=1", [], |r| {
            r.get(0)
        })?;
        let allocated: i64 = tx.query_row(
            "SELECT coalesce(sum(limit_bytes),0) FROM admin_budgets",
            [],
            |r| r.get(0),
        )?;
        let children: i64 = tx.query_row(
            "SELECT coalesce(sum(quota_bytes),0) FROM accounts WHERE owner_admin_id=?1",
            [id],
            |r| r.get(0),
        )?;
        let children = children
            .checked_add(pending_reserved(&tx, id, "")?)
            .ok_or(StoreError::QuotaExceeded)?;
        let bytes = number(bytes)?;
        let own = fixed_own(&tx, id)?;
        if bytes
            < children
                .checked_add(own.unwrap_or(0))
                .ok_or(StoreError::QuotaExceeded)?
            || bytes > pool - (allocated - current) - pending_reserved(&tx, "__root__", "")?
        {
            return Err(StoreError::QuotaExceeded);
        }
        before_commit(own.unwrap_or(bytes - children) as u64)?;
        tx.execute("INSERT INTO admin_budgets(account_id,limit_bytes) VALUES(?1,?2) ON CONFLICT(account_id) DO UPDATE SET limit_bytes=excluded.limit_bytes",params![id,bytes])?;
        tx.commit()?;
        Ok(())
    }
    /// Root-only fixed own-file reservation. Callback checks real usage and
    /// restricts native quota before committing; increases are applied afterwards.
    pub(super) fn set_admin_own_with(
        &mut self,
        actor: &Authority,
        id: &str,
        bytes: u64,
        before: impl FnOnce(u64) -> Result<(), StoreError>,
    ) -> Result<(), StoreError> {
        if !matches!(actor, Authority::Root) {
            return Err(StoreError::Forbidden);
        }
        let bytes = number(bytes)?;
        if bytes == 0 {
            return Err(StoreError::InvalidInput);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let target = account(&tx, id)?;
        if target.kind != AccountKind::Virtual
            || target.role != Role::Admin
            || target.status != AccountStatus::Active
        {
            return Err(StoreError::Forbidden);
        }
        let (budget, children): (i64,i64) = tx.query_row(
            "SELECT coalesce((SELECT limit_bytes FROM admin_budgets WHERE account_id=?1),0),coalesce((SELECT sum(quota_bytes) FROM accounts WHERE owner_admin_id=?1),0)",
            [id], |r| Ok((r.get(0)?,r.get(1)?)))?;
        if bytes > budget - children - pending_reserved(&tx, id, "")? {
            return Err(StoreError::QuotaExceeded);
        }
        before(bytes as u64)?;
        tx.execute("INSERT INTO security_state(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![format!("admin_own_quota_v1:{id}"),bytes.to_string()])?;
        tx.execute("INSERT INTO security_state(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![format!("virtual_quota_sync_v1:{id}"),serde_json::json!({"version":1,"account_id":id}).to_string()])?;
        // Quota-bearing grants must be refreshed after own-limit changes.
        tx.execute(
            "UPDATE accounts SET generation=generation+1,updated_at=?1 WHERE id=?2",
            params![number(now())?, id],
        )?;
        tx.commit()?;
        Ok(())
    }
    /// Offline restore never resurrects enrollment proofs or browser grants.
    pub fn invalidate_restored_sessions(&mut self, actor: &Authority) -> Result<(), StoreError> {
        if !matches!(actor, Authority::Root) {
            return Err(StoreError::Forbidden);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("DELETE FROM session_grants", [])?;
        tx.execute("DELETE FROM enrollment", [])?;
        tx.execute("INSERT INTO security_state(key,value) VALUES('undo_epoch_v1',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [uuid::Uuid::new_v4().to_string()])?;
        tx.execute(
            "UPDATE accounts SET generation=generation+1,updated_at=?1",
            [number(now())?],
        )?;
        tx.commit()?;
        Ok(())
    }
    /// Offline Root migration preserves the password hash without exposing it.
    pub(crate) fn copy_admin_credentials(
        &mut self,
        actor: &Authority,
        source: &str,
        source_generation: u64,
        target: &str,
        target_generation: u64,
    ) -> Result<(), StoreError> {
        if !matches!(actor, Authority::Root) {
            return Err(StoreError::Forbidden);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let from = account(&tx, source)?;
        let to = account(&tx, target)?;
        if from.kind != AccountKind::Linux
            || to.kind != AccountKind::Virtual
            || from.role != Role::Admin
            || to.role != Role::Admin
            || from.status != AccountStatus::Active
            || to.status != AccountStatus::Active
            || !from.password_enrolled
            || from.generation != source_generation
            || to.generation != target_generation
        {
            return Err(StoreError::Forbidden);
        }
        tx.execute("UPDATE accounts SET web_password_hash=(SELECT web_password_hash FROM accounts WHERE id=?1),generation=generation+1,updated_at=?2 WHERE id=?3",params![source,number(now())?,target])?;
        tx.execute(
            "DELETE FROM security_state WHERE key=?1",
            [format!("password_change_required:{target}")],
        )?;
        if from.password_change_required {
            tx.execute(
                "INSERT INTO security_state(key,value) VALUES(?1,'1')",
                [format!("password_change_required:{target}")],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    /// Root marks an active account for a restricted first-login session.
    pub fn require_password_change(
        &mut self,
        actor: &Authority,
        id: &str,
    ) -> Result<Account, StoreError> {
        if !matches!(actor, Authority::Root) {
            return Err(StoreError::Forbidden);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let target = account(&tx, id)?;
        if target.status != AccountStatus::Active {
            return Err(StoreError::Forbidden);
        }
        tx.execute("INSERT INTO security_state(key,value) VALUES(?1,'1') ON CONFLICT(key) DO UPDATE SET value='1'",[format!("password_change_required:{id}")])?;
        tx.execute(
            "UPDATE accounts SET generation=generation+1,updated_at=?1 WHERE id=?2",
            params![number(now())?, id],
        )?;
        let result = account(&tx, id)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn set_password(
        &mut self,
        actor: &Authority,
        id: &str,
        password: &str,
    ) -> Result<Account, StoreError> {
        let hash = super::password::hash(password)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let target = account(&tx, id)?;
        if let Some(caller) = authorized(&tx, actor)? {
            if caller.id != target.id {
                manage(&tx, actor, &target)?;
            }
        }
        if target.status == AccountStatus::Deleted {
            return Err(StoreError::Forbidden);
        }
        if target.password_change_required {
            let old: zeroize::Zeroizing<String> = zeroize::Zeroizing::new(tx.query_row(
                "SELECT web_password_hash FROM accounts WHERE id=?1",
                [id],
                |r| r.get(0),
            )?);
            if super::password::verify(password, &old)? {
                return Err(StoreError::InvalidInput);
            }
        }
        tx.execute(
            "DELETE FROM security_state WHERE key=?1",
            [format!("password_change_required:{id}")],
        )?;
        tx.execute("UPDATE accounts SET web_password_hash=?1,generation=generation+1,updated_at=?2 WHERE id=?3",params![hash.as_str(),number(now())?,id])?;
        let result = account(&tx, id)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn verify_password(&self, username: &str, password: &str) -> Result<Account, StoreError> {
        let found = self.find_username(username)?;
        let hash:zeroize::Zeroizing<String>=zeroize::Zeroizing::new(match found.as_ref(){Some(a)=>self.connection.query_row("SELECT coalesce(web_password_hash,(SELECT value FROM security_state WHERE key='dummy_password_hash')) FROM accounts WHERE id=?1",[&a.id],|r|r.get(0))?,None=>self.connection.query_row("SELECT value FROM security_state WHERE key='dummy_password_hash'",[],|r|r.get(0))?});
        let valid = super::password::verify(password, &hash)?;
        match found {
            Some(a)
                if valid
                    && a.password_enrolled
                    && a.login_enabled
                    && a.status == AccountStatus::Active =>
            {
                Ok(a)
            }
            _ => Err(StoreError::InvalidCredentials),
        }
    }
    pub fn transition(
        &mut self,
        actor: &Authority,
        id: &str,
        status: AccountStatus,
        allow_admin_delete: bool,
    ) -> Result<Account, StoreError> {
        self.transition_checked(actor, id, status, allow_admin_delete, None)
    }
    pub(crate) fn transition_expected(
        &mut self,
        actor: &Authority,
        id: &str,
        status: AccountStatus,
        allow_admin_delete: bool,
        generation: u64,
    ) -> Result<Account, StoreError> {
        self.transition_checked(actor, id, status, allow_admin_delete, Some(generation))
    }
    fn transition_checked(
        &mut self,
        actor: &Authority,
        id: &str,
        status: AccountStatus,
        allow_admin_delete: bool,
        generation: Option<u64>,
    ) -> Result<Account, StoreError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let target = account(&tx, id)?;
        if generation.is_some_and(|generation| generation != target.generation) {
            return Err(StoreError::Conflict);
        }
        if let Some(caller) = authorized(&tx, actor)? {
            let own_linux_revoke = caller.id == target.id
                && target.kind == AccountKind::Linux
                && status == AccountStatus::Revoked;
            if !own_linux_revoke {
                manage(&tx, actor, &target)?;
            }
            if status == AccountStatus::Deleted && !allow_admin_delete {
                return Err(StoreError::Forbidden);
            }
        }
        if target.status == AccountStatus::Deleted {
            return Err(StoreError::Conflict);
        }
        let self_revoke = matches!(actor,Authority::Account{id,..} if id==&target.id)
            && target.kind == AccountKind::Linux
            && status == AccountStatus::Revoked;
        let deny_enrollment = status != AccountStatus::Active && !self_revoke;
        let (state, login) = match status {
            AccountStatus::Active => ("active", true),
            AccountStatus::Suspended => ("suspended", false),
            AccountStatus::Revoked => ("revoked", false),
            AccountStatus::Deleted => ("deleted", false),
        };
        tx.execute("UPDATE accounts SET status=?1,login_enabled=?2,generation=generation+1,updated_at=?3,deleted_at=?4,deny_enrollment=?6 WHERE id=?5",params![state,login,number(now())?,if status==AccountStatus::Deleted{Some(number(now())?)}else{None},id,deny_enrollment])?;
        // An old PAM proof must not revive after administrative suspend + enable.
        tx.execute(
            "DELETE FROM enrollment WHERE username=?1",
            [&target.username],
        )?;
        let result = account(&tx, id)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn verify_generation(
        &self,
        id: &str,
        generation: u64,
        uid: u32,
        gid: u32,
    ) -> Result<Account, StoreError> {
        let account = self.get(id)?;
        if account.generation != generation
            || account.uid != uid
            || account.gid != gid
            || !account.login_enabled
            || account.status != AccountStatus::Active
        {
            return Err(StoreError::Forbidden);
        }
        Ok(account)
    }
    #[cfg(test)]
    pub(crate) fn memory() -> Self {
        let mut store = Self {
            connection: Connection::open_in_memory().unwrap(),
        };
        store
            .connection
            .execute_batch("PRAGMA foreign_keys=ON;")
            .unwrap();
        store.initialize_schema().unwrap();
        store
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_page_budget_refuses_growth_without_partial_insert() {
        let connection = Connection::open_in_memory().unwrap();
        apply_metadata_budget(&connection, 16 * 1024 * 1024).unwrap();
        connection
            .execute_batch("CREATE TABLE grows (payload BLOB)")
            .unwrap();
        let failed = connection.execute("INSERT INTO grows VALUES(zeroblob(33554432))", []);
        assert!(failed.is_err());
        let count: i64 = connection
            .query_row("SELECT count(*) FROM grows", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
        let pages: i64 = connection
            .pragma_query_value(None, "page_count", |row| row.get(0))
            .unwrap();
        let size: i64 = connection
            .pragma_query_value(None, "page_size", |row| row.get(0))
            .unwrap();
        assert!(pages * size <= 16 * 1024 * 1024);
    }
    #[test]
    fn root_scope_is_independent_of_virtual_identity_and_invalidates_old_authority() {
        let mut store = AccountStore::memory();
        let account = store
            .create(
                &Authority::Root,
                NewAccount {
                    username: "panel".into(),
                    uid: 200000,
                    gid: 200000,
                    home: "/home/tealdrive/virtual-homes/tdv_200000".into(),
                    kind: AccountKind::Virtual,
                    role: Role::Admin,
                    owner_admin_id: None,
                    quota_bytes: None,
                },
            )
            .unwrap();
        assert!(!account.global_admin);
        let old = Authority::Account {
            id: account.id.clone(),
            generation: account.generation,
        };
        assert!(matches!(
            store.set_panel_scope(&old, &account.id, true),
            Err(StoreError::Forbidden)
        ));
        let global = store
            .set_panel_scope(&Authority::Root, &account.id, true)
            .unwrap();
        assert!(global.global_admin);
        assert_eq!(global.kind, AccountKind::Virtual);
        assert_eq!(global.uid, account.uid);
        assert_eq!(global.home, account.home);
        assert_eq!(global.generation, account.generation + 1);
        assert!(authorized(&store.connection, &old).is_err());
        let authority = Authority::Account {
            id: global.id.clone(),
            generation: global.generation,
        };
        let tenant = store
            .set_panel_scope(&Authority::Root, &account.id, false)
            .unwrap();
        assert!(!tenant.global_admin);
        assert_eq!(tenant.role, Role::Admin);
        assert!(authorized(&store.connection, &authority).is_err());
        let ordinary = store
            .create(&Authority::Root, linux("ordinary", 1000, Role::User))
            .unwrap();
        assert!(matches!(
            store.set_panel_scope(&Authority::Root, &ordinary.id, true),
            Err(StoreError::Forbidden)
        ));
    }
    fn linux(name: &str, uid: u32, role: Role) -> NewAccount {
        NewAccount {
            username: name.into(),
            uid,
            gid: uid,
            home: format!("/home/{name}").into(),
            kind: AccountKind::Linux,
            role,
            owner_admin_id: None,
            quota_bytes: None,
        }
    }
    fn virtual_user(name: &str, uid: u32, owner: &str, bytes: u64) -> NewAccount {
        NewAccount {
            username: name.into(),
            uid,
            gid: uid,
            home: format!("/home/tealdrive/virtual-homes/{name}").into(),
            kind: AccountKind::Virtual,
            role: Role::User,
            owner_admin_id: Some(owner.into()),
            quota_bytes: Some(bytes),
        }
    }
    fn authority(account: &Account) -> Authority {
        Authority::Account {
            id: account.id.clone(),
            generation: account.generation,
        }
    }
    #[test]
    fn root_role_change_and_restore_invalidate_prior_authority() {
        let mut store = AccountStore::memory();
        let before = store
            .create(&Authority::Root, linux("alice", 1000, Role::User))
            .unwrap();
        assert!(matches!(
            store.set_role(&authority(&before), &before.id, Role::Admin),
            Err(StoreError::Forbidden)
        ));
        let promoted = store
            .set_role(&Authority::Root, &before.id, Role::Admin)
            .unwrap();
        assert!(promoted.generation > before.generation);
        assert!(matches!(
            authorized(&store.connection, &authority(&before)),
            Err(StoreError::Forbidden)
        ));
        assert!(store
            .invalidate_restored_sessions(&authority(&promoted))
            .is_err());
        store
            .invalidate_restored_sessions(&Authority::Root)
            .unwrap();
        assert!(matches!(
            authorized(&store.connection, &authority(&promoted)),
            Err(StoreError::Forbidden)
        ));
        assert_eq!(store.get(&before.id).unwrap().role, Role::Admin);
    }
    #[test]
    fn budget_usage_callback_runs_after_authorization_and_rolls_back() {
        let mut store = AccountStore::memory();
        let parent = store
            .create(&Authority::Root, linux("parent", 1000, Role::Admin))
            .unwrap();
        store.set_pool(&Authority::Root, 1000).unwrap();
        store
            .set_admin_budget(&Authority::Root, &parent.id, 600)
            .unwrap();
        let called = std::cell::Cell::new(false);
        assert!(store
            .set_admin_budget_with(&Authority::Root, &parent.id, 1001, |_| {
                called.set(true);
                Ok(())
            })
            .is_err());
        assert!(!called.get());
        assert!(store
            .set_admin_budget_with(&Authority::Root, &parent.id, 400, |remaining| {
                assert_eq!(remaining, 400);
                called.set(true);
                Err(StoreError::QuotaExceeded)
            })
            .is_err());
        assert!(called.get());
        assert_eq!(store.budget(&parent.id).unwrap()["limit_bytes"], 600);
    }
    #[test]
    fn root_admin_migration_preserves_credentials_and_rejects_stale_generations() {
        let mut store = AccountStore::memory();
        let from = store
            .create(&Authority::Root, linux("legacy", 1000, Role::Admin))
            .unwrap();
        let from = store
            .set_password(&Authority::Root, &from.id, "existing-private-web-password")
            .unwrap();
        let target = store
            .create(
                &Authority::Root,
                NewAccount {
                    username: "newpanel".into(),
                    uid: 200000,
                    gid: 200000,
                    home: "/home/tealdrive/virtual-homes/tdv_200000".into(),
                    kind: AccountKind::Virtual,
                    role: Role::Admin,
                    owner_admin_id: None,
                    quota_bytes: None,
                },
            )
            .unwrap();
        assert!(store
            .copy_admin_credentials(
                &authority(&from),
                &from.id,
                from.generation,
                &target.id,
                target.generation
            )
            .is_err());
        assert!(store
            .copy_admin_credentials(
                &Authority::Root,
                &from.id,
                from.generation - 1,
                &target.id,
                target.generation
            )
            .is_err());
        store
            .copy_admin_credentials(
                &Authority::Root,
                &from.id,
                from.generation,
                &target.id,
                target.generation,
            )
            .unwrap();
        assert_eq!(
            store
                .verify_password("newpanel", "existing-private-web-password")
                .unwrap()
                .id,
            target.id
        );
        assert_eq!(
            store
                .verify_password("legacy", "existing-private-web-password")
                .unwrap()
                .id,
            from.id
        );
        assert_eq!(store.get(&target.id).unwrap().uid, 200000);
        assert!(store
            .copy_admin_credentials(
                &Authority::Root,
                &from.id,
                from.generation,
                &target.id,
                target.generation
            )
            .is_err());
    }
    #[test]
    fn password_lifecycle_public_metadata_and_generation_revocation() {
        let mut store = AccountStore::memory();
        let account = store
            .create(&Authority::Root, linux("alice", 1000, Role::User))
            .unwrap();
        assert!(!account.password_enrolled);
        assert!(matches!(
            store.verify_password("alice", "linux-password"),
            Err(StoreError::InvalidCredentials)
        ));
        let enrolled = store
            .set_password(&authority(&account), &account.id, "separate-web-password")
            .unwrap();
        assert!(enrolled.password_enrolled);
        assert!(enrolled.generation > account.generation);
        let authenticated = store
            .verify_password("alice", "separate-web-password")
            .unwrap();
        assert_eq!(authenticated.id, account.id);
        assert!(store
            .verify_generation(&account.id, account.generation, 1000, 1000)
            .is_err());
        let public = serde_json::to_string(&authenticated).unwrap();
        assert!(!public.contains("argon2") && !public.contains("separate-web-password"));
        let revoked = store
            .transition(
                &authority(&enrolled),
                &account.id,
                AccountStatus::Revoked,
                false,
            )
            .unwrap();
        assert!(!revoked.login_enabled);
        assert!(store
            .verify_password("alice", "separate-web-password")
            .is_err());
        assert!(store
            .verify_generation(&account.id, enrolled.generation, 1000, 1000)
            .is_err());
        assert!(store
            .verify_password("missing", "separate-web-password")
            .is_err());
    }
    #[test]
    fn root_pool_admin_budget_child_allocation_and_cross_admin_denial() {
        let mut store = AccountStore::memory();
        let admin = store
            .create(&Authority::Root, linux("admin", 1001, Role::Admin))
            .unwrap();
        let other = store
            .create(&Authority::Root, linux("other", 1002, Role::Admin))
            .unwrap();
        store.set_pool(&Authority::Root, 1000).unwrap();
        store
            .set_admin_budget(&Authority::Root, &admin.id, 600)
            .unwrap();
        assert!(matches!(
            store.set_admin_budget(&Authority::Root, &other.id, 500),
            Err(StoreError::QuotaExceeded)
        ));
        assert!(matches!(
            store.set_pool(&Authority::Root, 599),
            Err(StoreError::QuotaExceeded)
        ));
        let child = store
            .create(
                &authority(&admin),
                virtual_user("virtual-one", 200000, &admin.id, 400),
            )
            .unwrap();
        assert!(matches!(
            store.create(
                &authority(&admin),
                virtual_user("virtual-two", 200001, &admin.id, 201)
            ),
            Err(StoreError::QuotaExceeded)
        ));
        assert!(matches!(
            store.set_admin_budget(&Authority::Root, &admin.id, 399),
            Err(StoreError::QuotaExceeded)
        ));
        assert!(matches!(
            store.create(
                &authority(&other),
                virtual_user("stolen", 200001, &admin.id, 1)
            ),
            Err(StoreError::Forbidden)
        ));
        assert!(matches!(
            store.transition(
                &authority(&other),
                &child.id,
                AccountStatus::Suspended,
                false
            ),
            Err(StoreError::Forbidden)
        ));
        assert!(store
            .list(&authority(&other), AccountKind::Virtual)
            .unwrap()
            .is_empty());
        assert!(matches!(
            store.transition(&authority(&child), &child.id, AccountStatus::Revoked, false),
            Err(StoreError::Forbidden)
        ));
        let suspended = store
            .transition(
                &authority(&admin),
                &child.id,
                AccountStatus::Suspended,
                false,
            )
            .unwrap();
        assert!(!suspended.login_enabled);
        assert!(matches!(
            store.transition(&authority(&admin), &child.id, AccountStatus::Deleted, false),
            Err(StoreError::Forbidden)
        ));
        let deleted = store
            .transition(&authority(&admin), &child.id, AccountStatus::Deleted, true)
            .unwrap();
        assert!(deleted.deleted_at.is_some());
        // Deleted account data retains its allocation until the filesystem retention/GC step succeeds.
        assert!(matches!(
            store.set_admin_budget(&Authority::Root, &admin.id, 399),
            Err(StoreError::QuotaExceeded)
        ));
    }
    #[test]
    fn identities_constraints_duplicates_and_untrusted_paths_fail_closed() {
        let mut store = AccountStore::memory();
        let user = store
            .create(&Authority::Root, linux("alice", 1000, Role::User))
            .unwrap();
        assert!(store
            .create(&Authority::Root, linux("other", 1000, Role::User))
            .is_err());
        assert!(store
            .create(&Authority::Root, linux("alice", 1001, Role::User))
            .is_err());
        assert!(store
            .create(&authority(&user), linux("promoted", 1002, Role::Admin))
            .is_err());
        assert!(store
            .create(&Authority::Root, linux("root", 0, Role::Admin))
            .is_err());
        assert!(store
            .verify_generation(&user.id, user.generation, 1001, 1000)
            .is_err());
        assert!(matches!(
            root_parent(Path::new("/tmp/accounts.db")),
            Err(StoreError::UntrustedStore)
        ));
        assert!(matches!(
            root_parent(Path::new("relative.db")),
            Err(StoreError::UntrustedStore)
        ));
        assert!(matches!(
            root_parent(Path::new("/root/../tmp/accounts.db")),
            Err(StoreError::UntrustedStore)
        ));
    }
    #[test]
    fn virtual_admin_uses_root_pool_and_own_quota_excludes_child_reservations() {
        let mut store = AccountStore::memory();
        let mut new = linux("virtual-admin", 200010, Role::Admin);
        new.kind = AccountKind::Virtual;
        let admin = store.create(&Authority::Root, new).unwrap();
        assert_eq!(store.effective_quota(&admin.id).unwrap(), Some(0));
        store.set_pool(&Authority::Root, 1000).unwrap();
        store
            .set_admin_budget(&Authority::Root, &admin.id, 600)
            .unwrap();
        assert_eq!(store.effective_quota(&admin.id).unwrap(), Some(600));
        let child = store
            .create(
                &authority(&admin),
                virtual_user("child", 200011, &admin.id, 400),
            )
            .unwrap();
        assert_eq!(store.effective_quota(&admin.id).unwrap(), Some(200));
        assert_eq!(store.effective_quota(&child.id).unwrap(), Some(400));
    }
}
