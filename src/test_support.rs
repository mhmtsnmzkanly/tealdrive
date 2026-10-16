//! Scratch space for tests. Not used by the shipped binaries.
//!
//! Every test process gets one directory under the system temp dir and removes it
//! at exit; roots left by crashed or killed processes are swept on the next run.
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const PREFIX: &str = "tealdrive-tests-";

/// A per-process directory with the same sticky, world-writable mode as /tmp, so
/// tests that drop privileges can still create entries in it.
pub fn scratch_dir() -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let system = std::env::temp_dir();
        sweep_stale(&system);
        let root = system.join(format!(
            "{PREFIX}{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&root).expect("create test scratch root");
        let mode = <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o1777);
        std::fs::set_permissions(&root, mode).expect("set test scratch root mode");
        let _ = CLEANUP_ROOT.set(root.clone());
        // SAFETY: registers a plain extern "C" function once; it only reads a OnceLock.
        unsafe {
            libc::atexit(remove_root);
        }
        root
    })
    .clone()
}

static CLEANUP_ROOT: OnceLock<PathBuf> = OnceLock::new();

extern "C" fn remove_root() {
    if let Some(root) = CLEANUP_ROOT.get() {
        let _ = std::fs::remove_dir_all(root);
    }
}

/// Removes roots whose owning process no longer exists.
fn sweep_stale(system: &Path) {
    let Ok(entries) = std::fs::read_dir(system) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(rest) = name.to_str().and_then(|n| n.strip_prefix(PREFIX)) else {
            continue;
        };
        let Some(pid) = rest
            .split('-')
            .next()
            .and_then(|p| p.parse::<libc::pid_t>().ok())
        else {
            continue;
        };
        // SAFETY: signal 0 only checks whether the process exists.
        let gone = unsafe { libc::kill(pid, 0) } != 0
            && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
        if gone {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}
