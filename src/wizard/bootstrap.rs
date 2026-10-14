//! First logical panel administrator. Never unlocks the Linux service account.
use super::*;
use serde::{Deserialize, Serialize};
const SECRET: &str = "/home/tealdrive/auth/bootstrap.json";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    version: u8,
    request: String,
    budget: u64,
    own: u64,
    password: zeroize::Zeroizing<String>,
    account_id: Option<String>,
}
fn read_pending() -> Result<Pending> {
    trusted_directory(Path::new(SECRET).parent().ok_or("Bootstrap parent")?)?;
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(SECRET)?;
    let m = file.metadata()?;
    if !m.is_file() || m.uid() != 0 || m.mode() & 0o077 != 0 || m.nlink() != 1 || m.len() > 4096 {
        return Err("Untrusted bootstrap credential file".into());
    }
    let mut bytes = zeroize::Zeroizing::new(Vec::new());
    std::io::Read::by_ref(&mut file)
        .take(4097)
        .read_to_end(&mut bytes)?;
    let state: Pending = serde_json::from_slice(&bytes)?;
    if state.version != 1 || uuid::Uuid::parse_str(&state.request).is_err() {
        return Err("Invalid bootstrap state".into());
    }
    Ok(state)
}
fn save(state: &Pending) -> Result<()> {
    let data = zeroize::Zeroizing::new(serde_json::to_vec_pretty(state)?);
    atomic(Path::new(SECRET), &data, 0o600, 0, 0)
}
pub(super) fn admin(budget: u64, own: u64) -> Result<()> {
    stopped()?;
    let mut cfg = config()?;
    if !cfg.virtual_accounts.enabled
        || !cfg.web_accounts.enabled
        || own == 0
        || own > budget
        || own < crate::accounts::virtuals::minimum_quota(&cfg)
    {
        return Err("Initialize virtual storage and choose a positive own quota within the delegated budget".into());
    }
    let lock_path = Path::new("/home/tealdrive/auth/bootstrap.lock");
    trusted_directory(lock_path.parent().ok_or("Lock parent")?)?;
    let held = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(lock_path)?;
    let m = held.metadata()?;
    if !m.is_file() || m.uid() != 0 || m.mode() & 0o077 != 0 || m.nlink() != 1 {
        return Err("Untrusted bootstrap lock".into());
    }
    if unsafe { libc::flock(held.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut db = store()?;
    let mut state = if Path::new(SECRET).symlink_metadata().is_ok() {
        let state = read_pending()?;
        if state.budget != budget || state.own != own {
            return Err("Existing bootstrap targets differ; no credentials reset".into());
        }
        if let Some(id) = &state.account_id {
            let account = db.get(id)?;
            return print(
                &serde_json::json!({"account":account,"credentialFile":SECRET,"existing":true,"passwordChangeRequired":account.password_change_required}),
            );
        }
        state
    } else {
        for kind in [AccountKind::Linux, AccountKind::Virtual] {
            if db.all_accounts(&Authority::Root, kind)?.iter().any(|a| {
                a.role == Role::Admin
                    && a.global_admin
                    && a.status == AccountStatus::Active
                    && cfg.account_policy.is_admin(&a.username)
            }) {
                return Err("A global panel administrator already exists; use explicit virtual account migration/provisioning".into());
            }
        }
        let password = zeroize::Zeroizing::new(format!(
            "{}{}!",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        ));
        let state = Pending {
            version: 1,
            request: uuid::Uuid::new_v4().to_string(),
            budget,
            own,
            password,
            account_id: None,
        };
        save(&state)?;
        state
    };
    let account = crate::accounts::virtuals::create_admin(
        &mut db,
        &cfg,
        "tealdrive",
        budget,
        &state.password,
        &state.request,
    )?;
    // Offline command: the random password is never exposed before restriction.
    db.require_password_change(&Authority::Root, &account.id)?;
    crate::accounts::virtuals::set_own_quota(&mut db, &cfg, &account.id, own)?;
    let account = db.set_panel_scope(&Authority::Root, &account.id, true)?;
    if !cfg
        .account_policy
        .admin_users
        .iter()
        .any(|name| name == "tealdrive")
    {
        cfg.account_policy.admin_users.push("tealdrive".into());
    }
    save_configs(&cfg)?;
    state.account_id = Some(account.id.clone());
    save(&state)?;
    print(
        &serde_json::json!({"account":account,"credentialFile":SECRET,"passwordChangeRequired":true,"linuxServiceLoginEnabled":false}),
    )
}
