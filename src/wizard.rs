//! Offline installation/control CLI. Never setuid; Root commands require real UID0.
use crate::{
    accounts::{
        policies::PolicyLayer,
        store::{AccountKind, AccountStatus, AccountStore, Authority, NewAccount, Role},
    },
    config::AppConfig,
    helper::account_gateway::{AccountCommand, AccountReply},
    layout,
};
use std::{
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const BINARIES: [&str; 4] = [
    "tealdrive-server",
    "tealdrive-helper",
    "tealdrive-wizard",
    "tealdrive-ctl",
];
const UNIT: &str = "tealdrive.service";
mod bootstrap;
mod migrate_admin;
mod storage;
mod tls;
mod tls_renewal;
fn root() -> Result<()> {
    if unsafe { libc::getuid() } != 0 || unsafe { libc::geteuid() } != 0 {
        return Err(
            "This command requires a real Root session; the Wizard must not be setuid.".into(),
        );
    }
    Ok(())
}
fn trusted_directory(path: &Path) -> Result<()> {
    for part in path.ancestors() {
        let metadata = std::fs::symlink_metadata(part)?;
        if !metadata.is_dir() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
            return Err("Untrusted directory ownership, permissions or symlink.".into());
        }
    }
    Ok(())
}
fn atomic(path: &Path, data: &[u8], mode: u32, uid: u32, gid: u32) -> Result<()> {
    let parent = path.parent().ok_or("Invalid output path")?;
    let temporary = parent.join(format!(".tealdrive-write-{}", uuid::Uuid::new_v4()));
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _cleanup = Cleanup(temporary.clone());
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&temporary)?;
    file.write_all(data)?;
    file.sync_all()?;
    if unsafe { libc::fchown(file.as_raw_fd(), uid, gid) } != 0
        || unsafe { libc::fchmod(file.as_raw_fd(), mode) } != 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    std::fs::rename(&temporary, path)?;
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}
fn command(program: &str, args: &[&str]) -> Result<()> {
    let metadata = std::fs::symlink_metadata(program)?;
    if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
        return Err("Untrusted system utility".into());
    }
    if !Command::new(program)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .status()?
        .success()
    {
        return Err("System utility failed; no broader fallback was attempted.".into());
    }
    Ok(())
}
fn config() -> Result<AppConfig> {
    Ok(crate::helper::security::trusted_config()?)
}
fn store() -> Result<AccountStore> {
    Ok(AccountStore::open(Path::new(layout::ACCOUNTS))?)
}
fn print<T: serde::Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
fn save_configs(cfg: &AppConfig) -> Result<()> {
    cfg.validate()?;
    let body = toml::to_string_pretty(cfg)?;
    // Helper ceiling first; a half-published update can restrict, never delegate Root.
    atomic(
        Path::new(layout::HELPER_CONFIG),
        body.as_bytes(),
        0o600,
        0,
        0,
    )?;
    atomic(
        Path::new(layout::SERVER_CONFIG),
        body.as_bytes(),
        0o644,
        0,
        0,
    )?;
    Ok(())
}
// Account management must not replace bind/origin/audit settings from the helper ceiling.
fn save_account_config(cfg: &AppConfig) -> Result<()> {
    let mut server = AppConfig::from_file(layout::SERVER_CONFIG)?;
    server.account_policy = cfg.account_policy.clone();
    server.web_accounts = cfg.web_accounts.clone();
    cfg.validate()?;
    server.validate()?;
    atomic(
        Path::new(layout::HELPER_CONFIG),
        toml::to_string_pretty(cfg)?.as_bytes(),
        0o600,
        0,
        0,
    )?;
    atomic(
        Path::new(layout::SERVER_CONFIG),
        toml::to_string_pretty(&server)?.as_bytes(),
        0o644,
        0,
        0,
    )?;
    Ok(())
}
fn approve_linux(name: &str, allow_nologin: bool) -> Result<()> {
    root()?;
    let identity = crate::helper::security::account(name)?;
    let mut cfg = config()?;
    if allow_nologin
        && !cfg
            .web_accounts
            .allowed_nologin_users
            .contains(&identity.username)
    {
        cfg.web_accounts
            .allowed_nologin_users
            .push(identity.username.clone());
    }
    crate::helper::account_gateway::linux_policy(&cfg, &identity, true)
        .map_err(|_| "Identity is excluded by Root enrollment policy")?;
    let home = identity.home_dir.clone().ok_or("Missing NSS home")?;
    if !home.is_absolute() || home == Path::new("/") {
        return Err("Unsafe NSS home".into());
    }
    let mut store = store()?;
    let account = match store.find_username(name)? {
        Some(account)
            if account.kind == AccountKind::Linux
                && account.uid == identity.uid
                && account.gid == identity.primary_gid
                && account.home == home =>
        {
            account
        }
        Some(_) => {
            return Err("Stored identity differs from NSS; explicit migration required".into())
        }
        None => store.create(
            &Authority::Root,
            NewAccount {
                username: identity.username.clone(),
                uid: identity.uid,
                gid: identity.primary_gid,
                home,
                kind: AccountKind::Linux,
                role: Role::User,
                owner_admin_id: None,
                quota_bytes: None,
            },
        )?,
    };
    if !cfg
        .account_policy
        .allowed_users
        .contains(&identity.username)
    {
        cfg.account_policy.allowed_users.push(identity.username);
    }
    save_account_config(&cfg)?;
    print(&account)
}
fn bytes(value: &str) -> Result<u64> {
    let units = [
        ("TiB", 1u64 << 40),
        ("GiB", 1u64 << 30),
        ("MiB", 1u64 << 20),
        ("KiB", 1u64 << 10),
        ("B", 1),
    ];
    for (suffix, multiplier) in units {
        if let Some(number) = value.strip_suffix(suffix) {
            return number
                .parse::<u64>()?
                .checked_mul(multiplier)
                .ok_or_else(|| "Size overflow".into());
        }
    }
    Ok(value.parse()?)
}
fn secret(label: &str) -> Result<zeroize::Zeroizing<String>> {
    let mut tty = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")?;
    let fd = tty.as_raw_fd();
    let mut old: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut old) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    struct Restore(libc::c_int, libc::termios);
    impl Drop for Restore {
        fn drop(&mut self) {
            unsafe {
                libc::tcsetattr(self.0, libc::TCSAFLUSH, &self.1);
            }
        }
    }
    let _restore = Restore(fd, old);
    let mut mode = old;
    mode.c_lflag &= !libc::ECHO;
    if unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &mode) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    write!(tty, "{label}: ")?;
    tty.flush()?;
    let mut value = zeroize::Zeroizing::new(Vec::new());
    loop {
        let mut byte = [0u8; 1];
        tty.read_exact(&mut byte)?;
        if byte[0] == b'\n' {
            break;
        }
        if value.len() >= 1024 {
            return Err("Password too long".into());
        }
        value.push(byte[0]);
    }
    writeln!(tty)?;
    Ok(zeroize::Zeroizing::new(String::from_utf8(std::mem::take(
        &mut *value,
    ))?))
}
fn local(command: AccountCommand) -> Result<AccountReply> {
    let helper = Path::new(layout::HOME).join("bin/tealdrive-helper");
    let client = crate::helper::client::HelperProcessClient::new(helper, Duration::from_secs(15));
    client
        .account_command(command)
        .map_err(|_| "Local helper operation was denied or unavailable.".into())
}
pub fn control(args: Vec<String>) -> Result<()> {
    let action = args.first().map(String::as_str).unwrap_or("status");
    if args.len() > 1 || !matches!(action, "start" | "stop" | "restart" | "reload") {
        return control_inner(args);
    }
    root()?;
    let caller = crate::accounts::grants::CallerBinding::capture()?;
    let limits = config().map(|cfg| cfg.control_audit).unwrap_or_default();
    let intent = (|| {
        let mut store = store()?;
        Ok::<_, Box<dyn std::error::Error>>(store.begin_control(
            &limits,
            &caller,
            &format!("Service_{action}"),
            None,
            None,
            crate::session::store::InMemorySessionStore::now_epoch_seconds(),
        )?)
    })();
    let intent = match intent {
        Ok(record) => Some(record),
        // Root must be able to stop a broken or exhausted installation for repair.
        Err(_) if action == "stop" => None,
        Err(error) => return Err(error),
    };
    let result = control_inner(args);
    if let Some(record) = intent {
        if let Ok(mut store) = store() {
            let _ = store.finish_control(
                &record,
                result.is_ok(),
                if result.is_ok() {
                    None
                } else {
                    Some("ServiceCommandFailed")
                },
                crate::session::store::InMemorySessionStore::now_epoch_seconds(),
            );
        }
    }
    result
}
fn control_inner(args: Vec<String>) -> Result<()> {
    let operation = args.first().map(String::as_str).unwrap_or("status");
    if args.len() > 1 {
        return Err("Usage: tealdrive-ctl start|stop|restart|reload|status".into());
    }
    match operation {
        "status" => command("/usr/bin/systemctl", &["status", "--no-pager", UNIT]),
        "start" | "stop" | "restart" => {
            root()?;
            if operation != "stop" {
                config()?.validate()?;
            }
            command("/usr/bin/systemctl", &[operation, UNIT])
        }
        "reload" => {
            root()?;
            config()?.validate()?;
            command(
                "/usr/bin/systemctl",
                &["kill", "--kill-who=main", "--signal=HUP", UNIT],
            )
        }
        _ => Err("Usage: tealdrive-ctl start|stop|restart|reload|status".into()),
    }
}
pub fn run(args: Vec<String>) -> Result<()> {
    let action = args.first().map(String::as_str);
    let audited = unsafe { libc::getuid() } == 0
        && unsafe { libc::geteuid() } == 0
        && matches!(
            action,
            Some(
                "install"
                    | "update"
                    | "rollback"
                    | "repair"
                    | "uninstall"
                    | "restore"
                    | "account"
                    | "config"
                    | "policy"
                    | "pool"
                    | "budget"
                    | "virtual"
                    | "gc"
                    | "storage"
                    | "tls"
                    | "channel"
                    | "bootstrap"
            )
        );
    let readonly = matches!(
        args.get(1).map(String::as_str),
        Some("show" | "validate" | "list" | "get" | "pending" | "status" | "check")
    );
    let restore_or_repair = matches!(action, Some("restore" | "repair"));
    let control = if audited && !readonly && Path::new(layout::ACCOUNTS).exists() {
        match store() {
            Ok(mut store) => {
                let caller = crate::accounts::grants::CallerBinding::capture()?;
                let limits = config().map(|cfg| cfg.control_audit).unwrap_or_default();
                let action = format!("Wizard_{}", action.unwrap_or("unknown"));
                let record = store.begin_control(
                    &limits,
                    &caller,
                    &action,
                    None,
                    None,
                    crate::session::store::InMemorySessionStore::now_epoch_seconds(),
                )?;
                Some(record)
            }
            Err(_) if matches!(action, Some("repair" | "restore" | "install")) => None,
            Err(error) => return Err(error),
        }
    } else {
        None
    };
    let result = run_inner(args);
    if let Some(record) = control.as_ref() {
        if let Ok(mut store) = store() {
            let code = if result.is_ok() {
                None
            } else {
                Some("WizardCommandFailed")
            };
            let finished = store.finish_control(
                record,
                result.is_ok(),
                code,
                crate::session::store::InMemorySessionStore::now_epoch_seconds(),
            );
            if finished.is_err() && result.is_ok() && restore_or_repair {
                let caller = crate::accounts::grants::CallerBinding::capture()?;
                let limits = config().map(|cfg| cfg.control_audit).unwrap_or_default();
                if let Ok(completed) = store.begin_control(
                    &limits,
                    &caller,
                    "WizardRecoveryCompleted",
                    None,
                    None,
                    crate::session::store::InMemorySessionStore::now_epoch_seconds(),
                ) {
                    let _ = store.finish_control(
                        &completed,
                        true,
                        None,
                        crate::session::store::InMemorySessionStore::now_epoch_seconds(),
                    );
                }
            }
        }
    }
    result
}
fn run_inner(args: Vec<String>) -> Result<()> {
    let parts: Vec<&str> = args.iter().map(String::as_str).collect();
    match parts.as_slice() {
        [] | ["--help"] | ["help"] => {
            println!("TealDrive Wizard (never setuid)\nOwn UID: init | status | revoke | policy get | policy personal <generation> <rules.json>\nRoot: install/update --bundle <directory> | repair | uninstall | diagnose | config validate/show/apply <file> | account list/approve <username>/admin <id>/user <id>/scope <id> global|tenant/allow <id>/deny <id>/password <id> | tls status/prepare <DNS-name>/activate <DNS-name> [--caddy <Caddyfile>]/renew <DNS-name> --caddy <Caddyfile> | storage init <physical-size>/init --pool-size <net-size>/expand --pool-size <net-size>/status/check/mount/sync | pool set <size> | budget set <id> <size>/own set <id> <size> | policy root/admin/personal <id> <generation> <rules.json> | backup <new-directory> | restore <directory>\nService: tealdrive-ctl start|stop|restart|reload|status");
            Ok(())
        }
        ["init"] => {
            let linux = secret("Linux password")?;
            let web = secret("New web password (12+ characters)")?;
            let confirm = secret("Confirm web password")?;
            if *web != *confirm {
                return Err("Passwords do not match".into());
            }
            print(&local(AccountCommand::LocalEnroll {
                linux_password: linux,
                web_password: web,
            })?)
        }
        ["status"] => print(&local(AccountCommand::LocalStatus)?),
        ["revoke"] => print(&local(AccountCommand::LocalRevoke)?),
        ["policy", "get"] => print(&local(AccountCommand::LocalPolicy)?),
        ["policy", "personal", generation, file] => {
            let rules = serde_json::from_slice(&std::fs::read(file)?)?;
            print(&local(AccountCommand::LocalSetPolicy {
                expected_generation: generation.parse()?,
                rules,
            })?)
        }
        ["service", rest @ ..] => control(rest.iter().map(|part| part.to_string()).collect()),
        ["config", "validate"] => {
            root()?;
            config()?.validate()?;
            println!("Configuration valid.");
            Ok(())
        }
        ["config", "show"] => {
            root()?;
            print(&config()?)
        }
        ["config", "apply", file] => {
            root()?;
            let cfg = AppConfig::from_file(file)?;
            save_configs(&cfg)?;
            println!("Configuration saved; restart if transport or bind settings changed.");
            Ok(())
        }
        ["account", "list"] => {
            root()?;
            let store = store()?;
            let mut accounts = store.all_accounts(&Authority::Root, AccountKind::Linux)?;
            accounts.extend(store.all_accounts(&Authority::Root, AccountKind::Virtual)?);
            print(&accounts)
        }
        ["account", "approve", name] => approve_linux(name, false),
        ["account", "approve", name, "--allow-nologin"] => approve_linux(name, true),
        ["account", "scope", id, scope @ ("global" | "tenant")] => {
            root()?;
            let account = store()?.set_panel_scope(&Authority::Root, id, *scope == "global")?;
            let mut cfg = config()?;
            cfg.account_policy
                .admin_users
                .retain(|name| name != &account.username);
            if *scope == "global" {
                cfg.account_policy
                    .admin_users
                    .push(account.username.clone());
            }
            save_account_config(&cfg)?;
            print(&account)
        }
        ["account", role @ ("admin" | "user"), id] => {
            root()?;
            let mut store = store()?;
            let account = store.set_role(
                &Authority::Root,
                id,
                if *role == "admin" {
                    Role::Admin
                } else {
                    Role::User
                },
            )?;
            let mut cfg = config()?;
            cfg.account_policy
                .admin_users
                .retain(|name| name != &account.username);
            if *role == "admin" {
                cfg.account_policy
                    .admin_users
                    .push(account.username.clone());
            }
            save_account_config(&cfg)?;
            print(&account)
        }
        ["account", action @ ("allow" | "deny"), id] => {
            root()?;
            let mut store = store()?;
            print(&store.transition(
                &Authority::Root,
                id,
                if *action == "allow" {
                    AccountStatus::Active
                } else {
                    AccountStatus::Suspended
                },
                false,
            )?)
        }
        ["account", "password", id] => {
            root()?;
            let password = secret("New web password")?;
            let confirm = secret("Confirm web password")?;
            if *password != *confirm {
                return Err("Passwords do not match".into());
            }
            print(&store()?.set_password(&Authority::Root, id, &password)?)
        }
        ["ui", "check", directory] => {
            root()?;
            let path = std::env::current_dir()?.join(directory);
            print(&crate::ui::package::load(&path)?.manifest)
        }
        ["ui", "install", "--bundle", directory] => {
            root()?;
            install_ui(&std::env::current_dir()?.join(directory))
        }
        ["ui", "disable"] => {
            root()?;
            set_ui(None)?;
            println!("Embedded UI selected; reload Root config or restart.");
            Ok(())
        }
        ["pending", "list"] => {
            root()?;
            print(&crate::accounts::virtuals::pending(&store()?)?)
        }
        ["pending", "cancel", key] => {
            root()?;
            crate::accounts::virtuals::cancel_pending(&mut store()?, &config()?, key)?;
            println!("Unstarted provisioning canceled; UID retired.");
            Ok(())
        }
        ["__purge-worker", id] => {
            root()?;
            crate::accounts::virtuals::purge_worker(id)?;
            Ok(())
        }
        ["__trash-expiry-worker", account_id, operation_id] => {
            root()?;
            crate::accounts::lifecycle::worker(account_id, operation_id)?;
            Ok(())
        }
        ["audit", "list", limit] => {
            root()?;
            print(&store()?.control_records(&Authority::Root, limit.parse()?)?)
        }
        ["__staging-expiry-worker", account_id, operation_id, as_of] => {
            root()?;
            crate::accounts::staging::worker(account_id, operation_id, as_of.parse()?)?;
            Ok(())
        }
        ["gc"] => {
            root()?;
            let cfg = config()?;
            let mut store = store()?;
            let purged = crate::accounts::virtuals::purge_deleted(&mut store, &cfg)?;
            let cleanup = crate::accounts::lifecycle::cleanup(&store, &cfg)?;
            let staged = crate::accounts::staging::cleanup(&store, &cfg)?;
            let expired = store.gc_operations(
                &cfg.journal,
                crate::session::store::InMemorySessionStore::now_epoch_seconds(),
            )?;
            print(
                &serde_json::json!({"virtual_purged":purged,"operation_logs_expired":expired,"trash_cleaned":cleanup.trash_cleaned,"trash_skipped":cleanup.trash_skipped,"staging_cleaned":staged.staging_cleaned,"staging_skipped":staged.staging_skipped}),
            )
        }
        ["account", "migrate-admin", source, name, budget, own] => {
            root()?;
            migrate_admin::migrate(source, name, bytes(budget)?, bytes(own)?)
        }
        ["bootstrap", "admin", budget, own] => {
            root()?;
            bootstrap::admin(bytes(budget)?, bytes(own)?)
        }
        ["account", "require-password-change", id] => {
            root()?;
            print(&store()?.require_password_change(&Authority::Root, id)?)
        }
        ["virtual", "admin", "create", name, budget, request] => {
            root()?;
            let password = secret("New virtual admin web password")?;
            let confirmation = secret("Confirm web password")?;
            if *password != *confirmation {
                return Err("Passwords do not match".into());
            }
            print(&crate::accounts::virtuals::create_admin(
                &mut store()?,
                &config()?,
                name,
                bytes(budget)?,
                &password,
                request,
            )?)
        }
        ["storage", "init", "--pool-size", size] => {
            root()?;
            storage::init_pool(bytes(size)?)
        }
        ["storage", "expand", "--pool-size", size] => {
            root()?;
            storage::expand(bytes(size)?)
        }
        ["storage", "init", size] => {
            root()?;
            storage::init(bytes(size)?)
        }
        ["channel", "keygen", path] => {
            root()?;
            let _lock = crate::accounts::virtuals::lock()?;
            let path = Path::new(path);
            trusted_directory(path.parent().ok_or("Invalid key path")?)?;
            if path.symlink_metadata().is_ok() {
                return Err("Channel key already exists; use a new path for rotation".into());
            }
            let pair = tealdrive_channel::builder().generate_keypair()?;
            let key = zeroize::Zeroizing::new(pair.private);
            atomic(path, &key, 0o640, 0, service_identity()?.primary_gid)?;
            let public: String = pair.public.iter().map(|b| format!("{b:02x}")).collect();
            print(
                &serde_json::json!({"publicKey":public,"keyPath":path,"protocol":tealdrive_channel::PATTERN,"enabled":false}),
            )
        }
        ["tls", "prepare", name] => {
            root()?;
            tls_renewal::prepare(name)
        }
        ["tls", "activate", name] => {
            root()?;
            tls_renewal::activate(name, None)
        }
        ["tls", "activate", name, "--caddy", path] => {
            root()?;
            tls_renewal::activate(name, Some(Path::new(path)))
        }
        ["tls", "renew", name, "--caddy", path] => {
            root()?;
            tls_renewal::renew(name, Path::new(path))
        }
        ["tls", "auto", name, "--caddy", path] => {
            root()?;
            tls_renewal::automatic(name, Path::new(path))
        }
        ["tls", "status"] => {
            root()?;
            tls::status()
        }
        ["storage", "check"] => {
            root()?;
            storage::check()
        }
        ["storage", "status"] => {
            root()?;
            print(&storage::inspect()?)
        }
        ["storage", "mount"] => {
            root()?;
            storage::mount()
        }
        ["storage", "sync"] => {
            root()?;
            crate::accounts::virtuals::sync_limits(&mut store()?, &config()?)?;
            println!("Virtual native limits synchronized with protected metadata.");
            Ok(())
        }
        ["pool", "set", size] => {
            root()?;
            let _lock = crate::accounts::virtuals::lock()?;
            if Path::new("/home/tealdrive/storage/expansion.json")
                .symlink_metadata()
                .is_ok()
            {
                return Err("Resume pending storage expansion before changing the pool".into());
            }
            storage::check_pool(bytes(size)?)?;
            store()?.set_pool(&Authority::Root, bytes(size)?)?;
            println!("Root pool updated.");
            Ok(())
        }
        ["budget", "own", "set", id, size] => {
            root()?;
            crate::accounts::virtuals::set_own_quota(&mut store()?, &config()?, id, bytes(size)?)?;
            println!("Admin fixed own-file quota updated; sessions must refresh.");
            Ok(())
        }
        ["budget", "set", id, size] => {
            root()?;
            crate::accounts::virtuals::set_budget(&mut store()?, &config()?, id, bytes(size)?)?;
            println!("Admin budget updated.");
            Ok(())
        }
        ["policy", layer @ ("root" | "admin" | "personal"), id, generation, file] => {
            root()?;
            let rules = serde_json::from_slice(&std::fs::read(file)?)?;
            print(&store()?.set_policy(
                &Authority::Root,
                id,
                generation.parse()?,
                match *layer {
                    "root" => PolicyLayer::Root,
                    "admin" => PolicyLayer::Admin,
                    _ => PolicyLayer::Personal,
                },
                rules,
            )?)
        }
        ["backup", directory] => {
            root()?;
            backup(Path::new(directory))
        }
        ["restore", directory] => {
            root()?;
            restore(Path::new(directory))
        }
        ["install", "--bundle", directory, "--pool-size", pool, "--admin-own-quota", own] => {
            root()?;
            let pool = bytes(pool)?;
            let own = bytes(own)?;
            if own == 0 || own > pool {
                return Err("Admin own quota must fit the pool".into());
            }
            install(Path::new(directory), false)?;
            storage::init_pool(pool)?;
            bootstrap::admin(pool, own)
        }
        ["install", "--bundle", directory] | ["update", "--bundle", directory] => {
            root()?;
            install(Path::new(directory), parts[0] == "update")
        }
        ["rollback", "--bundle", directory] => {
            root()?;
            stopped()?;
            publish_bundle(Path::new(directory))?;
            println!("Binaries restored. Authentication generations, keys, DB and user data were preserved.");
            Ok(())
        }
        ["repair"] => {
            root()?;
            repair()
        }
        ["diagnose"] => {
            root()?;
            diagnose()
        }
        ["uninstall"] => {
            root()?;
            if Path::new("/etc/systemd/system/tealdrive-gc.timer").exists() {
                command(
                    "/usr/bin/systemctl",
                    &["disable", "--now", "tealdrive-gc.timer"],
                )?;
            }
            control(vec!["stop".into()])?;
            stopped()?;
            // Remove only our exact Root-owned links; preserve unrelated PATH entries.
            for name in BINARIES {
                let link = Path::new("/usr/local/bin").join(name);
                if let Ok(metadata) = std::fs::symlink_metadata(&link) {
                    if metadata.file_type().is_symlink()
                        && metadata.uid() == 0
                        && std::fs::read_link(&link)?
                            == Path::new(layout::HOME).join("bin").join(name)
                    {
                        std::fs::remove_file(link)?;
                    }
                }
            }
            for name in BINARIES {
                let path = Path::new(layout::HOME).join("bin").join(name);
                if path.exists() {
                    std::fs::remove_file(path)?;
                }
            }
            println!("Binaries removed. Configuration, keys, DB and user data are preserved.");
            Ok(())
        }
        _ => Err("Unknown command/options. Run tealdrive-wizard --help.".into()),
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u8,
    version: String,
    sha256: std::collections::BTreeMap<String, String>,
}
fn hash_file(path: &Path) -> Result<String> {
    use sha2::Digest;
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != 0
        || metadata.mode() & 0o022 != 0
        || metadata.len() > 536870912
    {
        return Err("Untrusted file".into());
    }
    let mut digest = sha2::Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn verify_bundle(directory: &Path) -> Result<Manifest> {
    let absolute = if directory.is_absolute() {
        directory.to_path_buf()
    } else {
        std::env::current_dir()?.join(directory)
    };
    let directory = absolute.as_path();
    trusted_directory(directory)?;
    let raw = crate::helper::security::trusted_read(&directory.join("manifest.json"), false)?;
    let manifest: Manifest = serde_json::from_slice(&raw)?;
    if manifest.schema_version != 1
        || manifest.version.is_empty()
        || manifest.version.len() > 64
        || !manifest
            .version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        || manifest.sha256.len() != BINARIES.len()
    {
        return Err("Unsupported bundle manifest".into());
    }
    for name in BINARIES {
        let expected = manifest.sha256.get(name).ok_or("Bundle binary missing")?;
        if expected.len() != 64 || hash_file(&directory.join(name))? != *expected {
            return Err("Bundle checksum mismatch".into());
        }
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(directory.join(name))?;
        let mut magic = [0u8; 4];
        file.read_exact(&mut magic)?;
        if magic != *b"\x7fELF" {
            return Err("Bundle binary is not ELF".into());
        }
    }
    Ok(manifest)
}
fn service_identity() -> Result<crate::auth::AuthSuccess> {
    match crate::helper::security::account("tealdrive") {
        Ok(identity) => {
            if identity.uid == 0
                || !matches!(
                    identity.shell.as_deref(),
                    Some("/usr/sbin/nologin" | "/sbin/nologin" | "/bin/false")
                )
            {
                return Err("Existing tealdrive identity is unsafe".into());
            }
            Ok(identity)
        }
        Err(_) => {
            command(
                "/usr/sbin/useradd",
                &[
                    "--system",
                    "--user-group",
                    "--no-create-home",
                    "--home-dir",
                    layout::HOME,
                    "--shell",
                    "/usr/sbin/nologin",
                    "tealdrive",
                ],
            )?;
            Ok(crate::helper::security::account("tealdrive")?)
        }
    }
}
fn directory(path: &Path, uid: u32, gid: u32, mode: u32) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_dir() => return Err("Unsafe installation directory".into()),
        Ok(metadata) if metadata.uid() != uid && metadata.uid() != 0 => {
            return Err("Refusing to adopt a foreign-owned directory".into())
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => std::fs::create_dir(path)?,
        Err(error) => return Err(error.into()),
    }
    let path_c = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())?;
    if unsafe { libc::chown(path_c.as_ptr(), uid, gid) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    Ok(())
}
fn prepare_layout(identity: &crate::auth::AuthSuccess) -> Result<()> {
    let base = Path::new(layout::HOME);
    directory(base, 0, 0, 0o755)?;
    for (name, uid, gid, mode) in [
        ("bin", 0, 0, 0o755),
        ("policy", 0, 0, 0o755),
        ("auth", 0, 0, 0o700),
        ("storage", 0, 0, 0o700),
        ("virtual-homes", 0, 0, 0o711),
        ("config", identity.uid, identity.primary_gid, 0o700),
        ("data", identity.uid, identity.primary_gid, 0o700),
        ("logs", 0, 0, 0o711),
        ("tls", 0, identity.primary_gid, 0o750),
    ] {
        directory(&base.join(name), uid, gid, mode)?;
    }
    directory(
        &base.join("logs/server"),
        identity.uid,
        identity.primary_gid,
        0o700,
    )?;
    directory(&base.join("logs/security"), 0, 0, 0o700)?;
    directory(
        &base.join("data/runtime"),
        identity.uid,
        identity.primary_gid,
        0o700,
    )?;
    Ok(())
}
fn running() -> Result<bool> {
    for entry in std::fs::read_dir("/proc")? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().parse::<u32>().is_err() {
            continue;
        }
        let Ok(args) = std::fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        let command = args.split(|byte| *byte == 0).next().unwrap_or_default();
        let named = std::str::from_utf8(command)
            .ok()
            .and_then(|p| Path::new(p).file_name())
            .is_some_and(|n| n == "tealdrive-server");
        let executable = std::fs::read_link(entry.path().join("exe"))
            .ok()
            .is_some_and(|p| p.file_name().is_some_and(|n| n == "tealdrive-server"));
        if named || executable {
            return Ok(true);
        }
    }
    Ok(false)
}
fn stopped() -> Result<()> {
    if running()? {
        return Err("Stop the server before this offline operation. Use tealdrive-ctl stop; manually launched candidates must also exit.".into());
    }
    Ok(())
}
fn set_ui(directory: Option<PathBuf>) -> Result<()> {
    let mut cfg = AppConfig::from_file(layout::SERVER_CONFIG)?;
    cfg.server.static_dir = directory;
    cfg.validate()?;
    atomic(
        Path::new(layout::SERVER_CONFIG),
        toml::to_string_pretty(&cfg)?.as_bytes(),
        0o644,
        0,
        0,
    )
}
fn install_ui(source: &Path) -> Result<()> {
    let package = crate::ui::package::load(source)?;
    let base = Path::new(layout::HOME).join("ui");
    trusted_directory(Path::new(layout::HOME))?;
    directory(&base, 0, 0, 0o755)?;
    let packages = base.join("packages");
    directory(&packages, 0, 0, 0o755)?;
    let target = packages.join(format!(
        "{}-{}-{}",
        package.manifest.name,
        package.manifest.version,
        uuid::Uuid::new_v4()
    ));
    directory(&target, 0, 0, 0o755)?;
    for (name, contents) in &package.files {
        let file = target.join(name);
        let relative = Path::new(name).parent().ok_or("Invalid UI asset")?;
        let mut parent = target.clone();
        for component in relative.components() {
            parent.push(component);
            directory(&parent, 0, 0, 0o755)?;
        }
        atomic(&file, contents, 0o644, 0, 0)?;
    }
    atomic(
        &target.join("manifest.json"),
        &serde_json::to_vec_pretty(&package.manifest)?,
        0o644,
        0,
        0,
    )?;
    crate::ui::package::load(&target)?;
    set_ui(Some(target))?;
    println!("Root-approved UI package installed; reload Root config. /ui/default always opens the embedded UI. Package code is trusted application code and can act as the signed-in user.");
    Ok(())
}
fn binary_backup(directory: &Path) -> Result<()> {
    trusted_directory(directory.parent().ok_or("Invalid backup path")?)?;
    std::fs::create_dir(directory)?;
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))?;
    let mut hashes = std::collections::BTreeMap::new();
    for name in BINARIES {
        let source = Path::new(layout::HOME).join("bin").join(name);
        hash_file(&source)?;
        atomic(&directory.join(name), &std::fs::read(source)?, 0o600, 0, 0)?;
        hashes.insert(name.into(), hash_file(&directory.join(name))?);
    }
    atomic(
        &directory.join("manifest.json"),
        &serde_json::to_vec(&Manifest {
            schema_version: 1,
            version: "previous-binaries-v1".into(),
            sha256: hashes,
        })?,
        0o600,
        0,
        0,
    )?;
    Ok(())
}
fn publish_bundle(bundle: &Path) -> Result<()> {
    let manifest = verify_bundle(bundle)?;
    struct Staged(Vec<(&'static str, PathBuf)>);
    impl Drop for Staged {
        fn drop(&mut self) {
            for (_, path) in &self.0 {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    let mut staged = Staged(Vec::new());
    for name in BINARIES {
        let path = Path::new(layout::HOME)
            .join("bin")
            .join(format!(".bundle-{}-{name}", uuid::Uuid::new_v4()));
        staged.0.push((name, path.clone()));
        let mut source = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(bundle.join(name))?;
        let mut target = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)?;
        std::io::copy(&mut source, &mut target)?;
        target.sync_all()?;
        if hash_file(&path)? != manifest.sha256[name] {
            return Err("Bundle changed during staging".into());
        }
    }
    for (name, path) in &staged.0 {
        std::fs::set_permissions(
            path,
            std::fs::Permissions::from_mode(if *name == "tealdrive-helper" {
                0o4755
            } else {
                0o755
            }),
        )?;
        std::fs::rename(path, Path::new(layout::HOME).join("bin").join(name))?;
    }
    std::fs::File::open(Path::new(layout::HOME).join("bin"))?.sync_all()?;
    Ok(())
}
fn validate_path_links() -> Result<()> {
    trusted_directory(Path::new("/usr/local/bin"))?;
    for name in BINARIES {
        let link = Path::new("/usr/local/bin").join(name);
        match std::fs::symlink_metadata(&link) {
            Ok(metadata)
                if metadata.file_type().is_symlink()
                    && metadata.uid() == 0
                    && std::fs::read_link(&link)?
                        == Path::new(layout::HOME).join("bin").join(name) => {}
            Ok(_) => return Err("Refusing to replace an unrelated PATH executable or link.".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
fn install_path_links() -> Result<()> {
    validate_path_links()?;
    for name in BINARIES {
        let link = Path::new("/usr/local/bin").join(name);
        if link.symlink_metadata().is_err() {
            std::os::unix::fs::symlink(Path::new(layout::HOME).join("bin").join(name), &link)?;
        }
    }
    Ok(())
}
fn install(bundle: &Path, update: bool) -> Result<()> {
    let manifest = verify_bundle(bundle)?;
    stopped()?;
    validate_path_links()?;
    if !update && Path::new(layout::HELPER_CONFIG).exists() {
        return Err("Installation already exists; use update/repair.".into());
    }
    let identity = service_identity()?;
    prepare_layout(&identity)?;
    let base = Path::new(layout::HOME);
    if !update && Path::new(layout::HELPER_CONFIG).exists() {
        return Err(
            "Installation already exists. Use update/repair; existing config/data were preserved."
                .into(),
        );
    }
    let rollback = if update {
        let previous = base
            .join("storage")
            .join(format!("update-backup-{}", uuid::Uuid::new_v4()));
        backup(&previous)?;
        let binaries = previous.join("binaries");
        binary_backup(&binaries)?;
        Some(binaries)
    } else {
        None
    };
    if let Err(error) = publish_bundle(bundle) {
        if let Some(previous) = rollback {
            publish_bundle(&previous)?;
        }
        return Err(error);
    }
    if !Path::new(layout::HELPER_KEY).exists() {
        let mut key = zeroize::Zeroizing::new(vec![0u8; 32]);
        std::fs::File::open("/dev/urandom")?.read_exact(&mut key)?;
        atomic(Path::new(layout::HELPER_KEY), &key, 0o600, 0, 0)?;
    }
    if !Path::new(layout::ACCOUNTS).exists() {
        AccountStore::initialize(Path::new(layout::ACCOUNTS))?;
    }
    if !Path::new(layout::HELPER_CONFIG).exists() {
        // Conservative offline bootstrap; require an explicit origin before public exposure.
        let mut cfg = AppConfig::default();
        cfg.web_accounts.enabled = true;
        cfg.journal.enabled = true;
        cfg.web_accounts.server_uid = Some(identity.uid);
        cfg.roots.clear();
        cfg.home_template = Some(crate::policy::home::default_template());
        cfg.server.bind_addr = "127.0.0.1:13100".into();
        cfg.server.behind_tls_proxy = false;
        cfg.server.require_tls = Some(false);
        cfg.session.secure_cookies = false;
        cfg.security.allowed_origins = vec!["http://127.0.0.1:13100".into()];
        cfg.features.save_text_file = true;
        cfg.features.chmod_file = true;
        cfg.features.extract_archive = true;
        cfg.audit.path = Some(base.join("logs/server/audit.jsonl"));
        cfg.audit.retention_hours = Some(504);
        cfg.sensitive_policy
            .deny_edit_patterns
            .retain(|pattern| pattern != ".env" && pattern != ".env.*");
        cfg.sensitive_policy
            .warn_read_patterns
            .retain(|pattern| pattern != ".env" && pattern != ".env.*");
        save_configs(&cfg)?;
    }
    if !Path::new(layout::SETTINGS).exists() {
        atomic(
            Path::new(layout::SETTINGS),
            b"",
            0o600,
            identity.uid,
            identity.primary_gid,
        )?;
    }
    let unit = include_str!("../docs/deployment/tealdrive.service");
    if Path::new("/etc/systemd/system/tealdrive.service").exists() {
        let original = std::fs::read("/etc/systemd/system/tealdrive.service")?;
        atomic(
            &base.join("storage/previous-tealdrive.service"),
            &original,
            0o600,
            0,
            0,
        )?;
    }
    atomic(
        Path::new("/etc/systemd/system/tealdrive.service"),
        unit.as_bytes(),
        0o644,
        0,
        0,
    )?;
    if !Path::new("/etc/pam.d/tealdrive").exists() {
        atomic(
            Path::new("/etc/pam.d/tealdrive"),
            include_bytes!("../docs/deployment/pam.tealdrive"),
            0o644,
            0,
            0,
        )?;
    }
    for (name, body) in [
        (
            "tealdrive-gc.service",
            include_bytes!("../docs/deployment/tealdrive-gc.service").as_slice(),
        ),
        (
            "tealdrive-gc.timer",
            include_bytes!("../docs/deployment/tealdrive-gc.timer").as_slice(),
        ),
    ] {
        atomic(
            &Path::new("/etc/systemd/system").join(name),
            body,
            0o644,
            0,
            0,
        )?;
    }
    install_path_links()?;
    command("/usr/bin/systemctl", &["daemon-reload"])?;
    println!("Lifecycle timer is installed but not enabled automatically; enable after diagnose and recovery checks.\nInstalled {}. Server not auto-started. Default bind is loopback HTTP only. Configure TLS/proxy+origin before public exposure. Initialize virtual storage, then bootstrap admin <delegated-budget> <own-quota> for a random-password virtual administrator; legacy Linux approve/admin remains explicit. Configuration and user data were preserved.",manifest.version);
    Ok(())
}
fn repair() -> Result<()> {
    validate_path_links()?;
    let identity = service_identity()?;
    prepare_layout(&identity)?;
    for name in BINARIES {
        let path = Path::new(layout::HOME).join("bin").join(name);
        let metadata = std::fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.uid() != 0 {
            return Err(
                "Repair refuses an untrusted binary; reinstall from a verified bundle.".into(),
            );
        }
        std::fs::set_permissions(
            path,
            std::fs::Permissions::from_mode(if name == "tealdrive-helper" {
                0o4755
            } else {
                0o755
            }),
        )?;
    }
    config()?.validate()?;
    store()?;
    install_path_links()?;
    println!("Ownership, modes and configuration verified. User contents were not changed.");
    Ok(())
}
fn diagnose() -> Result<()> {
    let cfg = config()?;
    cfg.validate()?;
    let identity = crate::helper::security::account("tealdrive")?;
    if cfg.web_accounts.server_uid != Some(identity.uid) {
        return Err("Service UID mismatch".into());
    }
    let db = store()?;
    let mut problems = Vec::new();
    for account in db
        .all_accounts(&Authority::Root, AccountKind::Linux)?
        .into_iter()
        .chain(db.all_accounts(&Authority::Root, AccountKind::Virtual)?)
    {
        if db.is_purged(&account.id)? {
            continue;
        }
        if crate::helper::account_gateway::validate_account(&cfg, &account).is_err() {
            problems.push(serde_json::json!({"account_id":account.id,"username":account.username,"problem":"NSS/policy identity mismatch"}));
        }
    }
    let mount = Command::new("/usr/bin/findmnt")
        .args([
            "--noheadings",
            "--output",
            "OPTIONS",
            "--target",
            "/home/tealdrive/bin/tealdrive-helper",
        ])
        .output()?;
    let options = String::from_utf8_lossy(&mount.stdout);
    if options
        .split(',')
        .any(|option| matches!(option.trim(), "nosuid" | "noexec"))
    {
        return Err("Helper mount is nosuid/noexec; refusing a broader fallback.".into());
    }
    print(
        &serde_json::json!({"config_valid":true,"service_uid":identity.uid,"database_ready":true,"server_running":running()?,"identity_problems":problems,"kernel":String::from_utf8_lossy(&Command::new("/usr/bin/uname").arg("-r").output()?.stdout).trim()}),
    )
}
fn backup(directory: &Path) -> Result<()> {
    if directory.exists() {
        return Err("Backup destination already exists".into());
    }
    trusted_directory(directory.parent().ok_or("Invalid backup path")?)?;
    std::fs::create_dir(directory)?;
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))?;
    store()?.backup(&Authority::Root, &directory.join("accounts.db"))?;
    for (source, name, secret) in [
        (layout::HELPER_CONFIG, "helper.toml", false),
        (layout::SERVER_CONFIG, "server.toml", false),
        (layout::HELPER_KEY, "helper.key", true),
    ] {
        let bytes = zeroize::Zeroizing::new(crate::helper::security::trusted_read(
            Path::new(source),
            secret,
        )?);
        atomic(&directory.join(name), &bytes, 0o600, 0, 0)?;
    }
    let mut hashes = std::collections::BTreeMap::new();
    for name in ["accounts.db", "helper.toml", "server.toml", "helper.key"] {
        hashes.insert(name.into(), hash_file(&directory.join(name))?);
    }
    atomic(
        &directory.join("manifest.json"),
        &serde_json::to_vec(&Manifest {
            schema_version: 1,
            version: "metadata-backup-v1".into(),
            sha256: hashes,
        })?,
        0o600,
        0,
        0,
    )?;
    println!("Root-private metadata/key backup complete. User file contents remain on disk and need a filesystem backup.");
    Ok(())
}
fn restore(directory: &Path) -> Result<()> {
    stopped()?;
    trusted_directory(directory)?;
    let manifest: Manifest = serde_json::from_slice(&crate::helper::security::trusted_read(
        &directory.join("manifest.json"),
        true,
    )?)?;
    if manifest.schema_version != 1 || manifest.version != "metadata-backup-v1" {
        return Err("Unsupported backup".into());
    }
    for name in ["accounts.db", "helper.toml", "server.toml", "helper.key"] {
        if manifest.sha256.get(name) != Some(&hash_file(&directory.join(name))?) {
            return Err("Backup checksum mismatch".into());
        }
    }
    let cfg = AppConfig::from_file(directory.join("helper.toml"))?;
    let identity = crate::helper::security::account("tealdrive")?;
    if cfg.web_accounts.server_uid != Some(identity.uid) {
        return Err("Backup service UID differs; explicit identity migration required".into());
    }
    AccountStore::open(&directory.join("accounts.db"))?;
    let previous = Path::new(layout::HOME)
        .join("storage")
        .join(format!("before-restore-{}", uuid::Uuid::new_v4()));
    backup(&previous)?;
    for suffix in ["-wal", "-shm", "-journal"] {
        let path = PathBuf::from(format!("{}{suffix}", layout::ACCOUNTS));
        if path.exists() {
            let metadata = std::fs::symlink_metadata(&path)?;
            if !metadata.is_file() || metadata.uid() != 0 {
                return Err("Untrusted database sidecar".into());
            }
            std::fs::remove_file(path)?;
        }
    }
    let bytes = zeroize::Zeroizing::new(std::fs::read(directory.join("accounts.db"))?);
    atomic(Path::new(layout::ACCOUNTS), &bytes, 0o600, 0, 0)?;
    for (name, target, mode) in [
        ("helper.toml", layout::HELPER_CONFIG, 0o600),
        ("server.toml", layout::SERVER_CONFIG, 0o644),
        ("helper.key", layout::HELPER_KEY, 0o600),
    ] {
        let bytes = zeroize::Zeroizing::new(std::fs::read(directory.join(name))?);
        atomic(Path::new(target), &bytes, mode, 0, 0)?;
    }
    store()?.invalidate_restored_sessions(&Authority::Root)?;
    println!("Metadata restored offline. Run diagnose before starting; user contents and OS identities were not overwritten.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn size_boundaries_reject_ambiguous_and_overflowing_input() {
        assert_eq!(bytes("60GiB").unwrap(), 60 * (1 << 30));
        assert_eq!(bytes("0").unwrap(), 0);
        for value in ["-1", "1.5GiB", "10GB", "18446744073709551615TiB", ""] {
            assert!(bytes(value).is_err(), "{value}");
        }
    }
    #[test]
    fn arbitrary_service_and_password_arguments_are_rejected() {
        assert!(control(vec!["stop".into(), "caddy.service".into()]).is_err());
        assert!(run(vec!["init".into(), "password=secret".into()]).is_err());
        assert!(run(vec![
            "account".into(),
            "password".into(),
            "id".into(),
            "secret".into()
        ])
        .is_err());
    }
    #[test]
    fn manifest_rejects_unknown_fields_and_missing_checksum_map() {
        assert!(serde_json::from_str::<Manifest>(
            r#"{"schema_version":1,"version":"1","sha256":{},"run":"arbitrary"}"#
        )
        .is_err());
        assert!(serde_json::from_str::<Manifest>(r#"{"schema_version":1,"version":"1"}"#).is_err());
    }
}
