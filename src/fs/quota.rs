use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::config::AllowedRoot;
use crate::errors::TealDriveError;

/// Recursively computes total size of all regular files in a directory,
/// safely ignoring symlinks and preventing directory loop recursion.
pub fn compute_directory_size(root_path: &Path) -> u64 {
    let mut total_size = 0u64;
    let mut queue = vec![(root_path.to_path_buf(), 0usize)];
    let mut visited = HashSet::new();

    if let Ok(canon) = root_path.canonicalize() {
        visited.insert(canon);
    }

    while let Some((dir, depth)) = queue.pop() {
        if depth > 40 {
            continue;
        }

        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let meta = match fs::symlink_metadata(entry.path()) {
                Ok(meta) => meta,
                Err(_) => continue,
            };

            // Never follow symlinks to avoid escapes and loops
            if meta.is_symlink() {
                continue;
            }

            if meta.is_dir() {
                let path = entry.path();
                if let Ok(canon) = path.canonicalize() {
                    if visited.insert(canon) {
                        queue.push((path, depth + 1));
                    }
                }
            } else if meta.is_file() {
                total_size = total_size.saturating_add(meta.len());
            }
        }
    }

    total_size
}

/// Checks if writing `additional_bytes` to `root` would exceed its configured `quota_bytes`.
pub fn check_quota(root: &AllowedRoot, additional_bytes: u64) -> Result<(), TealDriveError> {
    if let Some(quota) = root.quota_bytes {
        let current_usage = compute_directory_size(&root.base_path);
        if current_usage.saturating_add(additional_bytes) > quota {
            return Err(TealDriveError::DiskFull);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::config::RootId;

    fn temp_test_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("tdrv-quota-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path.canonicalize().unwrap()
    }

    #[test]
    fn compute_directory_size_sums_files() {
        let base = temp_test_dir();
        fs::write(base.join("a.txt"), b"12345").unwrap(); // 5 bytes
        fs::create_dir_all(base.join("sub")).unwrap();
        fs::write(base.join("sub/b.txt"), b"1234567890").unwrap(); // 10 bytes

        let size = compute_directory_size(&base);
        assert_eq!(size, 15);

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn compute_directory_size_ignores_symlinks() {
        let base = temp_test_dir();
        fs::write(base.join("file.txt"), b"12345").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let _ = symlink(base.join("file.txt"), base.join("link.txt"));
        }

        let size = compute_directory_size(&base);
        assert_eq!(size, 5);

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn check_quota_enforces_limit() {
        let base = temp_test_dir();
        fs::write(base.join("existing.txt"), b"12345").unwrap(); // 5 bytes

        let root = AllowedRoot {
            root_id: RootId::new("home"),
            base_path: base.clone(),
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
            quota_bytes: Some(10),
        };

        // 5 + 4 <= 10 -> ok
        assert!(check_quota(&root, 4).is_ok());

        // 5 + 6 > 10 -> DiskFull
        assert_eq!(check_quota(&root, 6), Err(TealDriveError::DiskFull));

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn check_quota_unlimited_when_none() {
        let base = temp_test_dir();
        let root = AllowedRoot {
            root_id: RootId::new("home"),
            base_path: base.clone(),
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
            quota_bytes: None,
        };

        assert!(check_quota(&root, 1024 * 1024 * 1024).is_ok());
        let _ = fs::remove_dir_all(base);
    }
}
