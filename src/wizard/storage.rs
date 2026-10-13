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
        serde_json::json!({"mounted":true,"kernelUserQuota":true,"totalBytes":total,"poolCapacityBytes":capacity,"freeBytes":stat.f_bavail.saturating_mul(stat.f_frsize),"path":layout::VIRTUAL_HOMES}),
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
