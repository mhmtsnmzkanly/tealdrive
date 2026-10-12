//! Linux quota UAPI through an already verified directory FD (Linux 5.14+).
//! No caller-selected block device or quota file is accepted.
use crate::config::{AllowedRoot, RelativePath};
use std::{io, os::fd::AsRawFd};
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Usage {
    pub used_bytes: u64,
    pub hard_bytes: u64,
    pub inodes: u64,
    pub hard_inodes: u64,
}
fn call(
    file: &std::fs::File,
    cmd: libc::c_int,
    uid: u32,
    data: *mut libc::c_void,
) -> io::Result<()> {
    if unsafe {
        libc::syscall(
            libc::SYS_quotactl_fd,
            file.as_raw_fd(),
            libc::QCMD(cmd, libc::USRQUOTA),
            uid,
            data,
        )
    } == -1
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
fn get(file: &std::fs::File, uid: u32) -> io::Result<libc::dqblk> {
    let mut format = 0u32;
    call(
        file,
        libc::Q_GETFMT,
        0,
        std::ptr::addr_of_mut!(format).cast(),
    )?;
    let mut quota: libc::dqblk = unsafe { std::mem::zeroed() };
    call(
        file,
        libc::Q_GETQUOTA,
        uid,
        std::ptr::addr_of_mut!(quota).cast(),
    )?;
    Ok(quota)
}
pub fn read(root: &AllowedRoot, uid: u32) -> io::Result<Usage> {
    if uid == 0 {
        return Err(io::Error::from(io::ErrorKind::PermissionDenied));
    }
    let file = crate::helper::sandbox::open(
        root,
        &RelativePath::new(""),
        libc::O_RDONLY | libc::O_DIRECTORY,
        0,
    )?;
    let quota = get(&file, uid)?;
    Ok(Usage {
        used_bytes: quota.dqb_curspace,
        hard_bytes: quota.dqb_bhardlimit.saturating_mul(1024),
        inodes: quota.dqb_curinodes,
        hard_inodes: quota.dqb_ihardlimit,
    })
}
pub fn set(root: &AllowedRoot, uid: u32, bytes: u64, inodes: u64) -> io::Result<()> {
    if unsafe { libc::geteuid() } != 0 || uid == 0 || bytes % 1024 != 0 || inodes == 0 {
        return Err(io::Error::from(io::ErrorKind::PermissionDenied));
    }
    let file = crate::helper::sandbox::open(
        root,
        &RelativePath::new(""),
        libc::O_RDONLY | libc::O_DIRECTORY,
        0,
    )?;
    let mut quota = get(&file, uid)?;
    if quota.dqb_curspace > bytes && bytes != 0 || quota.dqb_curinodes > inodes {
        return Err(io::Error::from_raw_os_error(libc::EDQUOT));
    }
    // Zero would mean unlimited in the kernel; use an over-restrictive minimum.
    quota.dqb_bhardlimit = (bytes / 1024).max(1);
    quota.dqb_bsoftlimit = quota.dqb_bhardlimit;
    quota.dqb_ihardlimit = inodes;
    quota.dqb_isoftlimit = inodes;
    quota.dqb_valid = libc::QIF_LIMITS;
    call(
        &file,
        libc::Q_SETQUOTA,
        uid,
        std::ptr::addr_of_mut!(quota).cast(),
    )
}
pub fn assert_limit(root: &AllowedRoot, uid: u32, bytes: u64, inodes: u64) -> io::Result<Usage> {
    let usage = read(root, uid)?;
    if bytes % 1024 != 0 || usage.hard_bytes != bytes.max(1024) || usage.hard_inodes != inodes {
        return Err(io::Error::from(io::ErrorKind::PermissionDenied));
    }
    Ok(usage)
}
