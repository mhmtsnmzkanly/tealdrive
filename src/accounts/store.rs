//! Each helper operation opens a root-only database and uses short immediate transactions.
//! No network request chooses this path. Do not serialize the connection or password hashes.
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
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
    connection: Connection,
}
fn now() -> u64 {
    crate::session::store::InMemorySessionStore::now_epoch_seconds()
}
fn number(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::InvalidInput)
}
fn root_parent(path: &Path) -> Result<(), StoreError> {
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
fn row_account(row: &rusqlite::Row<'_>) -> rusqlite::Result<Account> {
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
    })
}
const COLUMNS:&str="id,username,uid,gid,home,kind,role,owner_admin_id,status,login_enabled,generation,web_password_hash IS NOT NULL,quota_bytes,undo_hours,created_at,updated_at,deleted_at";
fn account(connection: &Connection, id: &str) -> Result<Account, StoreError> {
    connection
        .query_row(
            &format!("SELECT {COLUMNS} FROM accounts WHERE id=?1"),
            [id],
            row_account,
        )
        .optional()?
        .ok_or(StoreError::NotFound)
}
fn authorized(connection: &Connection, actor: &Authority) -> Result<Option<Account>, StoreError> {
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
fn manage(connection: &Connection, actor: &Authority, target: &Account) -> Result<(), StoreError> {
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
        if version != 1 || application != 1413763670 {
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
            if number(quota)? > budget - allocated {
                return Err(StoreError::QuotaExceeded);
            }
        } else if new.owner_admin_id.is_some() || new.quota_bytes.is_some() {
            return Err(StoreError::InvalidInput);
        }
        let id = uuid::Uuid::new_v4().to_string();
        let time = now();
        tx.execute("INSERT INTO accounts(id,username,uid,gid,home,kind,role,owner_admin_id,quota_bytes,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10)",params![id,new.username,new.uid,new.gid,new.home.to_str().ok_or(StoreError::InvalidInput)?,if new.kind==AccountKind::Linux{"linux"}else{"virtual"},if new.role==Role::Admin{"admin"}else{"user"},new.owner_admin_id,new.quota_bytes.map(number).transpose()?,number(time)?])?;
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
        let budget: i64 = self
            .connection
            .query_row(
                "SELECT limit_bytes FROM admin_budgets WHERE account_id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0);
        let allocated: i64 = self.connection.query_row(
            "SELECT coalesce(sum(quota_bytes),0) FROM accounts WHERE owner_admin_id=?1",
            [id],
            |r| r.get(0),
        )?;
        if allocated > budget {
            return Err(StoreError::QuotaExceeded);
        }
        Ok(Some((budget - allocated) as u64))
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
        let bytes = number(bytes)?;
        if bytes < children || bytes > pool - (allocated - current) {
            return Err(StoreError::QuotaExceeded);
        }
        tx.execute("INSERT INTO admin_budgets(account_id,limit_bytes) VALUES(?1,?2) ON CONFLICT(account_id) DO UPDATE SET limit_bytes=excluded.limit_bytes",params![id,bytes])?;
        tx.commit()?;
        Ok(())
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
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let target = account(&tx, id)?;
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
        let (state, login) = match status {
            AccountStatus::Active => ("active", true),
            AccountStatus::Suspended => ("suspended", false),
            AccountStatus::Revoked => ("revoked", false),
            AccountStatus::Deleted => ("deleted", false),
        };
        tx.execute("UPDATE accounts SET status=?1,login_enabled=?2,generation=generation+1,updated_at=?3,deleted_at=?4 WHERE id=?5",params![state,login,number(now())?,if status==AccountStatus::Deleted{Some(number(now())?)}else{None},id])?;
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
    fn memory() -> Self {
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
