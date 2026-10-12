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
fn lock() -> Result<File, StoreError> {
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
        let retired:bool=store.connection.query_row("SELECT EXISTS(SELECT 1 FROM accounts WHERE uid=?1) OR EXISTS(SELECT 1 FROM security_state WHERE key=?2)",params![uid,format!("virtual_uid_retired:{uid}")],|r|r.get(0))?;
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
        if store.find_username(name)?.is_some() || crate::helper::security::account(name).is_ok() {
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
    let identity = crate::helper::security::account(&os_name);
    if identity.is_err() {
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
    for target in store.list(&Authority::Root, AccountKind::Virtual)? {
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
