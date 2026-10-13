//! Root-only virtual identity provisioning and quota management.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VirtualConfig {
    pub enabled: bool,
    pub allow_admin_delete: bool,
    pub deleted_retention_hours: u64,
    pub max_accounts: u32,
    pub enforcement: Enforcement,
    pub max_inodes: u64,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Enforcement {
    #[default]
    Application,
    UserQuota,
}
impl Default for VirtualConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            allow_admin_delete: false,
            deleted_retention_hours: 36,
            max_accounts: 4096,
            enforcement: Enforcement::Application,
            max_inodes: 100000,
        }
    }
}
impl VirtualConfig {
    pub fn validate(&self, web_enabled: bool) -> Result<(), crate::errors::TealDriveError> {
        if self.enabled && !web_enabled
            || !(1..=876000).contains(&self.deleted_retention_hours)
            || !(1..=100000).contains(&self.max_accounts)
            || !(32..=1000000).contains(&self.max_inodes)
        {
            return Err(crate::errors::TealDriveError::Validation);
        }
        Ok(())
    }
}

use super::store::{
    account, manage, now, number, pending_reserved, root_parent, Account, AccountKind,
    AccountStore, Authority, NewAccount, Role, StoreError,
};
use crate::config::{AllowedRoot, AppConfig};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use std::{
    ffi::CString,
    fs::{File, OpenOptions},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    version: u8,
    owner: String,
    name: String,
    quota: u64,
    uid: u32,
    account_id: Option<String>,
}
fn io_error(_: std::io::Error) -> StoreError {
    StoreError::Forbidden
}
fn key(owner: &str, request: &str) -> Result<String, StoreError> {
    let request = uuid::Uuid::parse_str(request).map_err(|_| StoreError::InvalidInput)?;
    Ok(format!("virtual_create_v1:{owner}:{request}"))
}
pub(crate) fn lock() -> Result<File, StoreError> {
    if unsafe { libc::geteuid() } != 0 {
        return Err(StoreError::Forbidden);
    }
    let path = Path::new("/home/tealdrive/auth/provision.lock");
    root_parent(path)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o077 != 0 {
        return Err(StoreError::UntrustedStore);
    }
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(file)
}
fn os_name(uid: u32) -> String {
    format!("tdv_{uid}")
}
fn home(uid: u32) -> PathBuf {
    Path::new(crate::layout::VIRTUAL_HOMES).join(os_name(uid))
}
fn command(program: &str, args: &[&str]) -> Result<(), StoreError> {
    let metadata = std::fs::symlink_metadata(program)?;
    if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
        return Err(StoreError::UntrustedStore);
    }
    let status = Command::new(program)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(StoreError::Forbidden)
    }
}
fn name_exists(name: &str) -> Result<bool, StoreError> {
    let name = CString::new(name).map_err(|_| StoreError::InvalidInput)?;
    let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
    let mut result = std::ptr::null_mut();
    let mut buffer = vec![0u8; 65536];
    if unsafe {
        libc::getpwnam_r(
            name.as_ptr(),
            &mut pwd,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        )
    } != 0
    {
        return Err(StoreError::Forbidden);
    }
    Ok(!result.is_null())
}
fn free_id(uid: u32) -> Result<bool, StoreError> {
    let mut passwd: libc::passwd = unsafe { std::mem::zeroed() };
    let mut pass_result = std::ptr::null_mut();
    let mut group: libc::group = unsafe { std::mem::zeroed() };
    let mut group_result = std::ptr::null_mut();
    let mut buffer = vec![0u8; 65536];
    if unsafe {
        libc::getpwuid_r(
            uid,
            &mut passwd,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut pass_result,
        )
    } != 0
    {
        return Err(StoreError::Forbidden);
    }
    if !pass_result.is_null() {
        return Ok(false);
    }
    if unsafe {
        libc::getgrgid_r(
            uid,
            &mut group,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut group_result,
        )
    } != 0
    {
        return Err(StoreError::Forbidden);
    }
    if !group_result.is_null() {
        return Ok(false);
    }
    let name = CString::new(os_name(uid)).map_err(|_| StoreError::InvalidInput)?;
    if unsafe {
        libc::getpwnam_r(
            name.as_ptr(),
            &mut passwd,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut pass_result,
        )
    } != 0
    {
        return Err(StoreError::Forbidden);
    }
    Ok(pass_result.is_null())
}
pub(crate) fn locked_os_password(name: &str) -> bool {
    let Ok(name) = CString::new(name) else {
        return false;
    };
    let mut shadow: libc::spwd = unsafe { std::mem::zeroed() };
    let mut result = std::ptr::null_mut();
    let mut buffer = zeroize::Zeroizing::new(vec![0u8; 65536]);
    if unsafe {
        libc::getspnam_r(
            name.as_ptr(),
            &mut shadow,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        )
    } != 0
        || result.is_null()
        || shadow.sp_pwdp.is_null()
    {
        return false;
    }
    matches!(unsafe { *shadow.sp_pwdp.cast::<u8>() }, b'!' | b'*')
}
fn allocate(store: &AccountStore, cfg: &AppConfig) -> Result<u32, StoreError> {
    let raw: Option<String> = store
        .connection
        .query_row(
            "SELECT value FROM security_state WHERE key='virtual_uid_cursor'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let min = cfg.web_accounts.virtual_min_uid;
    let max = cfg.web_accounts.virtual_max_uid;
    let start = match raw {
        Some(raw) => raw.parse::<u32>().map_err(|_| StoreError::InvalidInput)?,
        None => min,
    };
    if start < min || start > max {
        return Err(StoreError::InvalidInput);
    }
    for offset in 0..1024u64.min(u64::from(max - min) + 1) {
        let uid = min + ((u64::from(start - min) + offset) % (u64::from(max - min) + 1)) as u32;
        let retired:bool=store.connection.query_row("SELECT EXISTS(SELECT 1 FROM accounts WHERE uid=?1) OR EXISTS(SELECT 1 FROM security_state WHERE key=?2) OR EXISTS(SELECT 1 FROM security_state WHERE CASE WHEN key LIKE 'virtual_create_v1:%' THEN json_extract(value,'$.uid')=?1 ELSE 0 END)",params![uid,format!("virtual_uid_retired:{uid}")],|r|r.get(0))?;
        if !retired && free_id(uid)? {
            let next = if uid == max { min } else { uid + 1 };
            store.connection.execute("INSERT INTO security_state(key,value) VALUES('virtual_uid_cursor',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[next.to_string()])?;
            return Ok(uid);
        }
    }
    Err(StoreError::QuotaExceeded)
}
fn validate_input(cfg: &AppConfig, name: &str, quota: u64) -> Result<(), StoreError> {
    if !cfg.virtual_accounts.enabled || cfg.home_template.is_none() {
        return Err(StoreError::Forbidden);
    }
    if name.is_empty()
        || name.len() > 64
        || !name.as_bytes()[0].is_ascii_lowercase()
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b))
        || quota == 0
        || quota > i64::MAX as u64
        || crate::auth::account::is_reserved_service_username(name)
    {
        return Err(StoreError::InvalidInput);
    }
    if cfg.virtual_accounts.enforcement == Enforcement::UserQuota
        && (quota < 4096 || quota % 1024 != 0)
    {
        return Err(StoreError::InvalidInput);
    }
    Ok(())
}
pub(crate) fn root(cfg: &AppConfig, target: &Account) -> Result<AllowedRoot, StoreError> {
    if target.kind != AccountKind::Virtual
        || target.home != home(target.uid)
        || target.uid < cfg.web_accounts.virtual_min_uid
        || target.uid > cfg.web_accounts.virtual_max_uid
    {
        return Err(StoreError::Forbidden);
    }
    crate::helper::account_gateway::validate_account(cfg, target)
        .map_err(|_| StoreError::Forbidden)?;
    root_parent(&target.home)?;
    let metadata = std::fs::symlink_metadata(&target.home)?;
    if !metadata.is_dir() || metadata.uid() != target.uid || metadata.gid() != target.gid {
        return Err(StoreError::UntrustedStore);
    }
    let mut root = cfg
        .home_template
        .as_ref()
        .ok_or(StoreError::Forbidden)?
        .clone();
    root.base_path = target.home.clone();
    root.quota_bytes = target
        .quota_bytes
        .map(|q| root.quota_bytes.map_or(q, |ceiling| q.min(ceiling)));
    Ok(root)
}
pub(crate) fn usage(cfg: &AppConfig, root: &AllowedRoot, uid: u32) -> Result<u64, StoreError> {
    if cfg.virtual_accounts.enforcement == Enforcement::UserQuota {
        crate::helper::native_quota::read(root, uid)
            .map(|u| u.used_bytes)
            .map_err(io_error)
    } else {
        crate::helper::quota::measure(root).map_err(io_error)
    }
}
fn native_set(cfg: &AppConfig, root: &AllowedRoot, uid: u32, bytes: u64) -> Result<(), StoreError> {
    if cfg.virtual_accounts.enforcement == Enforcement::UserQuota {
        crate::helper::native_quota::set(
            root,
            uid,
            cfg.home_template
                .as_ref()
                .and_then(|template| template.quota_bytes)
                .map_or(bytes, |limit| bytes.min(limit)),
            cfg.virtual_accounts.max_inodes,
        )
        .map_err(io_error)?;
    }
    Ok(())
}
fn own_reservation(
    store: &AccountStore,
    cfg: &AppConfig,
    parent: &Account,
) -> Result<Option<(AllowedRoot, crate::helper::quota::Reservation, u64)>, StoreError> {
    if parent.kind != AccountKind::Virtual {
        return Ok(None);
    }
    let mut root = root(cfg, parent)?;
    root.quota_bytes = store.effective_quota(&parent.id)?;
    let held = crate::helper::quota::Reservation::acquire(&root).map_err(io_error)?;
    let used = if cfg.virtual_accounts.enforcement == Enforcement::UserQuota {
        crate::helper::native_quota::read(&root, parent.uid)
            .map_err(io_error)?
            .used_bytes
    } else {
        held.used_bytes
    };
    Ok(Some((root, held, used)))
}
impl AccountStore {
    pub fn is_purged(&self, id: &str) -> Result<bool, StoreError> {
        let raw: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM security_state WHERE key=?1",
                [format!("virtual_purged_v1:{id}")],
                |r| r.get(0),
            )
            .optional()?;
        let Some(raw) = raw else {
            return Ok(false);
        };
        let value: serde_json::Value =
            serde_json::from_str(&raw).map_err(|_| StoreError::InvalidInput)?;
        let account = self.get(id)?;
        if value["version"] != 1
            || value["uid"] != account.uid
            || account.status != super::store::AccountStatus::Deleted
            || account.quota_bytes != Some(0)
            || account.password_enrolled
        {
            return Err(StoreError::InvalidInput);
        }
        Ok(true)
    }
    pub fn pool_limit(&self) -> Result<u64, StoreError> {
        let limit: i64 = self.connection.query_row(
            "SELECT limit_bytes FROM quota_pool WHERE id=1",
            [],
            |r| r.get(0),
        )?;
        u64::try_from(limit).map_err(|_| StoreError::InvalidInput)
    }
    pub fn budget(&self, id: &str) -> Result<serde_json::Value, StoreError> {
        let target = self.get(id)?;
        if target.role != Role::Admin {
            return Err(StoreError::Forbidden);
        }
        let (limit,reserved,pending):(i64,i64,i64)=self.connection.query_row("SELECT coalesce((SELECT limit_bytes FROM admin_budgets WHERE account_id=?1),0),coalesce((SELECT sum(quota_bytes) FROM accounts WHERE owner_admin_id=?1),0),(SELECT coalesce(sum(CASE WHEN key LIKE 'virtual_create_v1:%' THEN CASE WHEN json_extract(value,'$.owner')=?1 AND json_extract(value,'$.account_id') IS NULL THEN json_extract(value,'$.quota') ELSE 0 END ELSE 0 END),0) FROM security_state)",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        let reserved = reserved
            .checked_add(pending)
            .ok_or(StoreError::QuotaExceeded)?;
        if reserved > limit {
            return Err(StoreError::QuotaExceeded);
        }
        Ok(
            serde_json::json!({"limit_bytes":limit,"reserved_bytes":reserved,"remaining_bytes":limit-reserved}),
        )
    }
    pub(crate) fn set_child_quota_with<F: FnOnce(u64) -> Result<(), StoreError>>(
        &mut self,
        actor: &Authority,
        id: &str,
        generation: u64,
        bytes: u64,
        before: F,
    ) -> Result<Account, StoreError> {
        if bytes == 0 {
            return Err(StoreError::InvalidInput);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let target = account(&tx, id)?;
        manage(&tx, actor, &target)?;
        if target.kind != AccountKind::Virtual
            || target.role != Role::User
            || target.status == super::store::AccountStatus::Deleted
            || target.generation != generation
        {
            return Err(StoreError::Conflict);
        }
        let owner = target
            .owner_admin_id
            .as_ref()
            .ok_or(StoreError::Forbidden)?;
        let (limit,allocated):(i64,i64)=tx.query_row("SELECT coalesce((SELECT limit_bytes FROM admin_budgets WHERE account_id=?1),0),coalesce((SELECT sum(quota_bytes) FROM accounts WHERE owner_admin_id=?1),0)",[owner],|r|Ok((r.get(0)?,r.get(1)?)))?;
        let allocated = allocated
            .checked_add(pending_reserved(&tx, owner, "")?)
            .ok_or(StoreError::QuotaExceeded)?;
        if number(bytes)? > limit - (allocated - number(target.quota_bytes.unwrap_or(0))?) {
            return Err(StoreError::QuotaExceeded);
        }
        before(
            (limit - allocated + number(target.quota_bytes.unwrap_or(0))? - number(bytes)?) as u64,
        )?;
        tx.execute("INSERT INTO security_state(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![format!("virtual_quota_sync_v1:{id}"),serde_json::json!({"version":1,"account_id":id}).to_string()])?;
        tx.execute(
            "UPDATE accounts SET quota_bytes=?1,generation=generation+1,updated_at=?2 WHERE id=?3",
            params![number(bytes)?, number(now())?, id],
        )?;
        let result = account(&tx, id)?;
        tx.commit()?;
        Ok(result)
    }
}
pub(crate) fn provision(
    store: &mut AccountStore,
    cfg: &AppConfig,
    actor: &Account,
    name: &str,
    quota: u64,
    password: &str,
    request: &str,
) -> Result<Account, StoreError> {
    validate_input(cfg, name, quota)?;
    if actor.role != Role::Admin {
        return Err(StoreError::Forbidden);
    }
    // Validate and perform bounded password hashing before making an OS change.
    let _password_check = super::password::hash(password)?;
    let _global = lock()?;
    let request_key = key(&actor.id, request)?;
    let saved: Option<String> = store
        .connection
        .query_row(
            "SELECT value FROM security_state WHERE key=?1",
            [&request_key],
            |r| r.get(0),
        )
        .optional()?;
    let existing = if let Some(raw) = saved {
        if raw.len() > 8192 {
            return Err(StoreError::InvalidInput);
        }
        let intent: Intent = serde_json::from_str(&raw).map_err(|_| StoreError::InvalidInput)?;
        if intent.version != 1
            || intent.owner != actor.id
            || intent.name != name
            || intent.quota != quota
        {
            return Err(StoreError::Conflict);
        }
        if let Some(id) = &intent.account_id {
            let target = store.get(id)?;
            if target.owner_admin_id.as_deref() != Some(&actor.id)
                || target.kind != AccountKind::Virtual
            {
                return Err(StoreError::Forbidden);
            }
            return Ok(target);
        }
        Some(intent)
    } else {
        None
    };
    let parent = own_reservation(store, cfg, actor)?;
    let intent = if let Some(intent) = existing {
        if let Some((parent_root, _, used)) = &parent {
            let remaining = store.effective_quota(&actor.id)?.unwrap_or(0);
            if remaining < *used {
                return Err(StoreError::QuotaExceeded);
            }
            native_set(cfg, parent_root, actor.uid, remaining)?;
        }
        intent
    } else {
        if store.find_username(name)?.is_some() || name_exists(name)? {
            return Err(StoreError::Conflict);
        }
        let pending_name:bool=store.connection.query_row("SELECT EXISTS(SELECT 1 FROM security_state WHERE CASE WHEN key LIKE 'virtual_create_v1:%' THEN json_extract(value,'$.name')=?1 AND json_extract(value,'$.account_id') IS NULL ELSE 0 END)",[name],|r|r.get(0))?;
        if pending_name {
            return Err(StoreError::Conflict);
        }
        let count:i64=store.connection.query_row("SELECT (SELECT count(*) FROM accounts WHERE kind='virtual' AND status!='deleted')+(SELECT count(*) FROM security_state WHERE CASE WHEN key LIKE 'virtual_create_v1:%' THEN json_extract(value,'$.account_id') IS NULL ELSE 0 END)",[],|r|r.get(0))?;
        if count >= i64::from(cfg.virtual_accounts.max_accounts) {
            return Err(StoreError::QuotaExceeded);
        }
        let budget = store.budget(&actor.id)?;
        if budget["remaining_bytes"].as_u64().unwrap_or(0) < quota {
            return Err(StoreError::QuotaExceeded);
        }
        let uid = allocate(store, cfg)?;
        let old_limit = store.effective_quota(&actor.id)?.unwrap_or(0);
        if let Some((parent_root, _, used)) = &parent {
            if quota > old_limit.saturating_sub(*used) {
                return Err(StoreError::QuotaExceeded);
            }
            native_set(cfg, parent_root, actor.uid, old_limit - quota)?;
        }
        let intent = Intent {
            version: 1,
            owner: actor.id.clone(),
            name: name.into(),
            quota,
            uid,
            account_id: None,
        };
        let result = (|| -> Result<(), StoreError> {
            let tx = store
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            let current = super::store::authorized(
                &tx,
                &Authority::Account {
                    id: actor.id.clone(),
                    generation: actor.generation,
                },
            )?
            .ok_or(StoreError::Forbidden)?;
            if current.role != Role::Admin {
                return Err(StoreError::Forbidden);
            }
            let (limit,allocated):(i64,i64)=tx.query_row("SELECT coalesce((SELECT limit_bytes FROM admin_budgets WHERE account_id=?1),0),coalesce((SELECT sum(quota_bytes) FROM accounts WHERE owner_admin_id=?1),0)",[&actor.id],|r|Ok((r.get(0)?,r.get(1)?)))?;
            if number(quota)? > limit - allocated - pending_reserved(&tx, &actor.id, "")? {
                return Err(StoreError::QuotaExceeded);
            }
            tx.execute(
                "INSERT INTO security_state(key,value) VALUES(?1,?2)",
                params![
                    request_key,
                    serde_json::to_string(&intent).map_err(|_| StoreError::InvalidInput)?
                ],
            )?;
            tx.commit()?;
            Ok(())
        })();
        if let Err(error) = result {
            if let Some((parent_root, _, _)) = &parent {
                let _ = native_set(cfg, parent_root, actor.uid, old_limit);
            }
            return Err(error);
        }
        intent
    };
    let uid = intent.uid;
    let home = ensure_os_identity(cfg, uid, quota)?;
    let raw = serde_json::to_string(&intent).map_err(|_| StoreError::InvalidInput)?;
    let authority = Authority::Account {
        id: actor.id.clone(),
        generation: actor.generation,
    };
    store.create_enrolled(
        &authority,
        NewAccount {
            username: name.into(),
            uid,
            gid: uid,
            home,
            kind: AccountKind::Virtual,
            role: Role::User,
            owner_admin_id: Some(actor.id.clone()),
            quota_bytes: Some(quota),
        },
        password,
        Some((&request_key, &raw)),
    )
}
fn ensure_os_identity(cfg: &AppConfig, uid: u32, quota: u64) -> Result<PathBuf, StoreError> {
    let os_name = os_name(uid);
    let uid_text = uid.to_string();
    let home = home(uid);
    root_parent(&home)?;
    // An intent owns only this generated UID/name/home; never adopts an arbitrary user.
    let name_c = CString::new(os_name.as_str()).map_err(|_| StoreError::InvalidInput)?;
    let mut group: libc::group = unsafe { std::mem::zeroed() };
    let mut result = std::ptr::null_mut();
    let mut bytes = vec![0u8; 65536];
    if unsafe {
        libc::getgrnam_r(
            name_c.as_ptr(),
            &mut group,
            bytes.as_mut_ptr().cast(),
            bytes.len(),
            &mut result,
        )
    } != 0
    {
        return Err(StoreError::Forbidden);
    }
    if result.is_null() {
        command("/usr/sbin/groupadd", &["--gid", &uid_text, &os_name])?;
    } else if group.gr_gid != uid {
        return Err(StoreError::Forbidden);
    }
    if !name_exists(&os_name)? {
        command(
            "/usr/sbin/useradd",
            &[
                "--no-create-home",
                "--no-log-init",
                "--uid",
                &uid_text,
                "--gid",
                &uid_text,
                "--home-dir",
                home.to_str().ok_or(StoreError::InvalidInput)?,
                "--shell",
                "/usr/sbin/nologin",
                "--password",
                "!",
                &os_name,
            ],
        )?;
    }
    let identity = crate::helper::security::account(&os_name).map_err(|_| StoreError::Forbidden)?;
    if identity.uid != uid
        || identity.primary_gid != uid
        || identity.home_dir.as_ref() != Some(&home)
        || identity.shell.as_deref() != Some("/usr/sbin/nologin")
        || !locked_os_password(&os_name)
    {
        return Err(StoreError::Forbidden);
    }
    match std::fs::symlink_metadata(&home) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(&home)?;
            std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(m) if m.is_dir() && (m.uid() == uid || m.uid() == 0) => {}
        _ => return Err(StoreError::UntrustedStore),
    }
    let mut child_root = cfg
        .home_template
        .as_ref()
        .ok_or(StoreError::Forbidden)?
        .clone();
    child_root.base_path = home.clone();
    child_root.quota_bytes = Some(child_root.quota_bytes.map_or(quota, |q| q.min(quota)));
    native_set(cfg, &child_root, uid, quota)?;
    let home_c =
        CString::new(home.as_os_str().as_encoded_bytes()).map_err(|_| StoreError::InvalidInput)?;
    if unsafe { libc::chown(home_c.as_ptr(), uid, uid) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(home)
}
/// Only an offline Root Wizard can create an unowned virtual administrator.
pub(crate) fn create_admin(
    store: &mut AccountStore,
    cfg: &AppConfig,
    name: &str,
    budget: u64,
    password: &str,
    request: &str,
) -> Result<Account, StoreError> {
    validate_input(cfg, name, budget)?;
    if !cfg.web_accounts.enabled || cfg.home_template.is_none() {
        return Err(StoreError::Forbidden);
    }
    let _password_check = super::password::hash(password)?;
    let _global = lock()?;
    let request_key = key("__root__", request)?;
    let saved: Option<String> = store
        .connection
        .query_row(
            "SELECT value FROM security_state WHERE key=?1",
            [&request_key],
            |r| r.get(0),
        )
        .optional()?;
    let intent = if let Some(raw) = saved {
        if raw.len() > 8192 {
            return Err(StoreError::InvalidInput);
        }
        let intent: Intent = serde_json::from_str(&raw).map_err(|_| StoreError::InvalidInput)?;
        if intent.version != 1
            || intent.owner != "__root__"
            || intent.name != name
            || intent.quota != budget
        {
            return Err(StoreError::Conflict);
        }
        if let Some(id) = &intent.account_id {
            let target = store.get(id)?;
            if target.kind != AccountKind::Virtual
                || target.role != Role::Admin
                || target.owner_admin_id.is_some()
            {
                return Err(StoreError::Forbidden);
            }
            return Ok(target);
        }
        intent
    } else {
        if store.find_username(name)?.is_some() || name_exists(name)? {
            return Err(StoreError::Conflict);
        }
        let pending_name: bool = store.connection.query_row("SELECT EXISTS(SELECT 1 FROM security_state WHERE CASE WHEN key LIKE 'virtual_create_v1:%' THEN json_extract(value,'$.name')=?1 AND json_extract(value,'$.account_id') IS NULL ELSE 0 END)", [name], |r| r.get(0))?;
        if pending_name {
            return Err(StoreError::Conflict);
        }
        let count: i64 = store.connection.query_row("SELECT (SELECT count(*) FROM accounts WHERE kind='virtual' AND status!='deleted')+(SELECT count(*) FROM security_state WHERE CASE WHEN key LIKE 'virtual_create_v1:%' THEN json_extract(value,'$.account_id') IS NULL ELSE 0 END)", [], |r| r.get(0))?;
        if count >= i64::from(cfg.virtual_accounts.max_accounts) {
            return Err(StoreError::QuotaExceeded);
        }
        let uid = allocate(store, cfg)?;
        let intent = Intent {
            version: 1,
            owner: "__root__".into(),
            name: name.into(),
            quota: budget,
            uid,
            account_id: None,
        };
        let tx = store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let pool: i64 = tx.query_row("SELECT limit_bytes FROM quota_pool WHERE id=1", [], |r| {
            r.get(0)
        })?;
        let allocated: i64 = tx.query_row(
            "SELECT coalesce(sum(limit_bytes),0) FROM admin_budgets",
            [],
            |r| r.get(0),
        )?;
        if number(budget)? > pool - allocated - pending_reserved(&tx, "__root__", "")? {
            return Err(StoreError::QuotaExceeded);
        }
        tx.execute(
            "INSERT INTO security_state(key,value) VALUES(?1,?2)",
            params![
                request_key,
                serde_json::to_string(&intent).map_err(|_| StoreError::InvalidInput)?
            ],
        )?;
        tx.commit()?;
        intent
    };
    let home = ensure_os_identity(cfg, intent.uid, budget)?;
    let raw = serde_json::to_string(&intent).map_err(|_| StoreError::InvalidInput)?;
    store.create_enrolled(
        &Authority::Root,
        NewAccount {
            username: name.into(),
            uid: intent.uid,
            gid: intent.uid,
            home,
            kind: AccountKind::Virtual,
            role: Role::Admin,
            owner_admin_id: None,
            quota_bytes: None,
        },
        password,
        Some((&request_key, &raw)),
    )
}
pub(crate) fn set_quota(
    store: &mut AccountStore,
    cfg: &AppConfig,
    authority: &Authority,
    target: &Account,
    generation: u64,
    bytes: u64,
) -> Result<Account, StoreError> {
    validate_input(cfg, &target.username, bytes)?;
    if target.kind != AccountKind::Virtual || target.role != Role::User {
        return Err(StoreError::Forbidden);
    }
    let _global = lock()?;
    let owner = store.get(
        target
            .owner_admin_id
            .as_ref()
            .ok_or(StoreError::Forbidden)?,
    )?;
    let parent = own_reservation(store, cfg, &owner)?;
    let target_root = root(cfg, target)?;
    let _held = crate::helper::quota::Reservation::acquire(&target_root).map_err(io_error)?;
    let used = if cfg.virtual_accounts.enforcement == Enforcement::UserQuota {
        crate::helper::native_quota::read(&target_root, target.uid)
            .map_err(io_error)?
            .used_bytes
    } else {
        _held.used_bytes
    };
    if bytes < used {
        return Err(StoreError::QuotaExceeded);
    }
    let old = target.quota_bytes.unwrap_or(0);
    let old_parent = store.budget(&owner.id)?["remaining_bytes"]
        .as_u64()
        .ok_or(StoreError::InvalidInput)?;
    let new_parent = old_parent
        .checked_add(old)
        .and_then(|v| v.checked_sub(bytes))
        .ok_or(StoreError::QuotaExceeded)?;
    if let Some((_, _, used)) = &parent {
        if new_parent < *used {
            return Err(StoreError::QuotaExceeded);
        }
    }
    let committed_remaining = std::cell::Cell::new(new_parent);
    let result =
        store.set_child_quota_with(authority, &target.id, generation, bytes, |remaining| {
            committed_remaining.set(remaining);
            if let Some((parent_root, _, used)) = &parent {
                if remaining < *used {
                    return Err(StoreError::QuotaExceeded);
                }
                if bytes > old {
                    native_set(cfg, parent_root, owner.uid, remaining)?;
                }
            }
            if bytes < old {
                native_set(cfg, &target_root, target.uid, bytes)?;
            }
            Ok(())
        });
    match result {
        Err(error) => {
            if bytes < old {
                let _ = native_set(cfg, &target_root, target.uid, old);
            }
            if bytes > old {
                if let Some((parent_root, _, _)) = &parent {
                    let _ = native_set(cfg, parent_root, owner.uid, old_parent);
                }
            }
            Err(error)
        }
        Ok(updated) => {
            native_set(cfg, &target_root, target.uid, bytes)?;
            if let Some((parent_root, _, _)) = &parent {
                native_set(cfg, parent_root, owner.uid, committed_remaining.get())?;
            }
            store.connection.execute(
                "DELETE FROM security_state WHERE key=?1",
                [format!("virtual_quota_sync_v1:{}", target.id)],
            )?;
            Ok(updated)
        }
    }
}

/// Root budget updates serialize with provisioning and account-directory writes.
pub(crate) fn set_budget(
    store: &mut AccountStore,
    cfg: &AppConfig,
    id: &str,
    bytes: u64,
) -> Result<(), StoreError> {
    let _global = lock()?;
    let target = store.get(id)?;
    if cfg.virtual_accounts.enforcement == Enforcement::UserQuota && bytes % 1024 != 0 {
        return Err(StoreError::InvalidInput);
    }
    let held = own_reservation(store, cfg, &target)?;
    let old_remaining = store.budget(id)?["remaining_bytes"]
        .as_u64()
        .ok_or(StoreError::InvalidInput)?;
    let new_remaining = std::cell::Cell::new(old_remaining);
    let result = store.set_admin_budget_with(&Authority::Root, id, bytes, |remaining| {
        new_remaining.set(remaining);
        if let Some((root, _, used)) = &held {
            if remaining < *used {
                return Err(StoreError::QuotaExceeded);
            }
            if remaining < old_remaining {
                native_set(cfg, root, target.uid, remaining)?;
            }
        }
        Ok(())
    });
    match result {
        Err(error) => {
            if let Some((root, _, _)) = &held {
                let _ = native_set(cfg, root, target.uid, old_remaining);
            }
            Err(error)
        }
        Ok(()) => {
            if let Some((root, _, _)) = &held {
                native_set(cfg, root, target.uid, new_remaining.get())?;
            }
            Ok(())
        }
    }
}
/// Root repair of committed quota changes; never changes allocation metadata.
pub(crate) fn sync_limits(store: &mut AccountStore, cfg: &AppConfig) -> Result<(), StoreError> {
    let _global = lock()?;
    if cfg.virtual_accounts.enforcement != Enforcement::UserQuota {
        return Err(StoreError::InvalidInput);
    }
    for target in store.all_accounts(&Authority::Root, AccountKind::Virtual)? {
        if store.is_purged(&target.id)? {
            continue;
        }
        crate::helper::account_gateway::validate_account(cfg, &target)
            .map_err(|_| StoreError::Forbidden)?;
        let root = root(cfg, &target)?;
        let _held = crate::helper::quota::Reservation::acquire(&root).map_err(io_error)?;
        let limit = store
            .effective_quota(&target.id)?
            .ok_or(StoreError::Forbidden)?;
        native_set(cfg, &root, target.uid, limit)?;
        store.connection.execute(
            "DELETE FROM security_state WHERE key=?1",
            [format!("virtual_quota_sync_v1:{}", target.id)],
        )?;
    }
    Ok(())
}

pub(crate) fn pending(store: &AccountStore) -> Result<Vec<serde_json::Value>, StoreError> {
    let mut statement = store.connection.prepare("SELECT key,value FROM security_state WHERE key LIKE 'virtual_create_v1:%' ORDER BY key LIMIT 4096")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut result = Vec::new();
    for row in rows {
        let (key, raw) = row?;
        let intent: Intent = serde_json::from_str(&raw).map_err(|_| StoreError::InvalidInput)?;
        if intent.version != 1 {
            return Err(StoreError::InvalidInput);
        }
        if intent.account_id.is_none() {
            result.push(serde_json::json!({"key":key,"intent":intent}));
        }
    }
    Ok(result)
}
pub(crate) fn cancel_pending(
    store: &mut AccountStore,
    cfg: &AppConfig,
    request_key: &str,
) -> Result<(), StoreError> {
    let _global = lock()?;
    if !request_key.starts_with("virtual_create_v1:") {
        return Err(StoreError::InvalidInput);
    }
    let raw: String = store.connection.query_row(
        "SELECT value FROM security_state WHERE key=?1",
        [request_key],
        |r| r.get(0),
    )?;
    let intent: Intent = serde_json::from_str(&raw).map_err(|_| StoreError::InvalidInput)?;
    if intent.version != 1
        || intent.account_id.is_some()
        || intent.uid < cfg.web_accounts.virtual_min_uid
        || intent.uid > cfg.web_accounts.virtual_max_uid
    {
        return Err(StoreError::Forbidden);
    }
    // Existing OS state/data is not deleted by a cancellation. Finish using the same request key.
    if !free_id(intent.uid)? || home(intent.uid).try_exists()? {
        return Err(StoreError::Conflict);
    }
    let parent = if intent.owner == "__root__" {
        None
    } else {
        let owner = store.get(&intent.owner)?;
        own_reservation(store, cfg, &owner)?.map(|held| (owner, held))
    };
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute("DELETE FROM security_state WHERE key=?1", [request_key])?;
    tx.execute(
        "INSERT INTO security_state(key,value) VALUES(?1,?2) ON CONFLICT(key) DO NOTHING",
        params![
            format!("virtual_uid_retired:{}", intent.uid),
            now().to_string()
        ],
    )?;
    tx.commit()?;
    if let Some((owner, (root, _, _))) = parent {
        native_set(
            cfg,
            &root,
            owner.uid,
            store
                .effective_quota(&owner.id)?
                .ok_or(StoreError::Forbidden)?,
        )?;
    }
    Ok(())
}
fn no_nested_mounts(path: &Path) -> Result<(), StoreError> {
    let info = std::fs::read_to_string("/proc/self/mountinfo")?;
    for line in info.lines() {
        let raw = line
            .split_whitespace()
            .nth(4)
            .ok_or(StoreError::InvalidInput)?;
        let decoded = raw
            .replace("\\040", " ")
            .replace("\\011", "\t")
            .replace("\\012", "\n")
            .replace("\\134", "\\");
        if Path::new(&decoded).starts_with(path) {
            return Err(StoreError::Forbidden);
        }
    }
    Ok(())
}
/// A fresh executable drops all IDs before deleting any user-owned content.
pub(crate) fn purge_worker(id: &str) -> Result<(), StoreError> {
    if unsafe { libc::getuid() } != 0 || unsafe { libc::geteuid() } != 0 {
        return Err(StoreError::Forbidden);
    }
    let cfg = crate::helper::security::trusted_config().map_err(|_| StoreError::Forbidden)?;
    let store = AccountStore::open(Path::new(crate::layout::ACCOUNTS))?;
    let target = store.get(id)?;
    if target.kind != AccountKind::Virtual
        || target.role != Role::User
        || target.status != super::store::AccountStatus::Deleted
    {
        return Err(StoreError::Forbidden);
    }
    let root = root(&cfg, &target)?;
    no_nested_mounts(&root.base_path)?;
    drop(store);
    // This worker does not need supplementary groups or any Root DB/key descriptor.
    if unsafe { libc::setgroups(0, std::ptr::null()) } != 0
        || unsafe { libc::setresgid(target.gid, target.gid, target.gid) } != 0
        || unsafe { libc::setresuid(target.uid, target.uid, target.uid) } != 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    crate::helper::sandbox::confine(&root)?;
    fn clear(path: &Path, device: u64, depth: usize) -> Result<(), StoreError> {
        if depth > 128 {
            return Err(StoreError::InvalidInput);
        }
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.is_dir() || metadata.dev() != device {
            return Err(StoreError::Forbidden);
        }
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let child = entry.path();
            let metadata = std::fs::symlink_metadata(&child)?;
            if metadata.is_dir() {
                clear(&child, device, depth + 1)?;
                std::fs::remove_dir(child)?;
            } else {
                std::fs::remove_file(child)?;
            }
        }
        Ok(())
    }
    clear(&target.home, std::fs::metadata(&target.home)?.dev(), 0)
}
pub(crate) fn purge_deleted(
    store: &mut AccountStore,
    cfg: &AppConfig,
) -> Result<usize, StoreError> {
    let _global = lock()?;
    let deadline = now().saturating_sub(
        cfg.virtual_accounts
            .deleted_retention_hours
            .saturating_mul(3600),
    );
    let ids: Vec<String> = {
        let mut statement = store.connection.prepare("SELECT id FROM accounts WHERE kind='virtual' AND role='user' AND status='deleted' AND deleted_at<=?1 AND NOT EXISTS(SELECT 1 FROM security_state WHERE key='virtual_purged_v1:'||accounts.id) ORDER BY deleted_at LIMIT 100")?;
        let rows = statement.query_map([number(deadline)?], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut completed = 0;
    for id in ids {
        let target = store.get(&id)?;
        let owner = store.get(
            target
                .owner_admin_id
                .as_deref()
                .ok_or(StoreError::Forbidden)?,
        )?;
        let _parent = own_reservation(store, cfg, &owner)?;
        let intent_key = format!("virtual_purge_intent_v1:{}", target.id);
        let intent: Option<String> = store
            .connection
            .query_row(
                "SELECT value FROM security_state WHERE key=?1",
                [&intent_key],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(raw) = &intent {
            let value: serde_json::Value =
                serde_json::from_str(raw).map_err(|_| StoreError::InvalidInput)?;
            if value["version"] != 1
                || value["id"] != target.id
                || value["uid"] != target.uid
                || value["home"] != target.home.to_str().ok_or(StoreError::InvalidInput)?
            {
                return Err(StoreError::Forbidden);
            }
        }
        let home_exists = match std::fs::symlink_metadata(&target.home) {
            Ok(_) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && intent.is_some() => false,
            _ => return Err(StoreError::Forbidden),
        };
        if home_exists {
            let root = root(cfg, &target)?;
            no_nested_mounts(&target.home)?;
            let _held = crate::helper::quota::Reservation::acquire(&root).map_err(io_error)?;
            if intent.is_none() {
                let tx = store
                    .connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)?;
                tx.execute("INSERT INTO security_state(key,value) VALUES(?1,?2)", params![intent_key, serde_json::json!({"version":1,"id":target.id,"uid":target.uid,"home":target.home}).to_string()])?;
                tx.execute("INSERT INTO security_state(key,value) VALUES(?1,?2) ON CONFLICT(key) DO NOTHING", params![format!("virtual_uid_retired:{}", target.uid), now().to_string()])?;
                tx.commit()?;
            }
            let executable = std::env::current_exe()?;
            let metadata = std::fs::symlink_metadata(&executable)?;
            if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
                return Err(StoreError::UntrustedStore);
            }
            let mut child = Command::new(executable)
                .args(["__purge-worker", &id])
                .env_clear()
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            let timeout = std::time::Instant::now() + std::time::Duration::from_secs(30);
            loop {
                if let Some(status) = child.try_wait()? {
                    if !status.success() {
                        return Err(StoreError::Forbidden);
                    }
                    break;
                }
                if std::time::Instant::now() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(StoreError::Forbidden);
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            std::fs::remove_dir(&target.home)?;
        }
        // Missing NSS/home is accepted only after a durable, verified purge intent.
        let generated = os_name(target.uid);
        if name_exists(&generated)? {
            let identity =
                crate::helper::security::account(&generated).map_err(|_| StoreError::Forbidden)?;
            if identity.uid != target.uid
                || identity.primary_gid != target.gid
                || identity.home_dir.as_ref() != Some(&target.home)
            {
                return Err(StoreError::Forbidden);
            }
            command("/usr/sbin/userdel", &[&generated])?;
        }
        let name_c = CString::new(generated.as_str()).map_err(|_| StoreError::InvalidInput)?;
        let mut group: libc::group = unsafe { std::mem::zeroed() };
        let mut group_result = std::ptr::null_mut();
        let mut buffer = vec![0u8; 65536];
        if unsafe {
            libc::getgrnam_r(
                name_c.as_ptr(),
                &mut group,
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut group_result,
            )
        } != 0
        {
            return Err(StoreError::Forbidden);
        }
        if !group_result.is_null() {
            if group.gr_gid != target.gid
                || (!group.gr_mem.is_null() && !unsafe { *group.gr_mem }.is_null())
            {
                return Err(StoreError::Forbidden);
            }
            command("/usr/sbin/groupdel", &[&generated])?;
        }
        let tx = store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO security_state(key,value) VALUES(?1,?2) ON CONFLICT(key) DO NOTHING",
            params![
                format!("virtual_uid_retired:{}", target.uid),
                now().to_string()
            ],
        )?;
        tx.execute(
            "INSERT INTO security_state(key,value) VALUES(?1,?2)",
            params![
                format!("virtual_purged_v1:{}", target.id),
                serde_json::json!({"version":1,"purged_at":now(),"uid":target.uid}).to_string()
            ],
        )?;
        tx.execute("UPDATE accounts SET quota_bytes=0,web_password_hash=NULL,generation=generation+1 WHERE id=?1", [&id])?;
        tx.execute("DELETE FROM session_grants WHERE account_id=?1", [&id])?;
        tx.commit()?;
        if let Some((parent_root, _, _)) = &_parent {
            native_set(
                cfg,
                parent_root,
                owner.uid,
                store
                    .effective_quota(&owner.id)?
                    .ok_or(StoreError::Forbidden)?,
            )?;
        }
        completed += 1;
    }
    Ok(completed)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn admin(store: &mut AccountStore, name: &str, uid: u32) -> Account {
        store
            .create(
                &Authority::Root,
                NewAccount {
                    username: name.into(),
                    uid,
                    gid: uid,
                    home: format!("/home/{name}").into(),
                    kind: AccountKind::Virtual,
                    role: Role::Admin,
                    owner_admin_id: None,
                    quota_bytes: None,
                },
            )
            .unwrap()
    }
    fn actor(account: &Account) -> Authority {
        Authority::Account {
            id: account.id.clone(),
            generation: account.generation,
        }
    }
    #[test]
    fn root_admin_intent_reserves_pool_and_finalizes_budget_atomically() {
        let mut store = AccountStore::memory();
        store.set_pool(&Authority::Root, 1000).unwrap();
        let intent = Intent {
            version: 1,
            owner: "__root__".into(),
            name: "tenant".into(),
            quota: 600,
            uid: 200002,
            account_id: None,
        };
        let key = key("__root__", &uuid::Uuid::new_v4().to_string()).unwrap();
        let raw = serde_json::to_string(&intent).unwrap();
        store
            .connection
            .execute(
                "INSERT INTO security_state(key,value) VALUES(?1,?2)",
                params![key, raw],
            )
            .unwrap();
        assert!(matches!(
            store.set_pool(&Authority::Root, 599),
            Err(StoreError::QuotaExceeded)
        ));
        let other = admin(&mut store, "other", 200001);
        assert!(matches!(
            store.set_admin_budget(&Authority::Root, &other.id, 401),
            Err(StoreError::QuotaExceeded)
        ));
        let tenant = store
            .create_enrolled(
                &Authority::Root,
                NewAccount {
                    username: "tenant".into(),
                    uid: 200002,
                    gid: 200002,
                    home: "/home/tealdrive/virtual-homes/tdv_200002".into(),
                    kind: AccountKind::Virtual,
                    role: Role::Admin,
                    owner_admin_id: None,
                    quota_bytes: None,
                },
                "root-provisioned-web-password",
                Some((&key, &raw)),
            )
            .unwrap();
        assert_eq!(store.effective_quota(&tenant.id).unwrap(), Some(600));
        assert_eq!(
            pending_reserved(&store.connection, "__root__", "").unwrap(),
            0
        );
        store
            .set_admin_budget(&Authority::Root, &other.id, 400)
            .unwrap();
        assert!(matches!(
            store.set_pool(&Authority::Root, 999),
            Err(StoreError::QuotaExceeded)
        ));
    }
    #[test]
    fn pending_creation_reserves_budget_and_finalizes_without_double_charge() {
        let mut store = AccountStore::memory();
        let parent = admin(&mut store, "parent", 200000);
        store.set_pool(&Authority::Root, 1000).unwrap();
        store
            .set_admin_budget(&Authority::Root, &parent.id, 1000)
            .unwrap();
        let intent = Intent {
            version: 1,
            owner: parent.id.clone(),
            name: "child".into(),
            quota: 200,
            uid: 200001,
            account_id: None,
        };
        let key = key(&parent.id, &uuid::Uuid::new_v4().to_string()).unwrap();
        let raw = serde_json::to_string(&intent).unwrap();
        store
            .connection
            .execute(
                "INSERT INTO security_state(key,value) VALUES(?1,?2)",
                params![key, raw],
            )
            .unwrap();
        assert_eq!(store.effective_quota(&parent.id).unwrap(), Some(800));
        assert_eq!(store.budget(&parent.id).unwrap()["reserved_bytes"], 200);
        assert!(matches!(
            store.set_admin_budget(&Authority::Root, &parent.id, 199),
            Err(StoreError::QuotaExceeded)
        ));
        let child = store
            .create_enrolled(
                &actor(&parent),
                NewAccount {
                    username: "child".into(),
                    uid: 200001,
                    gid: 200001,
                    home: "/home/child".into(),
                    kind: AccountKind::Virtual,
                    role: Role::User,
                    owner_admin_id: Some(parent.id.clone()),
                    quota_bytes: Some(200),
                },
                "initial-web-password",
                Some((&key, &raw)),
            )
            .unwrap();
        assert!(child.password_enrolled);
        assert_eq!(store.effective_quota(&parent.id).unwrap(), Some(800));
        assert_eq!(
            store
                .verify_password("child", "initial-web-password")
                .unwrap()
                .id,
            child.id
        );
        let saved: String = store
            .connection
            .query_row(
                "SELECT value FROM security_state WHERE key=?1",
                [key],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Intent>(&saved).unwrap().account_id,
            Some(child.id)
        );
    }
    #[test]
    fn quota_authority_and_cas_precede_external_effects_and_failure_rolls_back() {
        let mut store = AccountStore::memory();
        let parent = admin(&mut store, "parent", 200000);
        let other = admin(&mut store, "other", 200001);
        store.set_pool(&Authority::Root, 1000).unwrap();
        store
            .set_admin_budget(&Authority::Root, &parent.id, 1000)
            .unwrap();
        let child = store
            .create(
                &actor(&parent),
                NewAccount {
                    username: "child".into(),
                    uid: 200002,
                    gid: 200002,
                    home: "/home/child".into(),
                    kind: AccountKind::Virtual,
                    role: Role::User,
                    owner_admin_id: Some(parent.id.clone()),
                    quota_bytes: Some(200),
                },
            )
            .unwrap();
        let called = std::cell::Cell::new(false);
        assert!(store
            .set_child_quota_with(&actor(&other), &child.id, 1, 300, |_| {
                called.set(true);
                Ok(())
            })
            .is_err());
        assert!(!called.get());
        assert!(store
            .set_child_quota_with(&actor(&parent), &child.id, 2, 300, |_| {
                called.set(true);
                Ok(())
            })
            .is_err());
        assert!(!called.get());
        assert!(store
            .set_child_quota_with(&actor(&parent), &child.id, 1, 300, |remaining| {
                assert_eq!(remaining, 700);
                Err(StoreError::QuotaExceeded)
            })
            .is_err());
        let unchanged = store.get(&child.id).unwrap();
        assert_eq!(unchanged.generation, 1);
        assert_eq!(unchanged.quota_bytes, Some(200));
        let updated = store
            .set_child_quota_with(&actor(&parent), &child.id, 1, 300, |_| Ok(()))
            .unwrap();
        assert_eq!(updated.generation, 2);
        assert_eq!(updated.quota_bytes, Some(300));
        assert!(matches!(
            store.transition_expected(
                &actor(&parent),
                &child.id,
                super::super::store::AccountStatus::Deleted,
                true,
                1
            ),
            Err(StoreError::Conflict)
        ));
    }
}
