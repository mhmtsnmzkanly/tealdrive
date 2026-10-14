//! Offline Linux-panel-admin to virtual-home migration. Source data is retained.
use super::*;
use crate::config::AllowedRoot;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    ffi::CString,
    fs::File,
    os::unix::{ffi::OsStrExt, fs::DirBuilderExt},
};
const STATE: &str = "/home/tealdrive/auth/admin-migration.json";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Migration {
    version: u8,
    source: String,
    source_generation: u64,
    name: String,
    budget: u64,
    own: u64,
    request: String,
    target: Option<String>,
    published: bool,
    completed: bool,
}
#[derive(Debug, PartialEq, Eq)]
struct Manifest {
    digest: String,
    bytes: u64,
    entries: u64,
}
fn quiet(uid: u32) -> Result<()> {
    for entry in std::fs::read_dir("/proc")? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().parse::<u32>().is_err() {
            continue;
        }
        let status = match std::fs::read_to_string(entry.path().join("status")) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        if status
            .lines()
            .find_map(|l| l.strip_prefix("Uid:"))
            .is_some_and(|s| s.split_whitespace().any(|v| v.parse::<u32>() == Ok(uid)))
        {
            return Err(
                "Stop processes owned by the source/target account before migration".into(),
            );
        }
    }
    Ok(())
}
fn owned(file: &File, uid: u32, dev: u64) -> Result<std::fs::Metadata> {
    let m = file.metadata()?;
    if m.uid() != uid || m.dev() != dev || (!m.is_dir() && (!m.is_file() || m.nlink() != 1)) {
        return Err(
            "Migration refuses foreign ownership, mount crossings, hard links and special files"
                .into(),
        );
    }
    Ok(m)
}
fn apply(file: &File, uid: u32, gid: u32, mode: u32) -> Result<()> {
    if unsafe { libc::fchown(file.as_raw_fd(), uid, gid) } != 0
        || unsafe { libc::fchmod(file.as_raw_fd(), mode & 0o777) } != 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    file.sync_all()?;
    Ok(())
}
struct Walk {
    uid: u32,
    gid: u32,
    target_uid: u32,
    dev: u64,
    max_bytes: u64,
    max_entries: u64,
    bytes: u64,
    entries: u64,
    digest: Sha256,
}
fn tree(source: &File, target: Option<&Path>, walk: &mut Walk, depth: usize) -> Result<()> {
    let directory_before = owned(source, walk.uid, walk.dev)?;
    if depth > 64 {
        return Err("Migration directory nesting exceeds64".into());
    }
    let mut names = std::fs::read_dir(format!("/proc/self/fd/{}", source.as_raw_fd()))?
        .map(|e| e.map(|e| e.file_name()))
        .collect::<std::io::Result<Vec<_>>>()?;
    names.sort();
    for name in names {
        let name = name.to_str().ok_or("Migration requires UTF8 file names")?;
        crate::fs::filename::SafeFileName::parse(name)?;
        let handle = crate::helper::sandbox::open_at(source.as_raw_fd(), name, libc::O_PATH, 0)?;
        let before = owned(&handle, walk.uid, walk.dev)?;
        // Reopen only an already-validated regular file/directory, by its stable
        // descriptor. Never open a user-supplied device/FIFO before checking type.
        let file = File::open(format!("/proc/self/fd/{}", handle.as_raw_fd()))?;
        walk.entries = walk.entries.checked_add(1).ok_or("Entry overflow")?;
        if walk.entries > walk.max_entries {
            return Err("Migration exceeds inode budget".into());
        }
        walk.digest.update((name.len() as u64).to_be_bytes());
        walk.digest.update(name.as_bytes());
        walk.digest.update((before.mode() & 0o777).to_be_bytes());
        let destination = target.map(|base| base.join(name));
        if before.is_dir() {
            walk.digest.update(b"D");
            if let Some(path) = &destination {
                std::fs::DirBuilder::new().mode(0o700).create(path)?;
            }
            tree(&file, destination.as_deref(), walk, depth + 1)?;
            if let Some(path) = &destination {
                let directory = File::open(path)?;
                apply(&directory, walk.target_uid, walk.gid, before.mode())?;
            }
            walk.digest.update(b"E");
        } else {
            walk.bytes = walk
                .bytes
                .checked_add(before.len())
                .ok_or("Size overflow")?;
            if walk.bytes > walk.max_bytes {
                return Err("Source contents exceed own-file quota".into());
            }
            walk.digest.update(b"F");
            walk.digest.update(before.len().to_be_bytes());
            let mut output = destination
                .as_ref()
                .map(|p| {
                    std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .mode(0o600)
                        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                        .open(p)
                })
                .transpose()?;
            let mut input = file.take(before.len().checked_add(1).ok_or("Size overflow")?);
            let mut buffer = [0; 65536];
            let mut copied = 0u64;
            loop {
                let n = input.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                copied += n as u64;
                if copied > before.len() {
                    return Err("Source changed during migration".into());
                }
                walk.digest.update(&buffer[..n]);
                if let Some(out) = &mut output {
                    out.write_all(&buffer[..n])?;
                }
            }
            if copied != before.len() {
                return Err("Source changed during migration".into());
            }
            let file = input.into_inner();
            let after = owned(&file, walk.uid, walk.dev)?;
            if (
                before.ino(),
                before.len(),
                before.mtime(),
                before.mtime_nsec(),
                before.ctime(),
                before.ctime_nsec(),
            ) != (
                after.ino(),
                after.len(),
                after.mtime(),
                after.mtime_nsec(),
                after.ctime(),
                after.ctime_nsec(),
            ) {
                return Err("Source changed during migration".into());
            }
            if let Some(out) = output {
                apply(&out, walk.target_uid, walk.gid, before.mode())?;
            }
        }
    }
    let directory_after = owned(source, walk.uid, walk.dev)?;
    if (
        directory_before.mtime(),
        directory_before.mtime_nsec(),
        directory_before.ctime(),
        directory_before.ctime_nsec(),
    ) != (
        directory_after.mtime(),
        directory_after.mtime_nsec(),
        directory_after.ctime(),
        directory_after.ctime_nsec(),
    ) {
        return Err("Source directory changed during migration".into());
    }
    Ok(())
}
fn manifest(
    path: &Path,
    uid: u32,
    gid: u32,
    max_bytes: u64,
    max_entries: u64,
    copy: Option<(&Path, u32)>,
) -> Result<Manifest> {
    let root = AllowedRoot {
        base_path: path.into(),
        ..AllowedRoot::default()
    };
    let fd = crate::helper::sandbox::open(
        &root,
        &crate::config::RelativePath::new(""),
        libc::O_RDONLY | libc::O_DIRECTORY,
        0,
    )?;
    let dev = fd.metadata()?.dev();
    owned(&fd, uid, dev)?;
    let mut walk = Walk {
        uid,
        gid,
        target_uid: copy.map_or(uid, |(_, uid)| uid),
        dev,
        max_bytes,
        max_entries,
        bytes: 0,
        entries: 0,
        digest: Sha256::new(),
    };
    tree(&fd, copy.map(|(path, _)| path), &mut walk, 0)?;
    Ok(Manifest {
        digest: format!("{:x}", walk.digest.finalize()),
        bytes: walk.bytes,
        entries: walk.entries,
    })
}
fn save(state: &Migration) -> Result<()> {
    atomic(
        Path::new(STATE),
        &serde_json::to_vec_pretty(state)?,
        0o600,
        0,
        0,
    )
}
fn load() -> Result<Migration> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(STATE)?;
    let m = file.metadata()?;
    if !m.is_file() || m.uid() != 0 || m.mode() & 0o077 != 0 || m.nlink() != 1 || m.len() > 4096 {
        return Err("Untrusted migration journal".into());
    }
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(4097)
        .read_to_end(&mut bytes)?;
    let state: Migration = serde_json::from_slice(&bytes)?;
    if state.version != 1 || uuid::Uuid::parse_str(&state.request).is_err() {
        return Err("Invalid migration journal".into());
    }
    Ok(state)
}
pub(super) fn migrate(source_id: &str, name: &str, budget: u64, own: u64) -> Result<()> {
    stopped()?;
    let mut cfg = config()?;
    let mut db = store()?;
    let source = db.get(source_id)?;
    if source.kind != AccountKind::Linux
        || source.role != Role::Admin
        || source.status != AccountStatus::Active
        || !source.password_enrolled
        || !cfg.account_policy.is_admin(&source.username)
        || own == 0
        || own > budget
    {
        return Err(
            "An existing active Linux panel administrator and valid virtual budgets are required"
                .into(),
        );
    }
    crate::helper::account_gateway::validate_account(&cfg, &source)
        .map_err(|_| "Source NSS identity mismatch")?;
    quiet(source.uid)?;
    let mut state = if Path::new(STATE).symlink_metadata().is_ok() {
        let state = load()?;
        if state.source != source_id
            || state.name != name
            || state.budget != budget
            || state.own != own
        {
            return Err(
                "Another migration is recorded; inspect it before starting a different target"
                    .into(),
            );
        }
        state
    } else {
        let state = Migration {
            version: 1,
            source: source_id.into(),
            source_generation: source.generation,
            name: name.into(),
            budget,
            own,
            request: uuid::Uuid::new_v4().to_string(),
            target: None,
            published: false,
            completed: false,
        };
        save(&state)?;
        state
    };
    if state.completed {
        return print(
            &serde_json::json!({"completed":true,"targetAccountId":state.target,"sourceRetained":true}),
        );
    }
    if source.generation != state.source_generation {
        return Err("Source account generation changed; no credentials overwritten".into());
    }
    let password =
        zeroize::Zeroizing::new(format!("{}{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4()));
    let mut target = crate::accounts::virtuals::create_admin(
        &mut db,
        &cfg,
        name,
        budget,
        &password,
        &state.request,
    )?;
    if state.target.as_ref().is_some_and(|id| id != &target.id) {
        return Err("Migration account mismatch".into());
    }
    state.target = Some(target.id.clone());
    save(&state)?;
    quiet(target.uid)?;
    crate::accounts::virtuals::set_own_quota(&mut db, &cfg, &target.id, own)?;
    target = db.get(&target.id)?;
    let _global = crate::accounts::virtuals::lock()?;
    crate::accounts::virtuals::root(&cfg, &target)?;
    let expected = manifest(
        &source.home,
        source.uid,
        target.gid,
        own,
        cfg.virtual_accounts.max_inodes.saturating_sub(1),
        None,
    )?;
    let parent = target.home.parent().ok_or("Target parent")?;
    trusted_directory(parent)?;
    let staging = parent.join(format!(".migration-{}", state.request));
    if !state.published {
        let current = manifest(
            &target.home,
            target.uid,
            target.gid,
            own,
            cfg.virtual_accounts.max_inodes.saturating_sub(1),
            None,
        )?;
        if current.entries != 0 && current != expected {
            return Err(
                "Target home contains different data; migration never overwrites it".into(),
            );
        }
        if current != expected {
            if staging.symlink_metadata().is_ok() {
                let m = std::fs::symlink_metadata(&staging)?;
                if !m.is_dir() || ![0, target.uid].contains(&m.uid()) {
                    return Err("Untrusted migration staging".into());
                }
                std::fs::remove_dir_all(&staging)?;
            }
            std::fs::DirBuilder::new().mode(0o700).create(&staging)?;
            let copied = manifest(
                &source.home,
                source.uid,
                target.gid,
                own,
                cfg.virtual_accounts.max_inodes.saturating_sub(1),
                Some((&staging, target.uid)),
            )?;
            if copied != expected {
                return Err("Source changed since preflight".into());
            }
            apply(&File::open(&staging)?, target.uid, target.gid, 0o700)?;
            if manifest(
                &staging,
                target.uid,
                target.gid,
                own,
                cfg.virtual_accounts.max_inodes.saturating_sub(1),
                None,
            )? != expected
            {
                return Err("Copied data verification failed".into());
            }
            if std::fs::read_dir(&target.home)?.next().is_some() {
                return Err("Target home changed; no overwrite performed".into());
            }
            let a = CString::new(staging.as_os_str().as_bytes())?;
            let b = CString::new(target.home.as_os_str().as_bytes())?;
            if unsafe {
                libc::syscall(
                    libc::SYS_renameat2,
                    libc::AT_FDCWD,
                    a.as_ptr(),
                    libc::AT_FDCWD,
                    b.as_ptr(),
                    libc::RENAME_EXCHANGE,
                )
            } != 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
            File::open(parent)?.sync_all()?;
        }
        state.published = true;
        save(&state)?;
    }
    if manifest(
        &target.home,
        target.uid,
        target.gid,
        own,
        cfg.virtual_accounts.max_inodes.saturating_sub(1),
        None,
    )? != expected
    {
        return Err("Published virtual home verification failed".into());
    }
    if staging.symlink_metadata().is_ok() {
        std::fs::remove_dir_all(staging)?;
    }
    db.copy_admin_credentials(
        &Authority::Root,
        &source.id,
        source.generation,
        &target.id,
        target.generation,
    )?;
    let target = db.set_panel_scope(&Authority::Root, &target.id, true)?;
    if !cfg.account_policy.admin_users.contains(&target.username) {
        cfg.account_policy.admin_users.push(target.username.clone());
    }
    save_configs(&cfg)?;
    state.completed = true;
    save(&state)?;
    print(
        &serde_json::json!({"completed":true,"account":target,"sourceRetained":true,"sourceWebLoginStillEnabled":source.login_enabled,"files":expected.entries,"bytes":expected.bytes,"contentDigest":expected.digest,"next":"Verify new panel access, then account deny SOURCE_ID; source files are retained"}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn descriptor_copy_preserves_bytes_and_refuses_links_without_overwrite() {
        let base = std::env::temp_dir().join(format!("td-migrate-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&base).unwrap();
        let src = base.join("src");
        let dst = base.join("dst");
        std::fs::create_dir(&src).unwrap();
        std::fs::create_dir(&dst).unwrap();
        std::fs::create_dir(src.join("test")).unwrap();
        std::fs::write(src.join("test/hello gpt.md"), b"preserve me").unwrap();
        let uid = unsafe { libc::geteuid() };
        let gid = unsafe { libc::getegid() };
        let before = manifest(&src, uid, gid, 1000, 100, None).unwrap();
        let copied = manifest(&src, uid, gid, 1000, 100, Some((&dst, uid))).unwrap();
        assert_eq!(before, copied);
        assert_eq!(manifest(&dst, uid, gid, 1000, 100, None).unwrap(), before);
        assert!(manifest(&src, uid, gid, 1, 100, None).is_err());
        assert!(manifest(&src, uid, gid, 1000, 1, None).is_err());
        assert!(manifest(&src, uid, gid, 1000, 100, Some((&dst, uid))).is_err());
        std::os::unix::fs::symlink("/etc/shadow", src.join("link")).unwrap();
        assert!(manifest(&src, uid, gid, 1000, 100, None).is_err());
        std::fs::remove_file(src.join("link")).unwrap();
        std::fs::hard_link(src.join("test/hello gpt.md"), src.join("hardlink")).unwrap();
        assert!(manifest(&src, uid, gid, 1000, 100, None).is_err());
        std::fs::remove_file(src.join("hardlink")).unwrap();
        let fifo = CString::new(src.join("fifo").as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert!(manifest(&src, uid, gid, 1000, 100, None).is_err());
        std::fs::remove_dir_all(base).unwrap();
    }
}
