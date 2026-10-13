use std::io::{Read, Seek, SeekFrom, Write};

use crate::config::{AllowedRoot, RelativePath, SensitivePolicyConfig};
use crate::errors::ErrorKind;
use crate::fs::filename::SafeFileName;
use crate::fs::path::{resolve_new_target, validate_relative_path};
use crate::helper::listing::{helper_error, helper_error_from_io, helper_error_from_tealdrive};
use crate::helper::types::{HelperCommand, HelperRequest, HelperResponse};
use crate::policy::decision::PolicyDecision;
use crate::policy::sensitive::detect_sensitive;
use crate::policy::webroot::check_webroot_upload;

// Explicit descriptor forwarding across both privilege-separation execs.
const SOURCE_FD: libc::c_int = 197;
pub(crate) fn inherit_source(command: &mut std::process::Command, source: &std::fs::File) {
    use std::os::{fd::AsRawFd, unix::process::CommandExt};
    let fd = source.as_raw_fd();
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(fd, SOURCE_FD) < 0 || libc::fcntl(SOURCE_FD, libc::F_SETFD, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}
pub(crate) fn source(request: &HelperRequest) -> Result<Option<std::fs::File>, std::io::Error> {
    use std::os::{fd::FromRawFd, unix::fs::MetadataExt};
    let Some(size) = request.write_options.as_ref().and_then(|v| v.stream_size) else {
        return Ok(None);
    };
    if request.command != HelperCommand::UploadCommit
        || request
            .write_options
            .as_ref()
            .is_none_or(|v| !v.data.is_empty() || v.offset != 0 || !v.truncate)
    {
        return Err(std::io::ErrorKind::PermissionDenied.into());
    }
    let flags = unsafe { libc::fcntl(SOURCE_FD, libc::F_GETFL) };
    if flags < 0 || flags & libc::O_ACCMODE != libc::O_RDONLY {
        return Err(std::io::ErrorKind::PermissionDenied.into());
    }
    let duplicate = unsafe { libc::fcntl(SOURCE_FD, libc::F_DUPFD_CLOEXEC, 198) };
    if duplicate < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let file = unsafe { std::fs::File::from_raw_fd(duplicate) };
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::getuid() }
        || metadata.nlink() != 1
        || metadata.len() != size
        || metadata.mode() & 0o077 != 0
    {
        return Err(std::io::ErrorKind::PermissionDenied.into());
    }
    Ok(Some(file))
}
thread_local! {static STREAM: std::cell::RefCell<Option<std::fs::File>> = const {std::cell::RefCell::new(None)};}
pub(crate) fn with_source<T>(file: Option<std::fs::File>, action: impl FnOnce() -> T) -> T {
    struct Reset(Option<std::fs::File>);
    impl Drop for Reset {
        fn drop(&mut self) {
            STREAM.with(|slot| *slot.borrow_mut() = self.0.take());
        }
    }
    let _reset = Reset(STREAM.with(|slot| slot.replace(file)));
    action()
}
pub fn write_file_with_roots(
    request: &HelperRequest,
    roots: &[AllowedRoot],
    sensitive_config: &SensitivePolicyConfig,
) -> HelperResponse {
    if !matches!(
        request.command,
        HelperCommand::WriteFile | HelperCommand::UploadCommit
    ) {
        return HelperResponse::not_implemented(request);
    }
    if request.uid == 0 {
        return HelperResponse::permission_denied(request, "Helper refuses uid 0.");
    }

    let Some(root) = roots.iter().find(|root| root.root_id == request.root_id) else {
        return helper_error(
            request,
            ErrorKind::InvalidPath,
            "Unknown root id.",
            "invalid_root_id",
        );
    };

    if root.read_only {
        return helper_error(
            request,
            ErrorKind::PolicyDenied,
            "Root is read-only.",
            "read_only_root",
        );
    }

    if request.command == HelperCommand::UploadCommit && !root.uploads_allowed {
        return helper_error(
            request,
            ErrorKind::PolicyDenied,
            "Uploads not allowed in this root.",
            "uploads_disallowed",
        );
    }

    let Some(write_options) = &request.write_options else {
        return helper_error(
            request,
            ErrorKind::ValidationError,
            "Write options required.",
            "missing_write_options",
        );
    };

    let validated_path = match validate_relative_path(&request.relative_path) {
        Ok(path) => path,
        Err(err) => return helper_error_from_tealdrive(request, err),
    };

    if detect_sensitive(&validated_path, sensitive_config).is_sensitive {
        return helper_error(
            request,
            ErrorKind::PolicyDenied,
            "Target path is sensitive.",
            "sensitive_path",
        );
    }

    let (parent_str, filename_str) = match validated_path.0.rsplit_once('/') {
        Some((p, f)) => (p, f),
        None => ("", validated_path.0.as_str()),
    };

    let filename = match SafeFileName::parse(filename_str) {
        Ok(name) => name,
        Err(err) => return helper_error_from_tealdrive(request, err),
    };

    let webroot_policy = crate::helper::policy::current()
        .map(|policy| policy.config.webroot_policy)
        .unwrap_or_default();
    match check_webroot_upload(root.is_web_root, &filename, &webroot_policy) {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(_) => {
            return helper_error(
                request,
                ErrorKind::PolicyDenied,
                "Executable upload denied in webroot.",
                "webroot_executable_denied",
            );
        }
    }

    let parent_rel = RelativePath::new(parent_str);
    let _target = match resolve_new_target(root, &parent_rel, &filename.0) {
        Ok(target) => target,
        Err(err) => return helper_error_from_tealdrive(request, err),
    };

    if crate::fs::quota::check_quota(
        root,
        write_options
            .stream_size
            .unwrap_or(write_options.data.len() as u64),
    )
    .is_err()
    {
        return helper_error(
            request,
            ErrorKind::DiskFull,
            "Storage quota exceeded.",
            "quota_exceeded",
        );
    }
    let mut flags = libc::O_WRONLY | libc::O_CREAT;
    if request.command == HelperCommand::UploadCommit {
        flags |= libc::O_EXCL;
    }
    if write_options.truncate {
        flags |= libc::O_TRUNC;
    }
    let mut file = match crate::helper::sandbox::open(root, &validated_path, flags, 0o600) {
        Ok(file) => file,
        Err(err) => return helper_error_from_io(request, err),
    };

    if write_options.offset > 0 {
        if let Err(err) = file.seek(SeekFrom::Start(write_options.offset)) {
            return helper_error_from_io(request, err);
        }
    }

    let written = if let Some(size) = write_options.stream_size {
        STREAM.with(|slot| {
            let mut slot = slot.borrow_mut();
            let source = slot.as_mut().ok_or(std::io::ErrorKind::PermissionDenied)?;
            source.seek(SeekFrom::Start(0))?;
            let mut remaining = size;
            let mut buffer = [0u8; 64 * 1024];
            while remaining != 0 {
                crate::helper::cancellation::check()?;
                let limit = remaining.min(buffer.len() as u64) as usize;
                let count = source.read(&mut buffer[..limit])?;
                if count == 0 {
                    return Err(std::io::ErrorKind::UnexpectedEof.into());
                }
                file.write_all(&buffer[..count])?;
                remaining -= count as u64;
            }
            Ok(size)
        })
    } else {
        file.write_all(&write_options.data)
            .map(|()| write_options.data.len() as u64)
    };
    let written = match written {
        Ok(size) => size,
        Err(err) => return helper_error_from_io(request, err),
    };

    if let Err(err) = file.flush() {
        return helper_error_from_io(request, err);
    }

    let mut response = HelperResponse::operation_done(request, "Written.");
    response.bytes_written = written;
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{RelativePath, RootId};
    use crate::helper::ipc::tests_support::sample_request;
    use crate::helper::types::HelperWriteOptions;
    use std::fs;
    use std::path::PathBuf;

    fn temp_root() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("tealdrive-helper-upload-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temp root");
        path
    }

    fn root(
        path: PathBuf,
        read_only: bool,
        uploads_allowed: bool,
        is_web_root: bool,
    ) -> AllowedRoot {
        AllowedRoot {
            root_id: RootId::new("home"),
            base_path: path,
            read_only,
            uploads_allowed,
            hidden_files_allowed: false,
            is_web_root,
            quota_bytes: None,
        }
    }

    fn write_request(path: &str, data: Vec<u8>, offset: u64, truncate: bool) -> HelperRequest {
        let mut request = sample_request(HelperCommand::WriteFile);
        request.relative_path = RelativePath::new(path);
        request.target_relative_path = None;
        request.write_options = Some(HelperWriteOptions {
            stream_size: None,
            expected_revision: None,
            data,
            offset,
            truncate,
        });
        request
    }

    #[test]
    fn helper_streams_large_upload_without_a_large_ipc_payload() {
        use std::os::unix::fs::OpenOptionsExt;
        let base = temp_root();
        let roots = vec![root(base.clone(), false, true, false)];
        let source_path = base.join("source.tmp");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&source_path)
            .unwrap();
        let block = [0x5au8; 64 * 1024];
        for _ in 0..272 {
            file.write_all(&block).unwrap();
        }
        drop(file);
        let size = 17 * 1024 * 1024;
        let mut request = write_request("large.bin", Vec::new(), 0, true);
        request.command = HelperCommand::UploadCommit;
        request.write_options.as_mut().unwrap().stream_size = Some(size);
        assert!(
            crate::helper::ipc::encode_helper_request(&request)
                .unwrap()
                .len()
                < 4096
        );
        let response = with_source(Some(std::fs::File::open(&source_path).unwrap()), || {
            write_file_with_roots(&request, &roots, &SensitivePolicyConfig::default())
        });
        assert!(response.success, "{response:?}");
        assert_eq!(response.bytes_written, size);
        assert_eq!(
            std::fs::metadata(base.join("large.bin")).unwrap().len(),
            size
        );
        let mut output = std::fs::File::open(base.join("large.bin")).unwrap();
        let mut read_block = [0u8; 64 * 1024];
        for _ in 0..272 {
            output.read_exact(&mut read_block).unwrap();
            assert_eq!(read_block, block);
        }
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn helper_write_file_creates_file() {
        let base = temp_root();
        let roots = vec![root(base.clone(), false, true, false)];
        let request = write_request("hello.txt", b"world".to_vec(), 0, true);

        let response = write_file_with_roots(&request, &roots, &SensitivePolicyConfig::default());
        assert!(response.success, "{response:?}");
        assert_eq!(response.bytes_written, 5);
        assert_eq!(fs::read(base.join("hello.txt")).expect("read"), b"world");
    }

    #[test]
    fn helper_write_file_appends_or_seeks() {
        let base = temp_root();
        let roots = vec![root(base.clone(), false, true, false)];
        let req1 = write_request("seek.txt", b"AAAA".to_vec(), 0, true);
        let resp1 = write_file_with_roots(&req1, &roots, &SensitivePolicyConfig::default());
        assert!(resp1.success);

        let req2 = write_request("seek.txt", b"BB".to_vec(), 4, false);
        let resp2 = write_file_with_roots(&req2, &roots, &SensitivePolicyConfig::default());
        assert!(resp2.success);

        assert_eq!(fs::read(base.join("seek.txt")).expect("read"), b"AAAABB");
    }

    #[test]
    fn helper_write_file_rejects_uid_zero() {
        let base = temp_root();
        let roots = vec![root(base, false, true, false)];
        let mut request = write_request("test.txt", b"abc".to_vec(), 0, true);
        request.uid = 0;

        let response = write_file_with_roots(&request, &roots, &SensitivePolicyConfig::default());
        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::PermissionDenied));
    }

    #[test]
    fn helper_write_file_rejects_read_only_root() {
        let base = temp_root();
        let roots = vec![root(base, true, true, false)];
        let request = write_request("ro.txt", b"abc".to_vec(), 0, true);

        let response = write_file_with_roots(&request, &roots, &SensitivePolicyConfig::default());
        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::PolicyDenied));
    }

    #[test]
    fn upload_gate_does_not_disable_text_edits() {
        let base = temp_root();
        let roots = vec![root(base.clone(), false, false, false)];
        let mut request = write_request("upload.txt", b"abc".to_vec(), 0, true);
        request.command = HelperCommand::UploadCommit;
        let response = write_file_with_roots(&request, &roots, &SensitivePolicyConfig::default());
        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::PolicyDenied));
        request.command = HelperCommand::WriteFile;
        assert!(write_file_with_roots(&request, &roots, &SensitivePolicyConfig::default()).success);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn helper_write_file_rejects_sensitive_target() {
        let base = temp_root();
        let roots = vec![root(base, false, true, false)];
        let request = write_request(".env", b"SECRET=x".to_vec(), 0, true);

        let response = write_file_with_roots(&request, &roots, &SensitivePolicyConfig::default());
        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::PolicyDenied));
    }

    #[test]
    fn helper_write_file_rejects_webroot_executable() {
        let base = temp_root();
        let roots = vec![root(base, false, true, true)];
        let request = write_request("shell.php", b"<?php phpinfo(); ?>".to_vec(), 0, true);

        let response = write_file_with_roots(&request, &roots, &SensitivePolicyConfig::default());
        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::PolicyDenied));
    }

    #[test]
    fn helper_write_file_does_not_expose_absolute_path_on_error() {
        let base = temp_root();
        let roots = vec![root(base.clone(), false, true, false)];
        let request = write_request("missing_dir/file.txt", b"abc".to_vec(), 0, true);

        let response = write_file_with_roots(&request, &roots, &SensitivePolicyConfig::default());
        assert!(!response.success);
        let debug = format!("{response:?}");
        assert!(!debug.contains(base.to_string_lossy().as_ref()));
    }
}
