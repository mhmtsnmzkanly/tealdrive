use std::fs;
use std::io;

use crate::config::{AllowedRoot, SensitivePolicyConfig};
use crate::errors::ErrorKind;
use crate::fs::path::{resolve_existing_path, validate_relative_path};
use crate::helper::listing::{helper_error, helper_error_from_tealdrive};
use crate::helper::types::{HelperCommand, HelperRequest, HelperResponse};

pub fn move_file_with_roots(
    request: &HelperRequest,
    roots: &[AllowedRoot],
    _sensitive_config: &SensitivePolicyConfig,
) -> HelperResponse {
    if request.command != HelperCommand::Move {
        return HelperResponse::not_implemented(request);
    }
    if request.uid == 0 {
        return HelperResponse::permission_denied(request, "Helper refuses uid 0.");
    }

    let Some(root) = roots.iter().find(|r| r.root_id == request.root_id) else {
        return helper_error(
            request,
            ErrorKind::InvalidPath,
            "Unknown root id.",
            "invalid_root_id",
        );
    };
    let Some(target_rel) = request.target_relative_path.as_ref() else {
        return helper_error(
            request,
            ErrorKind::ValidationError,
            "Target path is required.",
            "missing_target",
        );
    };

    let target_validated = match validate_relative_path(target_rel) {
        Ok(p) => p,
        Err(e) => return helper_error_from_tealdrive(request, e),
    };
    if target_validated.0.is_empty() {
        return helper_error(
            request,
            ErrorKind::InvalidTarget,
            "Target path cannot be empty.",
            "empty_target",
        );
    }

    let source = match resolve_existing_path(root, &request.relative_path, false) {
        Ok(s) => s,
        Err(e) => return helper_error_from_tealdrive(request, e),
    };

    let target_path = source.root.canonical_base.join(&target_validated.0);
    if !target_path.starts_with(&source.root.canonical_base) {
        return helper_error(
            request,
            ErrorKind::InvalidPath,
            "Target escapes root.",
            "target_escapes_root",
        );
    }

    if fs::symlink_metadata(&target_path).is_ok() {
        return helper_error(
            request,
            ErrorKind::AlreadyExists,
            "Target already exists.",
            "already_exists",
        );
    }

    match fs::rename(&source.resolved_path, &target_path) {
        Ok(()) => HelperResponse::operation_done(request, "Moved."),
        Err(e) => helper_error_from_move_io(request, e),
    }
}

pub fn copy_file_with_roots(
    request: &HelperRequest,
    roots: &[AllowedRoot],
    _sensitive_config: &SensitivePolicyConfig,
) -> HelperResponse {
    if request.command != HelperCommand::Copy {
        return HelperResponse::not_implemented(request);
    }
    if request.uid == 0 {
        return HelperResponse::permission_denied(request, "Helper refuses uid 0.");
    }

    let Some(root) = roots.iter().find(|r| r.root_id == request.root_id) else {
        return helper_error(
            request,
            ErrorKind::InvalidPath,
            "Unknown root id.",
            "invalid_root_id",
        );
    };
    let Some(target_rel) = request.target_relative_path.as_ref() else {
        return helper_error(
            request,
            ErrorKind::ValidationError,
            "Target path is required.",
            "missing_target",
        );
    };

    let target_validated = match validate_relative_path(target_rel) {
        Ok(p) => p,
        Err(e) => return helper_error_from_tealdrive(request, e),
    };
    if target_validated.0.is_empty() {
        return helper_error(
            request,
            ErrorKind::InvalidTarget,
            "Target path cannot be empty.",
            "empty_target",
        );
    }

    let source = match resolve_existing_path(root, &request.relative_path, false) {
        Ok(s) => s,
        Err(e) => return helper_error_from_tealdrive(request, e),
    };

    if source.resolved_path.is_dir() {
        return helper_error(
            request,
            ErrorKind::InvalidTarget,
            "Directories cannot be copied.",
            "directory_copy_unsupported",
        );
    }

    let target_path = source.root.canonical_base.join(&target_validated.0);
    if !target_path.starts_with(&source.root.canonical_base) {
        return helper_error(
            request,
            ErrorKind::InvalidPath,
            "Target escapes root.",
            "target_escapes_root",
        );
    }

    if fs::symlink_metadata(&target_path).is_ok() {
        return helper_error(
            request,
            ErrorKind::AlreadyExists,
            "Target already exists.",
            "already_exists",
        );
    }

    match fs::copy(&source.resolved_path, &target_path) {
        Ok(_) => HelperResponse::operation_done(request, "Copied."),
        Err(e) => helper_error_from_copy_io(request, e),
    }
}

fn helper_error_from_move_io(request: &HelperRequest, error: io::Error) -> HelperResponse {
    let (kind, message, reason) = match error.kind() {
        io::ErrorKind::AlreadyExists => (
            ErrorKind::AlreadyExists,
            "Target already exists.",
            "already_exists",
        ),
        io::ErrorKind::NotFound => (ErrorKind::NotFound, "Source not found.", "not_found"),
        io::ErrorKind::PermissionDenied => (
            ErrorKind::PermissionDenied,
            "Permission denied.",
            "permission_denied",
        ),
        _ => match error.raw_os_error() {
            Some(18) => (
                ErrorKind::CrossDeviceMove,
                "Cross-device move is not supported.",
                "cross_device_move",
            ),
            Some(20) => (
                ErrorKind::InvalidTarget,
                "Parent is not a directory.",
                "parent_not_directory",
            ),
            Some(28) => (ErrorKind::DiskFull, "Disk is full.", "disk_full"),
            Some(36) => (ErrorKind::NameTooLong, "Name too long.", "name_too_long"),
            _ => (
                ErrorKind::InternalError,
                "Filesystem error.",
                "filesystem_error",
            ),
        },
    };
    helper_error(request, kind, message, reason)
}

fn helper_error_from_copy_io(request: &HelperRequest, error: io::Error) -> HelperResponse {
    let (kind, message, reason) = match error.kind() {
        io::ErrorKind::AlreadyExists => (
            ErrorKind::AlreadyExists,
            "Target already exists.",
            "already_exists",
        ),
        io::ErrorKind::NotFound => (ErrorKind::NotFound, "Source not found.", "not_found"),
        io::ErrorKind::PermissionDenied => (
            ErrorKind::PermissionDenied,
            "Permission denied.",
            "permission_denied",
        ),
        _ => match error.raw_os_error() {
            Some(28) => (ErrorKind::DiskFull, "Disk is full.", "disk_full"),
            Some(36) => (ErrorKind::NameTooLong, "Name too long.", "name_too_long"),
            _ => (
                ErrorKind::InternalError,
                "Filesystem error.",
                "filesystem_error",
            ),
        },
    };
    helper_error(request, kind, message, reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{RelativePath, RootId};
    use crate::helper::ipc::tests_support::sample_request;
    use std::path::PathBuf;

    fn temp_root() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("tealdrive-helper-move-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temp root");
        path
    }

    fn root(path: PathBuf) -> AllowedRoot {
        AllowedRoot {
            root_id: RootId::new("home"),
            base_path: path,
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
        }
    }

    fn move_request(src: &str, dst: &str) -> HelperRequest {
        let mut request = sample_request(HelperCommand::Move);
        request.relative_path = RelativePath::new(src);
        request.target_relative_path = Some(RelativePath::new(dst));
        request
    }

    fn copy_request(src: &str, dst: &str) -> HelperRequest {
        let mut request = sample_request(HelperCommand::Copy);
        request.relative_path = RelativePath::new(src);
        request.target_relative_path = Some(RelativePath::new(dst));
        request
    }

    #[test]
    fn helper_move_moves_one_file() {
        let base = temp_root();
        fs::write(base.join("old.txt"), b"hello").expect("file");
        let response = move_file_with_roots(
            &move_request("old.txt", "new.txt"),
            &[root(base.clone())],
            &SensitivePolicyConfig::default(),
        );

        assert!(response.success);
        assert!(!base.join("old.txt").exists());
        assert!(base.join("new.txt").is_file());
    }

    #[test]
    fn helper_move_moves_into_subdirectory() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"hello").expect("file");
        fs::create_dir(base.join("archive")).expect("dir");
        let response = move_file_with_roots(
            &move_request("file.txt", "archive/file.txt"),
            &[root(base.clone())],
            &SensitivePolicyConfig::default(),
        );

        assert!(response.success);
        assert!(!base.join("file.txt").exists());
        assert!(base.join("archive/file.txt").is_file());
    }

    #[test]
    fn helper_move_moves_directory() {
        let base = temp_root();
        fs::create_dir(base.join("old-dir")).expect("dir");
        let response = move_file_with_roots(
            &move_request("old-dir", "new-dir"),
            &[root(base.clone())],
            &SensitivePolicyConfig::default(),
        );

        assert!(response.success);
        assert!(!base.join("old-dir").exists());
        assert!(base.join("new-dir").is_dir());
    }

    #[test]
    fn helper_move_rejects_uid_zero() {
        let base = temp_root();
        let mut request = move_request("old.txt", "new.txt");
        request.uid = 0;
        let response =
            move_file_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());

        assert_eq!(response.error_kind, Some(ErrorKind::PermissionDenied));
    }

    #[test]
    fn helper_move_target_exists() {
        let base = temp_root();
        fs::write(base.join("old.txt"), b"hello").expect("file");
        fs::write(base.join("new.txt"), b"existing").expect("file");
        let response = move_file_with_roots(
            &move_request("old.txt", "new.txt"),
            &[root(base)],
            &SensitivePolicyConfig::default(),
        );

        assert_eq!(response.error_kind, Some(ErrorKind::AlreadyExists));
    }

    #[test]
    fn helper_move_source_not_found() {
        let base = temp_root();
        let response = move_file_with_roots(
            &move_request("missing.txt", "new.txt"),
            &[root(base)],
            &SensitivePolicyConfig::default(),
        );

        assert_eq!(response.error_kind, Some(ErrorKind::NotFound));
    }

    #[test]
    fn helper_move_response_does_not_expose_absolute_path() {
        let base = temp_root();
        fs::write(base.join("old.txt"), b"hello").expect("file");
        let response = move_file_with_roots(
            &move_request("old.txt", "new.txt"),
            &[root(base.clone())],
            &SensitivePolicyConfig::default(),
        );
        let serialized = format!("{response:?}");

        assert!(response.success);
        assert!(!serialized.contains(base.to_string_lossy().as_ref()));
    }

    #[test]
    fn helper_copy_copies_one_file() {
        let base = temp_root();
        fs::write(base.join("orig.txt"), b"hello").expect("file");
        let response = copy_file_with_roots(
            &copy_request("orig.txt", "copy.txt"),
            &[root(base.clone())],
            &SensitivePolicyConfig::default(),
        );

        assert!(response.success);
        assert!(base.join("orig.txt").is_file());
        assert!(base.join("copy.txt").is_file());
    }

    #[test]
    fn helper_copy_directory_rejected() {
        let base = temp_root();
        fs::create_dir(base.join("mydir")).expect("dir");
        let response = copy_file_with_roots(
            &copy_request("mydir", "mydir-copy"),
            &[root(base)],
            &SensitivePolicyConfig::default(),
        );

        assert_eq!(response.error_kind, Some(ErrorKind::InvalidTarget));
    }

    #[test]
    fn helper_copy_rejects_uid_zero() {
        let base = temp_root();
        let mut request = copy_request("orig.txt", "copy.txt");
        request.uid = 0;
        let response =
            copy_file_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());

        assert_eq!(response.error_kind, Some(ErrorKind::PermissionDenied));
    }

    #[test]
    fn helper_copy_target_exists() {
        let base = temp_root();
        fs::write(base.join("orig.txt"), b"hello").expect("file");
        fs::write(base.join("copy.txt"), b"existing").expect("file");
        let response = copy_file_with_roots(
            &copy_request("orig.txt", "copy.txt"),
            &[root(base)],
            &SensitivePolicyConfig::default(),
        );

        assert_eq!(response.error_kind, Some(ErrorKind::AlreadyExists));
    }

    #[test]
    fn helper_copy_source_not_found() {
        let base = temp_root();
        let response = copy_file_with_roots(
            &copy_request("missing.txt", "copy.txt"),
            &[root(base)],
            &SensitivePolicyConfig::default(),
        );

        assert_eq!(response.error_kind, Some(ErrorKind::NotFound));
    }

    #[test]
    fn helper_copy_response_does_not_expose_absolute_path() {
        let base = temp_root();
        fs::write(base.join("orig.txt"), b"hello").expect("file");
        let response = copy_file_with_roots(
            &copy_request("orig.txt", "copy.txt"),
            &[root(base.clone())],
            &SensitivePolicyConfig::default(),
        );
        let serialized = format!("{response:?}");

        assert!(response.success);
        assert!(!serialized.contains(base.to_string_lossy().as_ref()));
    }
}
