use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};

use crate::config::{AllowedRoot, RelativePath, SensitivePolicyConfig};
use crate::errors::ErrorKind;
use crate::fs::filename::SafeFileName;
use crate::fs::path::{resolve_new_target, validate_relative_path};
use crate::helper::listing::{helper_error, helper_error_from_io, helper_error_from_tealdrive};
use crate::helper::types::{HelperCommand, HelperRequest, HelperResponse};
use crate::policy::decision::PolicyDecision;
use crate::policy::sensitive::detect_sensitive;
use crate::policy::webroot::check_webroot_upload;

pub fn write_file_with_roots(
    request: &HelperRequest,
    roots: &[AllowedRoot],
    sensitive_config: &SensitivePolicyConfig,
) -> HelperResponse {
    if request.command != HelperCommand::WriteFile {
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

    if !root.uploads_allowed {
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

    match check_webroot_upload(root.is_web_root, &filename, &crate::config::WebRootPolicyConfig::default()) {
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
    let target = match resolve_new_target(root, &parent_rel, &filename.0) {
        Ok(target) => target,
        Err(err) => return helper_error_from_tealdrive(request, err),
    };

    let mut open_options = OpenOptions::new();
    open_options.write(true).create(true);
    if write_options.truncate {
        open_options.truncate(true);
    }

    let mut file = match open_options.open(&target.resolved_path) {
        Ok(file) => file,
        Err(err) => return helper_error_from_io(request, err),
    };

    if write_options.offset > 0 {
        if let Err(err) = file.seek(SeekFrom::Start(write_options.offset)) {
            return helper_error_from_io(request, err);
        }
    }

    if let Err(err) = file.write_all(&write_options.data) {
        return helper_error_from_io(request, err);
    }

    if let Err(err) = file.flush() {
        return helper_error_from_io(request, err);
    }

    let mut response = HelperResponse::operation_done(request, "Written.");
    response.bytes_written = write_options.data.len() as u64;
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
        let path = std::env::temp_dir().join(format!("tealdrive-helper-upload-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temp root");
        path
    }

    fn root(path: PathBuf, read_only: bool, uploads_allowed: bool, is_web_root: bool) -> AllowedRoot {
        AllowedRoot {
            root_id: RootId::new("home"),
            base_path: path,
            read_only,
            uploads_allowed,
            hidden_files_allowed: false,
            is_web_root,
        }
    }

    fn write_request(path: &str, data: Vec<u8>, offset: u64, truncate: bool) -> HelperRequest {
        let mut request = sample_request(HelperCommand::WriteFile);
        request.relative_path = RelativePath::new(path);
        request.target_relative_path = None;
        request.write_options = Some(HelperWriteOptions {
            data,
            offset,
            truncate,
        });
        request
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
    fn helper_write_file_rejects_disallowed_uploads() {
        let base = temp_root();
        let roots = vec![root(base, false, false, false)];
        let request = write_request("test.txt", b"abc".to_vec(), 0, true);

        let response = write_file_with_roots(&request, &roots, &SensitivePolicyConfig::default());
        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::PolicyDenied));
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
