//! Dedicated ext4 quota image. Never remounts the host filesystem or adopts user data.
use super::*;
const IMAGE: &str = "/home/tealdrive/storage/virtual-pool.ext4";
const MOUNT_UNIT: &str = "home-tealdrive-virtual\\x2dhomes.mount";
fn unit_path() -> PathBuf {
    Path::new("/etc/systemd/system").join(MOUNT_UNIT)
}
fn root_path() -> crate::config::AllowedRoot {
    crate::config::AllowedRoot {
        base_path: layout::VIRTUAL_HOMES.into(),
        ..Default::default()
    }
}
fn image() -> Result<std::fs::File> {
    trusted_directory(Path::new(IMAGE).parent().ok_or("Image path")?)?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(IMAGE)?;
    let m = file.metadata()?;
    if !m.is_file() || m.uid() != 0 || m.nlink() != 1 || m.mode() & 0o077 != 0 {
        return Err("Untrusted quota image".into());
    }
    Ok(file)
}
fn mounted() -> Result<bool> {
    trusted_directory(Path::new(layout::VIRTUAL_HOMES))?;
    Ok(std::fs::metadata(layout::VIRTUAL_HOMES)?.dev() != std::fs::metadata(layout::HOME)?.dev())
}
pub(super) fn inspect() -> Result<serde_json::Value> {
    if !mounted()? {
        return Err("Dedicated virtual volume is not mounted; no fallback.".into());
    }
    let root = root_path();
    // FD-based Q_GETFMT/Q_GETQUOTA proves actual kernel user quota support.
    crate::helper::native_quota::read(&root, service_identity()?.uid)?;
    let path = std::ffi::CString::new(layout::VIRTUAL_HOMES)?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(path.as_ptr(), &mut stat) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let total = stat.f_blocks.saturating_mul(stat.f_frsize);
    // Filesystem metadata, Root-reserved blocks and operational headroom stay outside allocations.
    let capacity = stat
        .f_blocks
        .saturating_sub(stat.f_bfree.saturating_sub(stat.f_bavail))
        .saturating_mul(stat.f_frsize)
        .saturating_mul(95)
        / 100
        / 1024
        * 1024;
    Ok(
        serde_json::json!({"mounted":true,"kernelUserQuota":true,"totalBytes":total,"poolCapacityBytes":capacity,"freeBytes":stat.f_bavail.saturating_mul(stat.f_frsize),"path":layout::VIRTUAL_HOMES,"imageBytes":image()?.metadata()?.len(),"pool":store()?.pool_status()?,"expansionPending":Path::new(EXPANSION).symlink_metadata().is_ok()}),
    )
}
pub(super) fn init(size: u64) -> Result<()> {
    stopped()?;
    let _lock = crate::accounts::virtuals::lock()?;
    if size < 128 * 1024 * 1024 || size > i64::MAX as u64 || size % 4096 != 0 {
        return Err("Volume size must be at least128MiB and aligned to4096 bytes.".into());
    }
    trusted_directory(Path::new(IMAGE).parent().ok_or("Image parent")?)?;
    trusted_directory(Path::new("/etc/systemd/system"))?;
    if mounted()?
        || Path::new(IMAGE).symlink_metadata().is_ok()
        || unit_path().symlink_metadata().is_ok()
        || std::fs::read_dir(layout::VIRTUAL_HOMES)?.next().is_some()
    {
        return Err(
            "Refusing to replace a volume, mount unit or nonempty virtual home directory.".into(),
        );
    }
    let store = store()?;
    if !store
        .all_accounts(&Authority::Root, AccountKind::Virtual)?
        .is_empty()
        || !crate::accounts::virtuals::pending(&store)?.is_empty()
    {
        return Err(
            "Storage initialization requires no virtual identities or provisioning intents.".into(),
        );
    }
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(IMAGE)?;
    // Allocate backing blocks now: sparse overcommit would turn logical quotas into false capacity.
    let result = unsafe { libc::posix_fallocate(file.as_raw_fd(), 0, i64::try_from(size)?) };
    if result != 0 {
        drop(file);
        std::fs::remove_file(IMAGE)?;
        return Err(std::io::Error::from_raw_os_error(result).into());
    }
    file.sync_all()?;
    drop(file);
    // A failed mkfs/mount is left for explicit Root inspection; never delete an existing volume.
    command(
        "/usr/sbin/mke2fs",
        &[
            "-q",
            "-t",
            "ext4",
            "-F",
            "-O",
            "quota",
            "-E",
            "quotatype=usrquota",
            IMAGE,
        ],
    )?;
    command(
        "/usr/bin/mount",
        &[
            "-o",
            "loop,nodev,nosuid,noexec,usrquota",
            IMAGE,
            layout::VIRTUAL_HOMES,
        ],
    )?;
    std::fs::set_permissions(
        layout::VIRTUAL_HOMES,
        std::fs::Permissions::from_mode(0o711),
    )?;
    let report = inspect()?;
    let body=format!("[Unit]\nDescription=TealDrive dedicated virtual user quota volume\nBefore=tealdrive.service\n\n[Mount]\nWhat={IMAGE}\nWhere={}\nType=ext4\nOptions=loop,nodev,nosuid,noexec,usrquota\n\n[Install]\nWantedBy=multi-user.target\n",layout::VIRTUAL_HOMES);
    atomic(&unit_path(), body.as_bytes(), 0o644, 0, 0)?;
    let dropin = Path::new("/etc/systemd/system/tealdrive.service.d");
    directory(dropin, 0, 0, 0o755)?;
    let dependency = format!("[Unit]\nRequires={MOUNT_UNIT}\nAfter={MOUNT_UNIT}\n");
    atomic(
        &dropin.join("virtual-storage.conf"),
        dependency.as_bytes(),
        0o644,
        0,
        0,
    )?;
    command("/usr/bin/systemctl", &["daemon-reload"])?;
    command("/usr/bin/systemctl", &["enable", MOUNT_UNIT])?;
    print(&report)?;
    println!("Volume ready. Set a Root pool within poolCapacityBytes and select virtual_accounts.enforcement=user_quota before enabling virtual accounts.");
    Ok(())
}
pub(super) fn mount() -> Result<()> {
    stopped()?;
    let _lock = crate::accounts::virtuals::lock()?;
    image()?;
    if !mounted()? {
        if std::fs::read_dir(layout::VIRTUAL_HOMES)?.next().is_some() {
            return Err("Refusing to hide existing virtual data beneath a mount.".into());
        }
        command(
            "/usr/bin/mount",
            &[
                "-o",
                "loop,nodev,nosuid,noexec,usrquota",
                IMAGE,
                layout::VIRTUAL_HOMES,
            ],
        )?;
    }
    print(&inspect()?)
}
pub(super) fn check_pool(size: u64) -> Result<()> {
    if config()?.virtual_accounts.enforcement == crate::accounts::virtuals::Enforcement::UserQuota {
        let capacity = inspect()?["poolCapacityBytes"]
            .as_u64()
            .ok_or("Missing capacity")?;
        if size > capacity {
            return Err("Pool exceeds dedicated filesystem capacity/headroom.".into());
        }
    }
    Ok(())
}

const EXPANSION: &str = "/home/tealdrive/storage/expansion.json";
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Expansion {
    pool_bytes: u64,
    image_bytes: u64,
    device: u64,
    inode: u64,
}
fn planned_image_bytes(pool: u64) -> Result<u64> {
    if pool < 4096 || pool % 1024 != 0 {
        return Err("Pool must be at least4KiB and aligned to1KiB.".into());
    }
    let size = pool
        .checked_mul(5)
        .and_then(|v| v.checked_add(3))
        .map(|v| v / 4)
        .and_then(|v| v.checked_add(4095))
        .map(|v| v / 4096 * 4096)
        .ok_or("Image size overflow")?
        .max(
            pool.checked_add(64 * 1024 * 1024)
                .ok_or("Image size overflow")?,
        )
        .max(128 * 1024 * 1024);
    if size > i64::MAX as u64 {
        return Err("Image size overflow".into());
    }
    Ok(size)
}
pub(super) fn init_pool(pool: u64) -> Result<()> {
    let mut cfg = config()?;
    if !cfg.web_accounts.enabled || cfg.home_template.is_none() {
        return Err("Web accounts and home template must be initialized first".into());
    }
    init(planned_image_bytes(pool)?)?;
    let report = inspect()?;
    if pool
        > report["poolCapacityBytes"]
            .as_u64()
            .ok_or("Missing capacity")?
    {
        return Err("Filesystem cannot provide requested net pool".into());
    }
    store()?.set_pool(&Authority::Root, pool)?;
    cfg.virtual_accounts.enforcement = crate::accounts::virtuals::Enforcement::UserQuota;
    cfg.virtual_accounts.enabled = true;
    save_configs(&cfg)?;
    println!("Net distributable pool published after capacity validation.");
    print(&inspect()?)
}
fn loop_device(file: &std::fs::File) -> Result<String> {
    use std::os::unix::fs::FileTypeExt;
    let information = std::fs::read_to_string("/proc/self/mountinfo")?;
    let source = information
        .lines()
        .filter_map(|line| {
            let (head, tail) = line.split_once(" - ")?;
            if head.split_whitespace().nth(4) != Some(layout::VIRTUAL_HOMES) {
                return None;
            }
            let mut fields = tail.split_whitespace();
            if fields.next() != Some("ext4") {
                return None;
            }
            fields.next().map(str::to_owned)
        })
        .next_back()
        .ok_or("Expected ext4 quota mount not found")?;
    let number = source
        .strip_prefix("/dev/loop")
        .ok_or("Expected loop device")?;
    if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Invalid loop device".into());
    }
    let metadata = std::fs::symlink_metadata(&source)?;
    if !metadata.file_type().is_block_device()
        || metadata.uid() != 0
        || metadata.rdev() != std::fs::metadata(layout::VIRTUAL_HOMES)?.dev()
    {
        return Err("Untrusted loop device".into());
    }
    let backing =
        std::fs::read_to_string(format!("/sys/class/block/loop{number}/loop/backing_file"))?;
    let backing = std::fs::metadata(backing.trim())?;
    let expected = file.metadata()?;
    if backing.dev() != expected.dev() || backing.ino() != expected.ino() {
        return Err("Mounted loop does not use the protected image".into());
    }
    Ok(source)
}
pub(super) fn check() -> Result<()> {
    let image = image()?;
    loop_device(&image)?;
    let report = inspect()?;
    if store()?.pool_limit()?
        > report["poolCapacityBytes"]
            .as_u64()
            .ok_or("Missing capacity")?
    {
        return Err("Pool exceeds verified capacity".into());
    }
    print(&report)
}
pub(super) fn expand(pool: u64) -> Result<()> {
    for _ in 0..3 {
        if expand_once(pool)? {
            return Ok(());
        }
    }
    Err("Capacity needs another verified expansion; resume the same net pool target".into())
}
fn expand_once(pool: u64) -> Result<bool> {
    stopped()?;
    let _lock = crate::accounts::virtuals::lock()?;
    let original = image()?;
    let device = loop_device(&original)?;
    let metadata = original.metadata()?;
    let desired = planned_image_bytes(pool)?;
    let mut store = store()?;
    let current = store.pool_limit()?;
    let path = Path::new(EXPANSION);
    if current > pool {
        return Err("Current pool exceeds requested target; shrink is unsupported".into());
    }
    let mut pending = if path.symlink_metadata().is_ok() {
        let mut record = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)?;
        let info = record.metadata()?;
        if !info.is_file()
            || info.uid() != 0
            || info.nlink() != 1
            || info.mode() & 0o077 != 0
            || info.len() > 4096
        {
            return Err("Untrusted expansion record".into());
        }
        let mut data = Vec::new();
        record.read_to_end(&mut data)?;
        let pending: Expansion = serde_json::from_slice(&data)?;
        if pending.pool_bytes != pool
            || pending.image_bytes < 128 * 1024 * 1024
            || pending.image_bytes > i64::MAX as u64
            || pending.image_bytes % 4096 != 0
            || pending.device != metadata.dev()
            || pending.inode != metadata.ino()
        {
            return Err("Resume the matching expansion; image/target changed".into());
        }
        pending
    } else {
        if pool <= current {
            return Err("Expansion requires a larger net pool; shrink is unsupported".into());
        }
        let pending = Expansion {
            pool_bytes: pool,
            image_bytes: desired,
            device: metadata.dev(),
            inode: metadata.ino(),
        };
        atomic(path, &serde_json::to_vec(&pending)?, 0o600, 0, 0)?;
        pending
    };
    pending.image_bytes = pending.image_bytes.max(desired);
    atomic(path, &serde_json::to_vec(&pending)?, 0o600, 0, 0)?;
    let mut writable = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(IMAGE)?;
    let actual = writable.metadata()?;
    if actual.dev() != pending.device || actual.ino() != pending.inode {
        return Err("Image changed before expansion".into());
    }
    // Reserve the whole image again on retry; a failed previous fallocate may
    // have left holes even when the apparent file size reached the target.
    let reserve = pending.image_bytes.max(actual.len());
    let additional = reserve.saturating_sub(actual.blocks().saturating_mul(512));
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstatvfs(writable.as_raw_fd(), &mut stat) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let required = additional
        .checked_add(1024 * 1024 * 1024)
        .ok_or("Disk reservation overflow")?;
    if stat.f_bavail.saturating_mul(stat.f_frsize) < required {
        return Err(
            "Insufficient backing space plus1GiB system headroom; old pool retained".into(),
        );
    }
    let result = unsafe { libc::posix_fallocate(writable.as_raw_fd(), 0, i64::try_from(reserve)?) };
    if result != 0 {
        return Err(std::io::Error::from_raw_os_error(result).into());
    }
    writable.flush()?;
    writable.sync_all()?;
    command("/usr/sbin/losetup", &["--set-capacity", &device])?;
    command("/usr/sbin/resize2fs", &[&device])?;
    let verified = inspect()?;
    let capacity = verified["poolCapacityBytes"]
        .as_u64()
        .ok_or("Missing capacity")?;
    if pool > capacity {
        if capacity == 0 {
            return Err("Filesystem capacity is zero; old pool retained".into());
        }
        let adjusted = (reserve as u128)
            .checked_mul(pool as u128)
            .and_then(|v| v.checked_mul(105))
            .map(|v| v / (capacity as u128) / 100)
            .and_then(|v| v.checked_add(4095))
            .map(|v| v / 4096 * 4096)
            .ok_or("Image adjustment overflow")?;
        pending.image_bytes = u64::try_from(adjusted)?;
        if pending.image_bytes > i64::MAX as u64 || pending.image_bytes <= reserve {
            return Err("Cannot reserve verified net capacity".into());
        }
        atomic(path, &serde_json::to_vec(&pending)?, 0o600, 0, 0)?;
        return Ok(false);
    }
    store.set_pool(&Authority::Root, pool)?;
    std::fs::remove_file(path)?;
    std::fs::File::open(path.parent().ok_or("Expansion parent")?)?.sync_all()?;
    println!("Expanded image/filesystem verified; net pool now available. Account quotas were not changed.");
    print(&inspect()?)?;
    Ok(true)
}
#[cfg(test)]
mod expansion_tests {
    #[test]
    fn capacity_planning_is_aligned_and_never_overflows() {
        assert_eq!(
            super::planned_image_bytes(64 * 1024 * 1024).unwrap(),
            128 * 1024 * 1024
        );
        assert_eq!(
            super::planned_image_bytes(128 * 1024 * 1024).unwrap(),
            192 * 1024 * 1024
        );
        assert!(super::planned_image_bytes(1).is_err());
        assert!(super::planned_image_bytes(u64::MAX).is_err());
    }
}
