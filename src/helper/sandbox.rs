//! Linux kernel confinement: every filesystem syscall is restricted to one pinned root.
//! No unsafe fallback is used on older kernels or when sandbox setup is denied.
use crate::config::{AllowedRoot, RelativePath};
use std::ffi::CString;
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;

#[repr(C)]
struct OpenHow {
    flags: u64,
    mode: u64,
    resolve: u64,
}
pub fn open(root: &AllowedRoot, path: &RelativePath, flags: i32, mode: u32) -> io::Result<File> {
    let path = crate::fs::path::validate_relative_path(path).map_err(io::Error::other)?;
    crate::helper::access::check(
        &path,
        if flags & (libc::O_WRONLY | libc::O_RDWR | libc::O_CREAT) != 0 {
            crate::policy::access::Access::Write
        } else if flags & libc::O_PATH != 0 {
            crate::policy::access::Access::Discover
        } else {
            crate::policy::access::Access::Read
        },
    )
    .map_err(|e| {
        if e == crate::errors::TealDriveError::NotFound {
            io::Error::from(io::ErrorKind::NotFound)
        } else {
            io::Error::from(io::ErrorKind::PermissionDenied)
        }
    })?;
    let base = CString::new(root.base_path.as_os_str().as_bytes()).map_err(io::Error::other)?;
    let rootfd = unsafe {
        libc::open(
            base.as_ptr(),
            libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if rootfd < 0 {
        return Err(io::Error::last_os_error());
    }
    let rootfd = unsafe { OwnedFd::from_raw_fd(rootfd) };
    let file = open_at(
        rootfd.as_raw_fd(),
        if path.0.is_empty() { "." } else { &path.0 },
        flags,
        mode,
    )?;
    crate::helper::access::check_metadata(&file.metadata()?)
        .map_err(|_| io::Error::from(io::ErrorKind::NotFound))?;
    Ok(file)
}
pub(crate) fn open_at(fd: i32, path: &str, flags: i32, mode: u32) -> io::Result<File> {
    let name = CString::new(path).map_err(io::Error::other)?;
    let how = OpenHow {
        flags: (flags | libc::O_CLOEXEC | libc::O_NOFOLLOW) as u64,
        mode: mode as u64,
        resolve: 0x08 | 0x04,
    }; // BENEATH | NO_SYMLINKS
    let fd = unsafe {
        libc::syscall(
            libc::SYS_openat2,
            fd,
            name.as_ptr(),
            &how,
            std::mem::size_of::<OpenHow>(),
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd as i32) })
}
#[repr(C)]
struct RulesetAttr {
    handled_access_fs: u64,
}
#[repr(C, packed)]
struct PathAttr {
    allowed_access: u64,
    parent_fd: i32,
}
const ACCESS: u64 = (1 << 15) - 1;
pub fn confine(root: &AllowedRoot) -> io::Result<()> {
    let abi = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<u8>(),
            0,
            1,
        )
    };
    if abi < 3 {
        return Err(io::Error::other(
            "Landlock ABI 3 or newer is required (Linux 6.2+)",
        ));
    }
    let name = CString::new(root.base_path.as_os_str().as_bytes()).map_err(io::Error::other)?;
    let fd = unsafe {
        libc::open(
            name.as_ptr(),
            libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let directory = unsafe { OwnedFd::from_raw_fd(fd) };
    let attr = RulesetAttr {
        handled_access_fs: ACCESS,
    };
    let fd = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            &attr,
            std::mem::size_of::<RulesetAttr>(),
            0,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let ruleset = unsafe { OwnedFd::from_raw_fd(fd as i32) };
    // Never permit execution or creation of devices, sockets, FIFOs, or symbolic links.
    let denied = (1 << 0) | (1 << 6) | (1 << 9) | (1 << 10) | (1 << 11) | (1 << 12);
    let writable = ACCESS & !denied;
    let allowed = if root.read_only {
        (1 << 2) | (1 << 3)
    } else {
        writable
    };
    let path = PathAttr {
        allowed_access: allowed,
        parent_fd: directory.as_raw_fd(),
    };
    if unsafe {
        libc::syscall(
            libc::SYS_landlock_add_rule,
            ruleset.as_raw_fd(),
            1,
            &path,
            0,
        )
    } < 0
    {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
        || unsafe { libc::syscall(libc::SYS_landlock_restrict_self, ruleset.as_raw_fd(), 0) } < 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn descriptor_open_rejects_symlinks_in_every_component() {
        let base = std::env::temp_dir().join(format!("tdrv-openat2-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(base.join("real")).unwrap();
        std::fs::write(base.join("real/file"), b"hello").unwrap();
        std::os::unix::fs::symlink(base.join("real"), base.join("link")).unwrap();
        let root = AllowedRoot {
            base_path: base.clone(),
            ..Default::default()
        };
        let result = open(&root, &RelativePath::new("real/file"), libc::O_RDONLY, 0);
        if result.as_ref().err().is_some_and(|e| {
            e.raw_os_error() == Some(libc::ENOSYS) || e.raw_os_error() == Some(libc::EPERM)
        }) {
            eprintln!("openat2 unavailable under current sandbox");
            return;
        }
        assert!(result.is_ok());
        assert!(open(&root, &RelativePath::new("link/file"), libc::O_RDONLY, 0).is_err());
        std::fs::remove_dir_all(base).unwrap();
    }
}
