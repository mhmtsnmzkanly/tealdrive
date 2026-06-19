use std::fs;
use std::io;

use crate::config::{AllowedRoot, SensitivePolicyConfig};
use crate::errors::ErrorKind;
use crate::fs::path::resolve_new_target;
use crate::helper::listing::{helper_error, helper_error_from_tealdrive};
use crate::helper::types::{HelperCommand, HelperRequest, HelperResponse};

pub fn create_folder_with_roots(
    request: &HelperRequest,
    roots: &[AllowedRoot],
    _sensitive_config: &SensitivePolicyConfig,
) -> HelperResponse {
    if request.command != HelperCommand::CreateFolder {
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
    let Some(folder_name) = request.target_relative_path.as_ref() else {
        return helper_error(
            request,
            ErrorKind::ValidationError,
            "Folder name is required.",
            "missing_folder_name",
        );
    };
    if folder_name.0.contains('/') || folder_name.0.contains('\\') {
        return helper_error(
            request,
            ErrorKind::ValidationError,
            "Invalid folder name.",
            "invalid_folder_name",
        );
    }
    let target = match resolve_new_target(root, &request.relative_path, &folder_name.0) {
        Ok(target) => target,
        Err(error) => return helper_error_from_tealdrive(request, error),
    };

    match fs::create_dir(&target.resolved_path) {
        Ok(()) => HelperResponse::operation_done(request, "Folder created."),
        Err(error) => helper_error_from_io(request, error),
    }
}

fn helper_error_from_io(request: &HelperRequest, error: io::Error) -> HelperResponse {
    let (kind, message, reason) = match error.kind() {
        io::ErrorKind::AlreadyExists => (
            ErrorKind::AlreadyExists,
            "Target already exists.",
            "already_exists",
        ),
        io::ErrorKind::NotFound => (ErrorKind::NotFound, "Parent not found.", "not_found"),
        io::ErrorKind::PermissionDenied => (
            ErrorKind::PermissionDenied,
            "Permission denied.",
            "permission_denied",
        ),
        _ => match error.raw_os_error() {
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
    use crate::config::{RelativePath, RootId};
    use crate::helper::ipc::tests_support::sample_request;
    use std::path::PathBuf;

    fn temp_root() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("tealdrive-helper-create-{}", uuid::Uuid::new_v4()));
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

    fn request(parent: &str, name: &str) -> HelperRequest {
        let mut request = sample_request(HelperCommand::CreateFolder);
        request.relative_path = RelativePath::new(parent);
        request.target_relative_path = Some(RelativePath::new(name));
        request
    }

    #[test]
    fn helper_create_folder_creates_one_directory() {
        let base = temp_root();
        let response = create_folder_with_roots(
            &request("", "new-folder"),
            &[root(base.clone())],
            &SensitivePolicyConfig::default(),
        );

        assert!(response.success);
        assert!(base.join("new-folder").is_dir());
    }

    #[test]
    fn helper_create_folder_rejects_uid_zero() {
        let base = temp_root();
        let mut request = request("", "new-folder");
        request.uid = 0;
        let response =
            create_folder_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());

        assert_eq!(response.error_kind, Some(ErrorKind::PermissionDenied));
    }

    #[test]
    fn helper_create_folder_does_not_create_recursively() {
        let base = temp_root();
        let response = create_folder_with_roots(
            &request("missing", "child"),
            &[root(base)],
            &SensitivePolicyConfig::default(),
        );

        assert_eq!(response.error_kind, Some(ErrorKind::NotFound));
    }

    #[test]
    fn helper_create_folder_rejects_invalid_name() {
        let base = temp_root();
        let response = create_folder_with_roots(
            &request("", "bad/name"),
            &[root(base)],
            &SensitivePolicyConfig::default(),
        );

        assert_eq!(response.error_kind, Some(ErrorKind::ValidationError));
    }

    #[test]
    fn helper_create_folder_returns_already_exists() {
        let base = temp_root();
        fs::create_dir(base.join("existing")).expect("existing");
        let response = create_folder_with_roots(
            &request("", "existing"),
            &[root(base)],
            &SensitivePolicyConfig::default(),
        );

        assert_eq!(response.error_kind, Some(ErrorKind::AlreadyExists));
    }

    #[test]
    fn helper_create_folder_response_does_not_expose_absolute_path() {
        let base = temp_root();
        let response = create_folder_with_roots(
            &request("", "new-folder"),
            &[root(base.clone())],
            &SensitivePolicyConfig::default(),
        );
        let serialized = format!("{response:?}");

        assert!(!serialized.contains(base.to_string_lossy().as_ref()));
    }
}
