//! Publish growing outputs only after successful completion. A failed operation
//! drops its staging output while the root quota lock is still held.
use crate::config::{AppConfig, RelativePath};
use crate::helper::types::{HelperCommand, HelperRequest, HelperResponse};
use std::ffi::CString;
use std::io;
use std::os::fd::AsRawFd;
pub fn rename(
    root: &crate::config::AllowedRoot,
    source: &RelativePath,
    target: &RelativePath,
    replace: bool,
) -> io::Result<()> {
    let split = |p: &RelativePath| -> io::Result<(RelativePath, String)> {
        let clean = crate::fs::path::validate_relative_path(p).map_err(io::Error::other)?;
        match clean.0.rsplit_once('/') {
            Some((p, n)) => Ok((RelativePath::new(p), n.to_owned())),
            None if !clean.0.is_empty() => Ok((RelativePath::new(""), clean.0)),
            _ => Err(io::Error::other("empty target")),
        }
    };
    let (sp, sn) = split(source)?;
    let (tp, tn) = split(target)?;
    let src = crate::helper::sandbox::open(root, &sp, libc::O_PATH | libc::O_DIRECTORY, 0)?;
    let dst = crate::helper::sandbox::open(root, &tp, libc::O_PATH | libc::O_DIRECTORY, 0)?;
    let sn = CString::new(sn).map_err(io::Error::other)?;
    let tn = CString::new(tn).map_err(io::Error::other)?;
    if unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            src.as_raw_fd(),
            sn.as_ptr(),
            dst.as_raw_fd(),
            tn.as_ptr(),
            if replace { 0 } else { libc::RENAME_NOREPLACE },
        )
    } < 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
pub fn execute(request: &HelperRequest, config: &AppConfig) -> HelperResponse {
    crate::helper::access::with_context(request, config, || execute_inner(request, config))
}
fn execute_inner(request: &HelperRequest, config: &AppConfig) -> HelperResponse {
    use HelperCommand::*;
    if !matches!(
        request.command,
        WriteFile | UploadCommit | Copy | CompressZip | CompressTarGz | ExtractArchive
    ) {
        return crate::helper::ipc::handle_helper_request_with_config(request, config);
    }
    let Some(root) = config.roots.iter().find(|r| r.root_id == request.root_id) else {
        return HelperResponse::permission_denied(request, "Unknown root.");
    };
    if let Err(error) = crate::helper::access::authorize(request, config) {
        return crate::helper::listing::helper_error_from_tealdrive(request, error);
    }
    if crate::helper::policy::authorize(request, config).is_err() {
        return HelperResponse::permission_denied(request, "Operation denied by helper policy.");
    }
    let destination = if matches!(request.command, WriteFile | UploadCommit) {
        &request.relative_path
    } else {
        match &request.target_relative_path {
            Some(p) => p,
            None => return HelperResponse::permission_denied(request, "Missing destination."),
        }
    };
    let clean = match crate::fs::path::validate_relative_path(destination) {
        Ok(p) if !p.0.is_empty() => p,
        _ => {
            return HelperResponse::permission_denied(
                request,
                "Destination must be a new file or directory.",
            )
        }
    };
    let expected = request
        .write_options
        .as_ref()
        .and_then(|w| w.expected_revision.as_deref());
    if request.command == WriteFile
        && expected.is_some_and(|token| {
            !crate::helper::revision::matches(
                root,
                &clean,
                token,
                request.limits.max_text_edit_size,
            )
        })
    {
        return crate::helper::listing::helper_error(
            request,
            crate::errors::ErrorKind::RevisionConflict,
            "File revision changed.",
            "revision_conflict",
        );
    }
    let parent = clean.0.rsplit_once('/').map_or("", |(p, _)| p);
    let extension = if request.command == CompressZip {
        ".zip"
    } else if request.command == CompressTarGz {
        ".tar.gz"
    } else {
        ""
    };
    let name = format!(
        ".tdrv-transaction-{}{extension}",
        super::artifact::current().unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
    );
    let staging = RelativePath::new(if parent.is_empty() {
        name
    } else {
        format!("{parent}/{name}")
    });
    let mut staged = request.clone();
    if matches!(request.command, WriteFile | UploadCommit) {
        staged.relative_path = staging.clone();
    } else {
        staged.target_relative_path = Some(staging.clone());
    }
    // Policy was checked on the actual destination, not on the private staging name.
    let mut stage_config = config.clone();
    stage_config
        .roots
        .iter_mut()
        .for_each(|r| r.hidden_files_allowed = true);
    let staging_path = root.base_path.join(&staging.0);
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(staging_path);
    let available = crate::helper::quota::budget().map(|remaining| {
        if request.command == WriteFile {
            let previous = crate::helper::sandbox::open(root, &clean, libc::O_PATH, 0)
                .ok()
                .and_then(|f| f.metadata().ok())
                .filter(|m| m.is_file())
                .map_or(0, |m| m.len());
            remaining.saturating_add(previous)
        } else {
            remaining
        }
    });
    crate::helper::access::with_alias(staging.clone(), clean.clone(), || {
        let mut response = crate::helper::quota::with_budget(available, || {
            crate::helper::policy::with_snapshot(config, || {
                crate::helper::ipc::handle_helper_request_with_config(&staged, &stage_config)
            })
        });
        if response.success && crate::helper::cancellation::check().is_err() {
            return HelperResponse::permission_denied(
                request,
                "Operation cancelled before publication.",
            );
        }
        if response.success
            && request.command == WriteFile
            && expected.is_some_and(|token| {
                !crate::helper::revision::matches(
                    root,
                    &clean,
                    token,
                    request.limits.max_text_edit_size,
                )
            })
        {
            return crate::helper::listing::helper_error(
                request,
                crate::errors::ErrorKind::RevisionConflict,
                "File revision changed before publication.",
                "revision_conflict",
            );
        }
        if response.success && request.command == WriteFile {
            if let Err(error) = preserve_permissions(root, &clean, &staging) {
                return crate::helper::listing::helper_error_from_io(request, error);
            }
        }
        if response.success {
            if let Err(error) = rename(root, &staging, &clean, request.command == WriteFile) {
                response = crate::helper::listing::helper_error_from_io(request, error);
            }
        }
        response
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concurrent_uploads_share_one_quota_reservation() {
        let base = crate::test_support::scratch_dir()
            .join(format!("tdrv-quota-concurrency-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&base).unwrap();
        let config = AppConfig {
            roots: vec![crate::config::AllowedRoot {
                base_path: base.clone(),
                quota_bytes: Some(600),
                ..Default::default()
            }],
            ..Default::default()
        };
        let handles: Vec<_> = (0..8)
            .map(|index| {
                let config = config.clone();
                std::thread::spawn(move || {
                    let mut request = crate::helper::ipc::tests_support::sample_request(
                        HelperCommand::UploadCommit,
                    );
                    request.relative_path = RelativePath::new(format!("file-{index}"));
                    request.write_options = Some(crate::helper::types::HelperWriteOptions {
                        stream_size: None,
                        expected_revision: None,
                        data: vec![1; 250],
                        offset: 0,
                        truncate: true,
                    });
                    let reservation =
                        crate::helper::quota::Reservation::acquire(&config.roots[0]).unwrap();
                    crate::helper::quota::with_budget(reservation.available, || {
                        execute(&request, &config)
                    })
                    .success
                })
            })
            .collect();
        let successes = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .filter(|ok| *ok)
            .count();
        assert_eq!(successes, 2);
        assert_eq!(crate::fs::quota::compute_directory_size(&base), 500);
        assert_eq!(
            std::fs::read_dir(&base).unwrap().count(),
            2,
            "failed transactions must leave no partial outputs"
        );
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn extraction_failure_keeps_destination_invisible() {
        let base = crate::test_support::scratch_dir()
            .join(format!("tdrv-extract-rollback-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&base).unwrap();
        std::fs::write(base.join("broken.zip"), b"invalid").unwrap();
        let mut config = AppConfig {
            roots: vec![crate::config::AllowedRoot {
                base_path: base.clone(),
                ..Default::default()
            }],
            ..Default::default()
        };
        config.features.extract_archive = true;
        let mut request =
            crate::helper::ipc::tests_support::sample_request(HelperCommand::ExtractArchive);
        request.relative_path = RelativePath::new("broken.zip");
        request.target_relative_path = Some(RelativePath::new("output"));
        assert!(!execute(&request, &config).success);
        assert!(!base.join("output").exists());
        assert_eq!(std::fs::read_dir(&base).unwrap().count(), 1);
        std::fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(test)]
mod archive_regressions {
    use super::*;
    use std::io::Write;
    #[test]
    fn archives_filter_hidden_sensitive_and_symlink_descendants() {
        let base = crate::test_support::scratch_dir()
            .join(format!("tdrv-archive-filter-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(base.join("source")).unwrap();
        std::fs::write(base.join("source/visible.txt"), b"visible").unwrap();
        std::fs::write(base.join("source/.secret"), b"secret").unwrap();
        std::fs::write(base.join("source/password.key"), b"private").unwrap();
        std::os::unix::fs::symlink("/etc/passwd", base.join("source/link")).unwrap();
        let config = AppConfig {
            roots: vec![crate::config::AllowedRoot {
                base_path: base.clone(),
                ..Default::default()
            }],
            ..Default::default()
        };
        for (command, name) in [
            (HelperCommand::CompressZip, "output.zip"),
            (HelperCommand::CompressTarGz, "output.tar.gz"),
        ] {
            let mut request = crate::helper::ipc::tests_support::sample_request(command);
            request.archive_source_paths = Some(vec![RelativePath::new("source")]);
            request.target_relative_path = Some(RelativePath::new(name));
            assert!(execute(&request, &config).success);
        }
        let mut zip =
            zip::ZipArchive::new(std::fs::File::open(base.join("output.zip")).unwrap()).unwrap();
        let names: Vec<_> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_owned())
            .collect();
        assert_eq!(names.len(), 2);
        assert!(names.iter().any(|p| p == "source/visible.txt"));
        let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(
            std::fs::File::open(base.join("output.tar.gz")).unwrap(),
        ));
        let names: Vec<_> = tar
            .entries()
            .unwrap()
            .map(|e| e.unwrap().path().unwrap().into_owned())
            .collect();
        assert_eq!(names.len(), 2);
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn extraction_cannot_publish_hidden_archive_entries() {
        let base = crate::test_support::scratch_dir()
            .join(format!("tdrv-extract-hidden-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&base).unwrap();
        let mut zip = zip::ZipWriter::new(std::fs::File::create(base.join("input.zip")).unwrap());
        zip.start_file("visible.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"visible").unwrap();
        zip.start_file("nested/.secret", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"secret").unwrap();
        zip.finish().unwrap();
        let mut config = AppConfig {
            roots: vec![crate::config::AllowedRoot {
                base_path: base.clone(),
                ..Default::default()
            }],
            ..Default::default()
        };
        config.features.extract_archive = true;
        let mut request =
            crate::helper::ipc::tests_support::sample_request(HelperCommand::ExtractArchive);
        request.relative_path = RelativePath::new("input.zip");
        request.target_relative_path = Some(RelativePath::new("output"));
        assert!(!execute(&request, &config).success);
        assert!(!base.join("output").exists());
        assert_eq!(std::fs::read_dir(&base).unwrap().count(), 1);
        std::fs::remove_dir_all(base).unwrap();
    }
}

fn preserve_permissions(
    root: &crate::config::AllowedRoot,
    original: &RelativePath,
    staged: &RelativePath,
) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let source = match crate::helper::sandbox::open(root, original, libc::O_RDONLY, 0) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let metadata = source.metadata()?;
    let target = crate::helper::sandbox::open(root, staged, libc::O_WRONLY, 0)?;
    if unsafe { libc::fchown(target.as_raw_fd(), metadata.uid(), metadata.gid()) } < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::fchmod(target.as_raw_fd(), metadata.mode() & 0o777) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let attribute = c"system.posix_acl_access";
    let size = unsafe {
        libc::fgetxattr(
            source.as_raw_fd(),
            attribute.as_ptr(),
            std::ptr::null_mut(),
            0,
        )
    };
    if size < 0 {
        let error = io::Error::last_os_error();
        return if matches!(
            error.raw_os_error(),
            Some(libc::ENODATA) | Some(libc::EOPNOTSUPP)
        ) {
            Ok(())
        } else {
            Err(error)
        };
    }
    if size > 65536 {
        return Err(io::Error::other("ACL too large"));
    }
    let mut value = vec![0u8; size as usize];
    let read = unsafe {
        libc::fgetxattr(
            source.as_raw_fd(),
            attribute.as_ptr(),
            value.as_mut_ptr().cast(),
            value.len(),
        )
    };
    if read < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe {
        libc::fsetxattr(
            target.as_raw_fd(),
            attribute.as_ptr(),
            value.as_ptr().cast(),
            read as usize,
            0,
        )
    } < 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod noreplace_tests {
    use super::*;
    #[test]
    fn destination_created_after_validation_is_never_overwritten() {
        let home = crate::test_support::scratch_dir()
            .join(format!("td-noreplace-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&home).unwrap();
        std::fs::write(home.join("source"), b"source").unwrap();
        let root = crate::config::AllowedRoot {
            base_path: home.clone(),
            ..Default::default()
        };
        assert!(!home.join("destination").exists());
        // An external writer publishes after the application checked the target.
        std::fs::write(home.join("destination"), b"external").unwrap();
        let error = rename(
            &root,
            &RelativePath::new("source"),
            &RelativePath::new("destination"),
            false,
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(home.join("source")).unwrap(), b"source");
        assert_eq!(
            std::fs::read(home.join("destination")).unwrap(),
            b"external"
        );
        std::fs::remove_file(home.join("source")).unwrap();
        std::fs::remove_file(home.join("destination")).unwrap();
        std::fs::remove_dir(home).unwrap();
    }
}
