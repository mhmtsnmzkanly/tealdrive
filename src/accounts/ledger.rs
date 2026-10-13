//! User-charged virtual operation records outside user homes, under Root-only directories.
//! The kernel bills the inode to the virtual UID; no writable ledger FD reaches a user worker.
use super::{
    journal::{JournalConfig, OperationRecord},
    store::{root_parent, Account, AccountKind, StoreError},
};
use crate::{config::AllowedRoot, helper::native_quota};
use std::{
    fs::{File, OpenOptions},
    io::{Seek, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};
fn directory(path: &Path) -> Result<(), StoreError> {
    root_parent(path)?;
    match std::fs::symlink_metadata(path) {
        Ok(m) if !m.is_dir() || m.uid() != 0 || m.mode() & 0o777 != 0o700 => {
            return Err(StoreError::UntrustedStore)
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(path)?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}
fn parent(account: &Account) -> Result<PathBuf, StoreError> {
    uuid::Uuid::parse_str(&account.id).map_err(|_| StoreError::InvalidInput)?;
    Ok(Path::new(crate::layout::VIRTUAL_HOMES)
        .join(".tealdrive-audit")
        .join(&account.id))
}
pub fn used(account: &Account) -> Result<u64, StoreError> {
    if account.kind != AccountKind::Virtual {
        return Ok(0);
    }
    let path = parent(account)?;
    if !path.try_exists()? {
        return Ok(0);
    }
    root_parent(&path.join("entry"))?;
    let mut bytes = 0u64;
    let mut count = 0;
    for entry in std::fs::read_dir(path)? {
        count += 1;
        if count > 100000 {
            return Err(StoreError::InvalidInput);
        }
        let entry = entry?;
        let metadata = std::fs::symlink_metadata(entry.path())?;
        if !metadata.is_file()
            || metadata.uid() != account.uid
            || metadata.nlink() != 1
            || metadata.mode() & 0o777 != 0o400
        {
            return Err(StoreError::UntrustedStore);
        }
        bytes = bytes
            .checked_add(metadata.len())
            .ok_or(StoreError::InvalidInput)?;
    }
    Ok(bytes)
}
pub struct Charge {
    file: File,
    reserved: u64,
}
impl Charge {
    pub fn reserved_bytes(&self) -> u64 {
        self.reserved
    }
    pub fn write(&mut self, record: &OperationRecord) -> Result<(), StoreError> {
        let bytes = serde_json::to_vec(record).map_err(|_| StoreError::InvalidInput)?;
        if bytes.len() as u64 > self.reserved {
            return Err(StoreError::InvalidInput);
        }
        self.file.rewind()?;
        self.file.write_all(&bytes)?;
        // Keep a bounded real allocation, including space for result metadata and undo state.
        let padding = vec![0u8; self.reserved as usize - bytes.len()];
        self.file.write_all(&padding)?;
        self.file.sync_all()?;
        Ok(())
    }
}
pub fn reserve_or_resume(
    account: &Account,
    record: &OperationRecord,
    root: &AllowedRoot,
    config: &JournalConfig,
    native: bool,
    home_used: u64,
    max_inodes: u64,
) -> Result<Option<Charge>, StoreError> {
    if account.kind == AccountKind::Virtual
        && record.range_next_offset.is_some()
        && record.log_reserve_bytes > 0
    {
        let path = parent(account)?.join(format!("{}.json", record.id));
        root_parent(&path)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(path)?;
        let m = file.metadata()?;
        if !m.is_file()
            || m.nlink() != 1
            || m.uid() != account.uid
            || m.gid() != account.gid
            || m.mode() & 0o777 != 0o400
            || m.len() != record.log_reserve_bytes
        {
            return Err(StoreError::UntrustedStore);
        }
        return Ok(Some(Charge {
            file,
            reserved: record.log_reserve_bytes,
        }));
    }
    reserve(account, record, root, config, native, home_used, max_inodes)
}
pub fn reserve(
    account: &Account,
    record: &OperationRecord,
    root: &AllowedRoot,
    config: &JournalConfig,
    native: bool,
    home_used: u64,
    max_inodes: u64,
) -> Result<Option<Charge>, StoreError> {
    if account.kind != AccountKind::Virtual {
        return Ok(None);
    }
    let limit = root.quota_bytes.ok_or(StoreError::QuotaExceeded)?;
    let reserved = (config.max_record_bytes as u64).div_ceil(4096) * 4096;
    let usage = if native {
        let usage = native_quota::read(root, account.uid)?;
        if usage.inodes >= max_inodes {
            return Err(StoreError::QuotaExceeded);
        }
        usage.used_bytes
    } else {
        home_used
            .checked_add(used(account)?)
            .ok_or(StoreError::InvalidInput)?
    };
    if usage
        .checked_add(reserved)
        .is_none_or(|value| value > limit)
    {
        return Err(StoreError::QuotaExceeded);
    }
    let base = Path::new(crate::layout::VIRTUAL_HOMES).join(".tealdrive-audit");
    directory(&base)?;
    let parent = parent(account)?;
    directory(&parent)?;
    uuid::Uuid::parse_str(&record.id).map_err(|_| StoreError::InvalidInput)?;
    let path = parent.join(format!("{}.json", record.id));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&path)?;
    let result = (|| {
        let rc = unsafe { libc::posix_fallocate(file.as_raw_fd(), 0, reserved as i64) };
        if rc != 0 {
            return Err(std::io::Error::from_raw_os_error(rc).into());
        }
        if unsafe { libc::fchown(file.as_raw_fd(), account.uid, account.gid) } != 0
            || unsafe { libc::fchmod(file.as_raw_fd(), 0o400) } != 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut charge = Charge { file, reserved };
        charge.write(record)?;
        if native {
            let usage = native_quota::read(root, account.uid)?;
            if usage.used_bytes > limit || usage.inodes > max_inodes {
                return Err(StoreError::QuotaExceeded);
            }
        }
        File::open(&parent)?.sync_all()?;
        Ok(charge)
    })();
    match result {
        Ok(charge) => Ok(Some(charge)),
        Err(error) => {
            let _ = std::fs::remove_file(path);
            Err(error)
        }
    }
}

pub fn remove_record(account: &Account, record: &OperationRecord) -> Result<(), StoreError> {
    if account.kind != AccountKind::Virtual {
        return Ok(());
    }
    if record.account_id != account.id {
        return Err(StoreError::Forbidden);
    }
    uuid::Uuid::parse_str(&record.id).map_err(|_| StoreError::InvalidInput)?;
    let path = parent(account)?.join(format!("{}.json", record.id));
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
        Ok(metadata)
            if !metadata.is_file()
                || (metadata.uid() != account.uid
                    && !(metadata.uid() == 0
                        && account.status == super::store::AccountStatus::Deleted))
                || metadata.nlink() != 1
                || metadata.mode() & 0o777 != 0o400 =>
        {
            return Err(StoreError::UntrustedStore)
        }
        Ok(_) => {}
    }
    root_parent(&path)?;
    std::fs::remove_file(path)?;
    Ok(())
}

/// After retained deletion, inaccessible user history moves to the Root system budget.
pub fn retire(account: &Account) -> Result<(), StoreError> {
    if account.kind != AccountKind::Virtual {
        return Ok(());
    }
    let parent = parent(account)?;
    if !parent.try_exists()? {
        return Ok(());
    }
    root_parent(&parent.join("entry"))?;
    for entry in std::fs::read_dir(parent)? {
        let path = entry?.path();
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)?;
        let m = file.metadata()?;
        if !m.is_file()
            || (m.uid() != 0 && m.uid() != account.uid)
            || m.nlink() != 1
            || m.mode() & 0o777 != 0o400
        {
            return Err(StoreError::UntrustedStore);
        }
        if unsafe { libc::fchown(file.as_raw_fd(), 0, 0) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        file.sync_all()?;
    }
    Ok(())
}
