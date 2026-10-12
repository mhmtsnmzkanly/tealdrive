//! Root-only fixture bootstrap, never a server/helper command or production installer.
use std::{os::unix::fs::MetadataExt, path::Path};
use tealdrive::{
    accounts::store::{AccountKind, AccountStore, Authority, NewAccount, Role},
    config::{AllowedRoot, AppConfig, RootId},
};
#[derive(serde::Deserialize)]
struct User {
    username: String,
    role: Role,
    password: zeroize::Zeroizing<String>,
    bootstrap: bool,
}
#[derive(serde::Deserialize)]
struct Fixture {
    users: Vec<User>,
    config_dir: String,
    audit_path: String,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if unsafe { libc::geteuid() } != 0
        || std::env::var("TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE").as_deref() != Ok("1")
    {
        return Err("Explicit root fixture opt-in required".into());
    }
    let path = std::env::args().nth(1).ok_or("Private manifest required")?;
    let metadata = std::fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o077 != 0 {
        return Err("Private root manifest required".into());
    }
    let raw = zeroize::Zeroizing::new(std::fs::read_to_string(path)?);
    let fixture: Fixture = serde_json::from_str(&raw)?;
    if fixture.users.len() != 3
        || !fixture
            .config_dir
            .starts_with("/home/tealdrive/policy/.admin-acceptance-")
    {
        return Err("Invalid fixture".into());
    }
    let mut cfg: AppConfig = toml::from_str(&String::from_utf8(
        tealdrive::helper::security::trusted_read(
            Path::new(tealdrive::layout::HELPER_CONFIG),
            false,
        )?,
    )?)?;
    cfg.web_accounts.self_registration = false;
    let mut store = AccountStore::open(Path::new(tealdrive::layout::ACCOUNTS))?;
    for (index, user) in fixture.users.iter().enumerate() {
        if !user.username.starts_with("tdaccept")
            || user.username.len() > 31
            || store.find_username(&user.username)?.is_some()
        {
            return Err("Fixture name occupied or invalid".into());
        }
        let identity = tealdrive::helper::security::account(&user.username)?;
        let home = identity.home_dir.ok_or("Missing home")?;
        if identity.uid < 1000
            || identity.uid >= cfg.web_accounts.virtual_min_uid
            || home != Path::new("/home").join(&user.username)
        {
            return Err("Invalid OS fixture identity".into());
        }
        cfg.roots.push(AllowedRoot {
            root_id: RootId::new(format!("accept_{index}")),
            base_path: home.clone(),
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
            quota_bytes: None,
        });
        if user.role == Role::Admin {
            cfg.account_policy.admin_users.push(user.username.clone());
        }
        if user.bootstrap {
            let account = store.create(
                &Authority::Root,
                NewAccount {
                    username: user.username.clone(),
                    uid: identity.uid,
                    gid: identity.primary_gid,
                    home,
                    kind: AccountKind::Linux,
                    role: user.role,
                    owner_admin_id: None,
                    quota_bytes: None,
                },
            )?;
            store.set_password(&Authority::Root, &account.id, &user.password)?;
        }
    }
    cfg.validate()?;
    std::fs::write(
        Path::new(&fixture.config_dir).join("helper.toml"),
        toml::to_string_pretty(&cfg)?,
    )?;
    let mut denied = cfg.clone();
    denied
        .web_accounts
        .denied_users
        .push(fixture.users[2].username.clone());
    std::fs::write(
        Path::new(&fixture.config_dir).join("helper-deny-pending.toml"),
        toml::to_string_pretty(&denied)?,
    )?;
    cfg.server.bind_addr = "127.0.0.1:13102".into();
    cfg.server.behind_tls_proxy = false;
    cfg.session.secure_cookies = false;
    cfg.security.allowed_origins = vec!["http://127.0.0.1:13102".into()];
    cfg.audit.path = Some(fixture.audit_path.into());
    cfg.validate()?;
    std::fs::write(
        Path::new(&fixture.config_dir).join("server.toml"),
        toml::to_string_pretty(&cfg)?,
    )?;
    Ok(())
}
