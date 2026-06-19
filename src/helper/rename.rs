use std::fs;
use std::io;

use crate::config::{AllowedRoot, RelativePath, SensitivePolicyConfig};
use crate::errors::ErrorKind;
use crate::fs::path::{resolve_existing_path, resolve_new_target};
use crate::helper::listing::{helper_error, helper_error_from_tealdrive};
use crate::helper::types::{HelperCommand, HelperRequest, HelperResponse};

pub fn rename_with_roots(
    request: &HelperRequest,
    roots: &[AllowedRoot],
    _sensitive_config: &SensitivePolicyConfig,
) -> HelperResponse {
    if request.command != HelperCommand::Rename {
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
    let Some(new_name) = request.target_relative_path.as_ref() else {
        return helper_error(
            request,
            ErrorKind::ValidationError,
            "New name is required.",
            "missing_new_name",
        );
    };
    if new_name.0.contains('/') || new_name.0.contains('\\') {
        return helper_error(
            request,
            ErrorKind::ValidationError,
            "Invalid new name.",
            "invalid_new_name",
        );
    }

    let source = match resolve_existing_path(root, &request.relative_path, false) {
        Ok(source) => source,
        Err(error) => return helper_error_from_tealdrive(request, error),
    };
    let parent_relative_path = parent_relative_path(&source.relative_path);
    let target = match resolve_new_target(root, &parent_relative_path, &new_name.0) {
        Ok(target) => target,
        Err(error) => return helper_error_from_tealdrive(request, error),
    };
    if fs::symlink_metadata(&target.resolved_path).is_ok() {
        return helper_error(
            request,
            ErrorKind::AlreadyExists,
            "Target already exists.",
            "already_exists",
        );
    }

    match fs::rename(&source.resolved_path, &target.resolved_path) {
        Ok(()) => HelperResponse::operation_done(request, "Renamed."),
        Err(error) => helper_error_from_io(request, error),
    }
}

fn parent_relative_path(path: &RelativePath) -> RelativePath {
    match path.0.rsplit_once('/') {
        Some((parent, _)) => RelativePath::new(parent),
        None => RelativePath::new(""),
    }
}

fn helper_error_from_io(request: &HelperRequest, error: io::Error) -> HelperResponse {
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
                "Cross-device rename is not supported.",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RootId;
    use crate::helper::ipc::tests_support::sample_request;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    fn temp_root() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("tealdrive-helper-rename-{}", uuid::Uuid::new_v4()));
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

    fn request(path: &str, new_name: &str) -> HelperRequest {
        let mut request = sample_request(HelperCommand::Rename);
        request.relative_path = RelativePath::new(path);
        request.target_relative_path = Some(RelativePath::new(new_name));
        request
    }

    #[test]
    fn helper_rename_renames_one_file() {
        let base = temp_root();
        fs::write(base.join("old.txt"), b"hello").expect("file");
        let response = rename_with_roots(
            &request("old.txt", "new.txt"),
            &[root(base.clone())],
            &SensitivePolicyConfig::default(),
        );

        assert!(response.success);
        assert!(!base.join("old.txt").exists());
        assert!(base.join("new.txt").is_file());
    }

    #[test]
    fn helper_rename_renames_one_directory() {
        let base = temp_root();
        fs::create_dir(base.join("old-dir")).expect("dir");
        let response = rename_with_roots(
            &request("old-dir", "new-dir"),
            &[root(base.clone())],
            &SensitivePolicyConfig::default(),
        );

        assert!(response.success);
        assert!(base.join("new-dir").is_dir());
    }

    #[test]
    fn helper_rename_rejects_uid_zero() {
        let base = temp_root();
        fs::write(base.join("old.txt"), b"hello").expect("file");
        let mut request = request("old.txt", "new.txt");
        request.uid = 0;
        let response =
            rename_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());

        assert_eq!(response.error_kind, Some(ErrorKind::PermissionDenied));
    }

    #[test]
    fn helper_rename_rejects_invalid_new_name() {
        let base = temp_root();
        fs::write(base.join("old.txt"), b"hello").expect("file");
        let response = rename_with_roots(
            &request("old.txt", "bad/name"),
            &[root(base)],
            &SensitivePolicyConfig::default(),
        );

        assert_eq!(response.error_kind, Some(ErrorKind::ValidationError));
    }

    #[test]
    fn helper_rename_rejects_target_exists() {
        let base = temp_root();
        fs::write(base.join("old.txt"), b"hello").expect("file");
        fs::write(base.join("new.txt"), b"existing").expect("file");
        let response = rename_with_roots(
            &request("old.txt", "new.txt"),
            &[root(base)],
            &SensitivePolicyConfig::default(),
        );

        assert_eq!(response.error_kind, Some(ErrorKind::AlreadyExists));
    }

    #[cfg(unix)]
    #[test]
    fn helper_rename_rejects_symlink_source_by_default() {
        let base = temp_root();
        fs::write(base.join("target.txt"), b"hello").expect("file");
        symlink(base.join("target.txt"), base.join("link.txt")).expect("symlink");
        let response = rename_with_roots(
            &request("link.txt", "renamed-link.txt"),
            &[root(base)],
            &SensitivePolicyConfig::default(),
        );

        assert_eq!(response.error_kind, Some(ErrorKind::PolicyDenied));
    }

    #[test]
    fn helper_rename_response_does_not_expose_absolute_path() {
        let base = temp_root();
        fs::write(base.join("old.txt"), b"hello").expect("file");
        let response = rename_with_roots(
            &request("old.txt", "new.txt"),
            &[root(base.clone())],
            &SensitivePolicyConfig::default(),
        );
        let serialized = format!("{response:?}");

        assert!(response.success);
        assert!(!serialized.contains(base.to_string_lossy().as_ref()));
    }
}
