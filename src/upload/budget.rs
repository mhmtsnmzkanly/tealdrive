//! Filesystem-wide reservations for server staging, including crash leftovers.
//! Advisory locks coordinate cooperating server processes; this is not an OS UID quota.
use crate::errors::TealDriveError;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
};
fn fail() -> TealDriveError {
    TealDriveError::DiskFull
}
fn private_file(path: &Path, create: bool) -> Result<File, TealDriveError> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(create)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| fail())?;
    let m = file.metadata().map_err(|_| fail())?;
    if !m.is_file()
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
        || m.nlink() != 1
    {
        return Err(fail());
    }
    Ok(file)
}
fn lock(file: &File, nonblock: bool) -> Result<bool, TealDriveError> {
    if unsafe {
        libc::flock(
            file.as_raw_fd(),
            libc::LOCK_EX | if nonblock { libc::LOCK_NB } else { 0 },
        )
    } == 0
    {
        return Ok(true);
    }
    let error = std::io::Error::last_os_error();
    if nonblock && error.raw_os_error() == Some(libc::EWOULDBLOCK) {
        Ok(false)
    } else {
        Err(fail())
    }
}
fn directory(parent: &Path) -> Result<PathBuf, TealDriveError> {
    let path = parent.join(format!(".tdrv-budget-{}", unsafe { libc::geteuid() }));
    match fs::DirBuilder::new().mode(0o700).create(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(fail()),
    }
    let m = fs::symlink_metadata(&path).map_err(|_| fail())?;
    if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 {
        return Err(fail());
    }
    Ok(path)
}
#[derive(Debug)]
pub(crate) struct DiskReservation {
    path: PathBuf,
    _file: File,
}
impl Drop for DiskReservation {
    fn drop(&mut self) {
        // Never unlink a lock file while another process could be updating the ledger.
        if let Some(parent) = self.path.parent() {
            if let Ok(global) = private_file(&parent.join("lock"), false) {
                if lock(&global, false).unwrap_or(false) {
                    let _ = fs::remove_file(&self.path);
                }
            }
        }
    }
}
fn stage_bytes(parent: &Path, id: &str) -> Result<Option<u64>, TealDriveError> {
    let path = parent.join(format!(".tdrv-upload-{id}.tmp"));
    match fs::symlink_metadata(path) {
        Ok(m) => {
            if !m.is_file() || m.uid() != unsafe { libc::geteuid() } || m.nlink() != 1 {
                return Err(fail());
            }
            Ok(Some(m.len()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(fail()),
    }
}
pub(crate) fn reserve(
    stage: &Path,
    bytes: u64,
    maximum: u64,
    max_transfers: u32,
) -> Result<DiskReservation, TealDriveError> {
    let parent = stage.parent().ok_or_else(fail)?;
    let dir = directory(parent)?;
    let global = private_file(&dir.join("lock"), true)?;
    lock(&global, false)?;
    let mut total = 0u64;
    let mut count = 0u32;
    let mut accounted = std::collections::HashSet::new();
    for (index, entry) in fs::read_dir(&dir).map_err(|_| fail())?.enumerate() {
        if index > 8192 {
            return Err(fail());
        }
        let entry = entry.map_err(|_| fail())?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            return Err(fail());
        };
        if name == "lock" {
            continue;
        }
        let Some(id) = name.strip_suffix(".lease") else {
            return Err(fail());
        };
        if uuid::Uuid::parse_str(id).is_err() {
            return Err(fail());
        }
        let mut file = private_file(&entry.path(), false)?;
        let abandoned = lock(&file, true)?;
        let actual = stage_bytes(parent, id)?;
        if abandoned && actual.is_none() {
            fs::remove_file(entry.path()).map_err(|_| fail())?;
            continue;
        }
        let charged = if abandoned {
            actual.unwrap_or(0)
        } else {
            let mut raw = String::new();
            std::io::Read::by_ref(&mut file)
                .take(32)
                .read_to_string(&mut raw)
                .map_err(|_| fail())?;
            if file.metadata().map_err(|_| fail())?.len() > 20 {
                return Err(fail());
            }
            raw.parse::<u64>()
                .map_err(|_| fail())?
                .max(actual.unwrap_or(0))
        };
        total = total.checked_add(charged).ok_or_else(fail)?;
        count = count.checked_add(1).ok_or_else(fail)?;
        accounted.insert(id.to_owned());
    }
    // Earlier releases and crashes can leave staging files without a lease.
    for (index, entry) in fs::read_dir(parent).map_err(|_| fail())?.enumerate() {
        if index > 65536 {
            return Err(fail());
        }
        let entry = entry.map_err(|_| fail())?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(id) = name
            .strip_prefix(".tdrv-upload-")
            .and_then(|v| v.strip_suffix(".tmp"))
        else {
            continue;
        };
        if uuid::Uuid::parse_str(id).is_err() || accounted.contains(id) {
            continue;
        }
        let m = fs::symlink_metadata(entry.path()).map_err(|_| fail())?;
        if m.uid() != unsafe { libc::geteuid() } {
            continue;
        }
        let actual = stage_bytes(parent, id)?.ok_or_else(fail)?;
        total = total.checked_add(actual).ok_or_else(fail)?;
        count = count.checked_add(1).ok_or_else(fail)?;
    }
    if total.checked_add(bytes).is_none_or(|v| v > maximum) || count >= max_transfers {
        return Err(fail());
    }
    let name = stage
        .file_name()
        .and_then(|v| v.to_str())
        .and_then(|v| v.strip_prefix(".tdrv-upload-"))
        .and_then(|v| v.strip_suffix(".tmp"))
        .ok_or_else(fail)?;
    if uuid::Uuid::parse_str(name).is_err() {
        return Err(fail());
    }
    let path = dir.join(format!("{name}.lease"));
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&path)
        .map_err(|_| fail())?;
    if !lock(&file, true)? {
        let _ = fs::remove_file(&path);
        return Err(fail());
    }
    if file
        .write_all(bytes.to_string().as_bytes())
        .and_then(|_| file.sync_all())
        .is_err()
    {
        let _ = fs::remove_file(&path);
        return Err(fail());
    }
    File::open(&dir)
        .and_then(|f| f.sync_all())
        .map_err(|_| fail())?;
    Ok(DiskReservation { path, _file: file })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concurrent_process_lease_and_crash_leftover_are_charged() {
        if let Ok(path) = std::env::var("TD_TEST_BUDGET_CHILD") {
            let dir = Path::new(&path);
            let stage = dir.join(".tdrv-upload-00000000-0000-4000-8000-000000000001.tmp");
            let _held = reserve(&stage, 80, 100, 10).unwrap();
            fs::write(&stage, [1u8; 10]).unwrap();
            fs::write(dir.join("ready"), b"ok").unwrap();
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        let dir = std::env::temp_dir().join(format!("td-budget-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(dir.clone());
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "upload::budget::tests::concurrent_process_lease_and_crash_leftover_are_charged",
                "--test-threads=1",
            ])
            .env("TD_TEST_BUDGET_CHILD", &dir)
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        for _ in 0..200 {
            if dir.join("ready").exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let second = dir.join(format!(".tdrv-upload-{}.tmp", uuid::Uuid::new_v4()));
        let active_rejected = reserve(&second, 30, 100, 10).is_err();
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(dir.join("ready").exists());
        assert!(active_rejected);
        assert!(reserve(&second, 91, 100, 10).is_err());
        let held = reserve(&second, 90, 100, 10).unwrap();
        drop(held);
        assert!(reserve(&second, 1, 100, 1).is_err());
        fs::remove_file(dir.join(".tdrv-upload-00000000-0000-4000-8000-000000000001.tmp")).unwrap();
        assert!(reserve(&second, 100, 100, 1).is_ok());
    }
    #[test]
    fn orphan_files_count_and_symlink_ledger_fails_closed() {
        let dir = std::env::temp_dir().join(format!("td-budget-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let old = dir.join(format!(".tdrv-upload-{}.tmp", uuid::Uuid::new_v4()));
        fs::write(&old, [1u8; 11]).unwrap();
        let next = dir.join(format!(".tdrv-upload-{}.tmp", uuid::Uuid::new_v4()));
        assert!(reserve(&next, 90, 100, 10).is_err());
        assert!(reserve(&next, 89, 100, 10).is_ok());
        let ledger = directory(&dir).unwrap();
        fs::remove_dir_all(&ledger).unwrap();
        std::os::unix::fs::symlink("/tmp", &ledger).unwrap();
        assert!(reserve(&next, 1, 100, 10).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
