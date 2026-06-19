use std::fs;
use std::path::Path;

use crate::config::{AllowedRoot, RelativePath, SensitivePolicyConfig};
use crate::errors::{ErrorKind, TealDriveError};
use crate::fs::path::validate_relative_path;
use crate::helper::listing::{
    directory_entry_from_metadata, helper_error, helper_error_from_io, helper_error_from_tealdrive,
};
use crate::helper::types::{HelperCommand, HelperRequest, HelperResponse};

pub fn file_metadata_with_roots(
    request: &HelperRequest,
    roots: &[AllowedRoot],
    sensitive_config: &SensitivePolicyConfig,
) -> HelperResponse {
    if request.command != HelperCommand::FileMetadata {
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
    let relative_path = match validate_relative_path(&request.relative_path) {
        Ok(path) => path,
        Err(error) => return helper_error_from_tealdrive(request, error),
    };
    let candidate = if relative_path.0.is_empty() {
        root.base_path.clone()
    } else {
        root.base_path.join(&relative_path.0)
    };
    let metadata = match fs::symlink_metadata(&candidate) {
        Ok(metadata) => metadata,
        Err(error) => return helper_error_from_io(request, error),
    };
    if let Err(error) =
        verify_candidate_inside_root(root, &candidate, metadata.file_type().is_symlink())
    {
        return helper_error_from_tealdrive(request, error);
    }

    let name = metadata_name(&candidate, &relative_path);
    let entry = directory_entry_from_metadata(name, &metadata, root, sensitive_config);
    HelperResponse::file_metadata(request, entry)
}

fn verify_candidate_inside_root(
    root: &AllowedRoot,
    candidate: &Path,
    is_symlink: bool,
) -> Result<(), TealDriveError> {
    let canonical_base = root
        .base_path
        .canonicalize()
        .map_err(|_| TealDriveError::InvalidPath)?;
    if is_symlink {
        let Some(parent) = candidate.parent() else {
            return Err(TealDriveError::InvalidPath);
        };
        let canonical_parent = parent
            .canonicalize()
            .map_err(|_| TealDriveError::InvalidPath)?;
        if canonical_parent.starts_with(&canonical_base) {
            Ok(())
        } else {
            Err(TealDriveError::PathEscapesRoot)
        }
    } else {
        let canonical = candidate
            .canonicalize()
            .map_err(|_| TealDriveError::InvalidPath)?;
        if canonical.starts_with(&canonical_base) {
            Ok(())
        } else {
            Err(TealDriveError::PathEscapesRoot)
        }
    }
}

fn metadata_name(candidate: &Path, relative_path: &RelativePath) -> String {
    candidate
        .file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            if relative_path.0.is_empty() {
                ".".to_owned()
            } else {
                relative_path.0.clone()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RootId;
    use crate::helper::ipc::tests_support::sample_request;
    use crate::helper::types::HelperResponsePayload;
    use crate::protocol::schema::FileKind;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    fn temp_root() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("tealdrive-helper-meta-{}", uuid::Uuid::new_v4()));
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

    #[test]
    fn helper_file_metadata_returns_safe_metadata() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"hello").expect("file");
        let mut request = sample_request(HelperCommand::FileMetadata);
        request.relative_path = RelativePath::new("file.txt");
        let response =
            file_metadata_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());

        assert!(response.success);
        let HelperResponsePayload::FileMetadata(entry) = response.payload else {
            panic!("expected metadata");
        };
        assert_eq!(entry.name, "file.txt");
        assert_eq!(entry.file_type, FileKind::File);
        assert_eq!(entry.size, 5);
    }

    #[test]
    fn helper_file_metadata_missing_maps_not_found() {
        let base = temp_root();
        let mut request = sample_request(HelperCommand::FileMetadata);
        request.relative_path = RelativePath::new("missing.txt");
        let response =
            file_metadata_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());

        assert_eq!(response.error_kind, Some(ErrorKind::NotFound));
    }

    #[test]
    fn helper_file_metadata_uid_zero_rejected() {
        let base = temp_root();
        let mut request = sample_request(HelperCommand::FileMetadata);
        request.uid = 0;
        let response =
            file_metadata_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());

        assert_eq!(response.error_kind, Some(ErrorKind::PermissionDenied));
    }

    #[test]
    fn helper_file_metadata_does_not_leak_absolute_path() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"hello").expect("file");
        let mut request = sample_request(HelperCommand::FileMetadata);
        request.relative_path = RelativePath::new("file.txt");
        let response = file_metadata_with_roots(
            &request,
            &[root(base.clone())],
            &SensitivePolicyConfig::default(),
        );
        let serialized = format!("{response:?}");

        assert!(!serialized.contains(base.to_string_lossy().as_ref()));
    }

    #[cfg(unix)]
    #[test]
    fn helper_file_metadata_flags_symlink_without_following() {
        let base = temp_root();
        fs::write(base.join("target"), b"hello").expect("file");
        symlink(base.join("target"), base.join("link")).expect("symlink");
        let mut request = sample_request(HelperCommand::FileMetadata);
        request.relative_path = RelativePath::new("link");
        let response =
            file_metadata_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());
        let HelperResponsePayload::FileMetadata(entry) = response.payload else {
            panic!("expected metadata");
        };

        assert!(entry.is_symlink);
        assert_eq!(entry.file_type, FileKind::Symlink);
        assert!(entry.symlink_target.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn helper_file_metadata_has_unix_mode_and_permissions() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"hello").expect("file");
        let mut request = sample_request(HelperCommand::FileMetadata);
        request.relative_path = RelativePath::new("file.txt");
        let response =
            file_metadata_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());
        let HelperResponsePayload::FileMetadata(entry) = response.payload else {
            panic!("expected metadata");
        };

        assert!(entry.mode.is_some());
        assert!(entry.permissions.is_some());
    }
}
