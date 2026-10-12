use std::os::unix::fs::{MetadataExt, PermissionsExt};
use tealdrive::accounts::store::{
    AccountKind, AccountStore, Authority, NewAccount, Role, StoreError,
};
#[test]
#[ignore = "requires root for trusted SQLite/WAL ownership and UID isolation"]
fn protected_sqlite_survives_restart_and_enforces_concurrent_budgets() {
    if let Ok(path) = std::env::var("TEALDRIVE_ACCOUNT_PRIVACY_CHILD") {
        assert_eq!(unsafe { libc::setgid(65534) }, 0);
        assert_eq!(unsafe { libc::setuid(65534) }, 0);
        assert!(std::fs::read(&path).is_err());
        assert!(matches!(
            AccountStore::open(std::path::Path::new(&path)),
            Err(StoreError::UntrustedStore)
        ));
        return;
    }
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let dir = std::env::current_dir()
        .unwrap()
        .join(format!(".accounts-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = dir.join("accounts.db");
    let mut store = AccountStore::initialize(&path).unwrap();
    let admin = store
        .create(
            &Authority::Root,
            NewAccount {
                username: "admin".into(),
                uid: 1000,
                gid: 1000,
                home: "/home/admin".into(),
                kind: AccountKind::Linux,
                role: Role::Admin,
                owner_admin_id: None,
                quota_bytes: None,
            },
        )
        .unwrap();
    store.set_pool(&Authority::Root, 1000).unwrap();
    store
        .set_admin_budget(&Authority::Root, &admin.id, 600)
        .unwrap();
    store
        .set_password(&Authority::Root, &admin.id, "separate-private-web-password")
        .unwrap();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let m = entry.unwrap().metadata().unwrap();
        assert_eq!(m.uid(), 0);
        assert_eq!(m.mode() & 0o077, 0);
    }
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "protected_sqlite_survives_restart_and_enforces_concurrent_budgets",
            "--ignored",
        ])
        .env("TEALDRIVE_ACCOUNT_PRIVACY_CHILD", &path)
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "{}",
        String::from_utf8_lossy(&child.stderr)
    );
    let backup_path = dir.join("backup.db");
    store.backup(&Authority::Root, &backup_path).unwrap();
    let backup = AccountStore::open(&backup_path).unwrap();
    assert!(backup
        .verify_password("admin", "separate-private-web-password")
        .is_ok());
    assert_eq!(
        std::fs::metadata(&backup_path).unwrap().mode() & 0o777,
        0o600
    );
    assert!(store.backup(&Authority::Root, &backup_path).is_err());
    drop(backup);
    let snapshot = store.get(&admin.id).unwrap();
    drop(store);
    let reopened = AccountStore::open(&path).unwrap();
    assert_eq!(reopened.get(&admin.id).unwrap(), snapshot);
    assert!(reopened
        .verify_password("admin", "separate-private-web-password")
        .is_ok());
    drop(reopened);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let workers: Vec<_> = (0..2)
        .map(|index| {
            let path = path.clone();
            let id = admin.id.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut store = AccountStore::open(&path).unwrap();
                barrier.wait();
                store.create(
                    &Authority::Root,
                    NewAccount {
                        username: format!("child-{index}"),
                        uid: 200000 + index,
                        gid: 200000 + index,
                        home: format!("/home/virtual-{index}").into(),
                        kind: AccountKind::Virtual,
                        role: Role::User,
                        owner_admin_id: Some(id),
                        quota_bytes: Some(400),
                    },
                )
            })
        })
        .collect();
    let results: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(StoreError::QuotaExceeded)))
            .count(),
        1
    );
    assert!(AccountStore::initialize(&path).is_err());
    let alias = dir.join("alias.db");
    std::os::unix::fs::symlink(&path, &alias).unwrap();
    assert!(AccountStore::open(&alias).is_err());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(matches!(
        AccountStore::open(&path),
        Err(StoreError::UntrustedStore)
    ));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let raw = rusqlite::Connection::open(&path).unwrap();
    raw.pragma_update(None, "user_version", 99).unwrap();
    drop(raw);
    assert!(matches!(
        AccountStore::open(&path),
        Err(StoreError::UnsupportedSchema)
    ));
    std::fs::remove_dir_all(dir).unwrap();
}
