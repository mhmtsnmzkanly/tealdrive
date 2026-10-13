//! Bounded optimistic revisions. Content and file identity come from the same open FD.
use crate::config::{AllowedRoot, RelativePath};
use sha2::{Digest, Sha256};
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
fn identity(m: &std::fs::Metadata) -> String {
    format!(
        "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
        m.dev(),
        m.ino(),
        m.mode(),
        m.uid(),
        m.gid(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec()
    )
}
pub fn read(
    root: &AllowedRoot,
    path: &RelativePath,
    limit: usize,
) -> io::Result<(Vec<u8>, String, std::fs::Metadata)> {
    let mut file = crate::helper::sandbox::open(root, path, libc::O_RDONLY, 0)?;
    let before = file.metadata()?;
    if !before.is_file() || before.len() > limit as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a bounded regular file",
        ));
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    if bytes.len() > limit || identity(&before) != identity(&after) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "file changed during read",
        ));
    }
    let mut hash = Sha256::new();
    hash.update(identity(&after));
    hash.update([0]);
    hash.update(&bytes);
    Ok((bytes, format!("{:x}", hash.finalize()), after))
}
pub fn matches(root: &AllowedRoot, path: &RelativePath, expected: &str, limit: usize) -> bool {
    if expected == "missing" {
        return crate::helper::sandbox::open(root, path, libc::O_PATH, 0)
            .is_err_and(|e| e.kind() == io::ErrorKind::NotFound);
    }
    read(root, path, limit).is_ok_and(|(_, revision, _)| revision == expected)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_revision_never_overwrites_original_and_mode_is_preserved() {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!("tdrv-revision-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&base).unwrap();
        std::fs::write(base.join("note.txt"), b"before").unwrap();
        std::fs::set_permissions(
            base.join("note.txt"),
            std::fs::Permissions::from_mode(0o640),
        )
        .unwrap();
        let root = AllowedRoot {
            base_path: base.clone(),
            ..Default::default()
        };
        let (_, revision, _) = read(&root, &RelativePath::new("note.txt"), 1024).unwrap();
        std::fs::write(base.join("note.txt"), b"external edit").unwrap();
        let mut config = crate::config::AppConfig {
            roots: vec![root.clone()],
            ..Default::default()
        };
        config.features.save_text_file = true;
        let mut request = crate::helper::ipc::tests_support::sample_request(
            crate::helper::types::HelperCommand::WriteFile,
        );
        request.relative_path = RelativePath::new("note.txt");
        request.write_options = Some(crate::helper::types::HelperWriteOptions {
            stream_size: None,
            expected_revision: Some(revision),
            data: b"panel edit".to_vec(),
            offset: 0,
            truncate: true,
        });
        let result = crate::helper::transaction::execute(&request, &config);
        assert_eq!(
            result.error_kind,
            Some(crate::errors::ErrorKind::RevisionConflict)
        );
        assert_eq!(
            std::fs::read(base.join("note.txt")).unwrap(),
            b"external edit"
        );
        let (_, fresh, _) = read(&root, &request.relative_path, 1024).unwrap();
        request.write_options.as_mut().unwrap().expected_revision = Some(fresh);
        assert!(crate::helper::transaction::execute(&request, &config).success);
        assert_eq!(std::fs::read(base.join("note.txt")).unwrap(), b"panel edit");
        assert_eq!(
            std::fs::metadata(base.join("note.txt")).unwrap().mode() & 0o777,
            0o640
        );
        request.write_options.as_mut().unwrap().expected_revision = Some("missing".into());
        assert_eq!(
            crate::helper::transaction::execute(&request, &config).error_kind,
            Some(crate::errors::ErrorKind::RevisionConflict)
        );
        assert_eq!(std::fs::read_dir(&base).unwrap().count(), 1);
        std::fs::remove_dir_all(base).unwrap();
    }
}
