use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{AllowedRoot, RelativePath, SensitivePolicyConfig};
use crate::errors::ErrorKind;
use crate::fs::filename::SafeFileName;
use crate::fs::path::resolve_existing_path;
use crate::helper::listing::{helper_error, helper_error_from_tealdrive};
use crate::helper::types::{HelperCommand, HelperRequest, HelperResponse, HelperTrashEntry};
use crate::trash::model::TrashEntry;

pub(crate) const TRASH_DIR_NAME: &str = ".tealdrive-trash";

pub fn move_file_to_trash_with_roots(
    request: &HelperRequest,
    roots: &[AllowedRoot],
    _sensitive_config: &SensitivePolicyConfig,
) -> HelperResponse {
    if request.command != HelperCommand::MoveToTrash {
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

    let source = match resolve_existing_path(root, &request.relative_path, false) {
        Ok(s) => s,
        Err(e) => return helper_error_from_tealdrive(request, e),
    };

    let trash_dir = source.root.canonical_base.join(TRASH_DIR_NAME);
    if source.resolved_path.starts_with(&trash_dir) {
        return helper_error(
            request,
            ErrorKind::InvalidTarget,
            "Cannot move trash directory contents.",
            "trash_path_denied",
        );
    }

    let trash_files_dir = trash_dir.join("files");
    let trash_info_dir = trash_dir.join("info");

    if fs::create_dir_all(&trash_files_dir).is_err() {
        return helper_error(
            request,
            ErrorKind::InternalError,
            "Failed to create trash directory.",
            "create_trash_dir_failed",
        );
    }
    if fs::create_dir_all(&trash_info_dir).is_err() {
        return helper_error(
            request,
            ErrorKind::InternalError,
            "Failed to create trash info directory.",
            "create_info_dir_failed",
        );
    }

    let trash_id = uuid::Uuid::new_v4().to_string();
    let trash_file_path = trash_files_dir.join(&trash_id);
    let info_path = trash_info_dir.join(format!("{}.msgpack", trash_id));

    let display_name = source
        .resolved_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| source.relative_path.0.clone());

    let deleted_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let entry = TrashEntry {
        trash_id: trash_id.clone(),
        original_root_id: root.root_id.clone(),
        original_relative_path: source.relative_path.clone(),
        trash_relative_path: RelativePath::new(format!("files/{}", trash_id)),
        display_name: display_name.clone(),
        deleted_at,
        username: request.username.clone(),
        uid: request.uid,
    };

    let entry_bytes = match rmp_serde::to_vec_named(&entry) {
        Ok(b) => b,
        Err(_) => {
            return helper_error(
                request,
                ErrorKind::InternalError,
                "Failed to serialize trash metadata.",
                "serialize_failed",
            )
        }
    };

    if fs::write(&info_path, &entry_bytes).is_err() {
        return helper_error(
            request,
            ErrorKind::InternalError,
            "Failed to write trash metadata.",
            "write_info_failed",
        );
    }

    if let Err(e) = fs::rename(&source.resolved_path, &trash_file_path) {
        let _ = fs::remove_file(&info_path);
        return helper_error_from_trash_io(request, e);
    }

    HelperResponse::trash_entry(
        request,
        HelperTrashEntry {
            trash_id,
            display_name,
            deleted_at,
            original_relative_path: entry.original_relative_path,
            original_root_id: entry.original_root_id,
        },
    )
}

pub fn restore_file_from_trash_with_roots(
    request: &HelperRequest,
    roots: &[AllowedRoot],
    _sensitive_config: &SensitivePolicyConfig,
) -> HelperResponse {
    if request.command != HelperCommand::RestoreFromTrash {
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

    let trash_id = match SafeFileName::parse(&request.relative_path.0) {
        Ok(name) => name.0,
        Err(_) => {
            return helper_error(
                request,
                ErrorKind::ValidationError,
                "Invalid trash id.",
                "invalid_trash_id",
            )
        }
    };

    let canonical_base = match root.base_path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            return helper_error(
                request,
                ErrorKind::InvalidPath,
                "Cannot access root.",
                "root_canonicalize_failed",
            )
        }
    };

    let trash_dir = canonical_base.join(TRASH_DIR_NAME);
    let trash_files_dir = trash_dir.join("files");
    let trash_info_dir = trash_dir.join("info");

    let info_path = trash_info_dir.join(format!("{}.msgpack", trash_id));
    let info_bytes = match fs::read(&info_path) {
        Ok(b) => b,
        Err(_) => {
            return helper_error(
                request,
                ErrorKind::NotFound,
                "Trash entry not found.",
                "not_found",
            )
        }
    };

    let entry: TrashEntry = match rmp_serde::from_slice(&info_bytes) {
        Ok(e) => e,
        Err(_) => {
            return helper_error(
                request,
                ErrorKind::InternalError,
                "Corrupt trash metadata.",
                "corrupt_metadata",
            )
        }
    };

    let trash_file_path = trash_files_dir.join(&trash_id);
    if fs::symlink_metadata(&trash_file_path).is_err() {
        return helper_error(
            request,
            ErrorKind::NotFound,
            "Trash file not found.",
            "trash_file_missing",
        );
    }

    let original_root = roots.iter().find(|r| r.root_id == entry.original_root_id);
    let original_canonical = match original_root {
        Some(r) => match r.base_path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                return helper_error(
                    request,
                    ErrorKind::InvalidPath,
                    "Cannot access original root.",
                    "orig_root_error",
                )
            }
        },
        None => {
            return helper_error(
                request,
                ErrorKind::InvalidPath,
                "Original root not found.",
                "original_root_missing",
            )
        }
    };

    let original_path = original_canonical.join(&entry.original_relative_path.0);
    if !original_path.starts_with(&original_canonical) {
        return helper_error(
            request,
            ErrorKind::InvalidPath,
            "Original path escapes root.",
            "original_path_escapes_root",
        );
    }

    if fs::symlink_metadata(&original_path).is_ok() {
        return helper_error(
            request,
            ErrorKind::AlreadyExists,
            "Original location already exists.",
            "already_exists",
        );
    }

    match fs::rename(&trash_file_path, &original_path) {
        Ok(()) => {}
        Err(e) => return helper_error_from_trash_io(request, e),
    }

    let _ = fs::remove_file(&info_path);

    HelperResponse::operation_done(request, "Restored.")
}

fn helper_error_from_trash_io(request: &HelperRequest, error: std::io::Error) -> HelperResponse {
    let (kind, message, reason) = match error.kind() {
        std::io::ErrorKind::NotFound => (ErrorKind::NotFound, "Source not found.", "not_found"),
        std::io::ErrorKind::PermissionDenied => (
            ErrorKind::PermissionDenied,
            "Permission denied.",
            "permission_denied",
        ),
        std::io::ErrorKind::AlreadyExists => (
            ErrorKind::AlreadyExists,
            "Target already exists.",
            "already_exists",
        ),
        _ => match error.raw_os_error() {
            Some(28) => (ErrorKind::DiskFull, "Disk is full.", "disk_full"),
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
    use crate::errors::ErrorKind;
    use crate::helper::ipc::tests_support::sample_request;
    use crate::helper::types::HelperResponsePayload;
    use std::path::PathBuf;

    fn temp_root() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("tealdrive-helper-trash-{}", uuid::Uuid::new_v4()));
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

    fn trash_request(src: &str) -> HelperRequest {
        let mut request = sample_request(HelperCommand::MoveToTrash);
        request.relative_path = RelativePath::new(src);
        request.target_relative_path = None;
        request
    }

    fn restore_request(trash_id: &str) -> HelperRequest {
        let mut request = sample_request(HelperCommand::RestoreFromTrash);
        request.relative_path = RelativePath::new(trash_id);
        request.target_relative_path = None;
        request
    }

    #[test]
    fn helper_trash_moves_file() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"hello").expect("file");
        let roots = vec![root(base.clone())];
        let response = move_file_to_trash_with_roots(
            &trash_request("file.txt"),
            &roots,
            &SensitivePolicyConfig::default(),
        );

        assert!(response.success, "{response:?}");
        assert!(!base.join("file.txt").exists());
        let HelperResponsePayload::TrashEntry(ref entry) = response.payload else {
            panic!("expected TrashEntry payload");
        };
        assert_eq!(entry.display_name, "file.txt");
        assert!(!entry.trash_id.is_empty());
    }

    #[test]
    fn helper_trash_moves_directory() {
        let base = temp_root();
        fs::create_dir(base.join("mydir")).expect("dir");
        fs::write(base.join("mydir/inner.txt"), b"hi").expect("inner");
        let roots = vec![root(base.clone())];
        let response = move_file_to_trash_with_roots(
            &trash_request("mydir"),
            &roots,
            &SensitivePolicyConfig::default(),
        );

        assert!(response.success, "{response:?}");
        assert!(!base.join("mydir").exists());
    }

    #[test]
    fn helper_trash_creates_metadata() {
        let base = temp_root();
        fs::write(base.join("report.txt"), b"data").expect("file");
        let roots = vec![root(base.clone())];
        let response = move_file_to_trash_with_roots(
            &trash_request("report.txt"),
            &roots,
            &SensitivePolicyConfig::default(),
        );

        assert!(response.success);
        let HelperResponsePayload::TrashEntry(ref entry) = response.payload else {
            panic!("no trash entry");
        };
        let info_path = base
            .join(TRASH_DIR_NAME)
            .join("info")
            .join(format!("{}.msgpack", entry.trash_id));
        assert!(info_path.exists(), "info file must exist");
        let bytes = fs::read(&info_path).expect("read info");
        let stored: TrashEntry = rmp_serde::from_slice(&bytes).expect("decode");
        assert_eq!(stored.display_name, "report.txt");
        assert_eq!(
            stored.original_relative_path,
            RelativePath::new("report.txt")
        );
    }

    #[test]
    fn helper_restore_restores_file() {
        let base = temp_root();
        fs::write(base.join("orig.txt"), b"content").expect("file");
        let roots = vec![root(base.clone())];

        let trash_resp = move_file_to_trash_with_roots(
            &trash_request("orig.txt"),
            &roots,
            &SensitivePolicyConfig::default(),
        );
        assert!(trash_resp.success);
        let HelperResponsePayload::TrashEntry(ref te) = trash_resp.payload else {
            panic!("no trash entry");
        };
        let trash_id = te.trash_id.clone();

        let restore_resp = restore_file_from_trash_with_roots(
            &restore_request(&trash_id),
            &roots,
            &SensitivePolicyConfig::default(),
        );

        assert!(restore_resp.success, "{restore_resp:?}");
        assert!(base.join("orig.txt").exists());
        let info_path = base
            .join(TRASH_DIR_NAME)
            .join("info")
            .join(format!("{}.msgpack", trash_id));
        assert!(!info_path.exists(), "info file must be cleaned up");
    }

    #[test]
    fn helper_restore_restores_directory() {
        let base = temp_root();
        fs::create_dir(base.join("docs")).expect("dir");
        fs::write(base.join("docs/note.txt"), b"hi").expect("inner");
        let roots = vec![root(base.clone())];

        let trash_resp = move_file_to_trash_with_roots(
            &trash_request("docs"),
            &roots,
            &SensitivePolicyConfig::default(),
        );
        assert!(trash_resp.success);
        let HelperResponsePayload::TrashEntry(ref te) = trash_resp.payload else {
            panic!("no trash entry");
        };
        let trash_id = te.trash_id.clone();

        let restore_resp = restore_file_from_trash_with_roots(
            &restore_request(&trash_id),
            &roots,
            &SensitivePolicyConfig::default(),
        );
        assert!(restore_resp.success);
        assert!(base.join("docs/note.txt").exists());
    }

    #[test]
    fn helper_trash_consecutive_files_have_unique_ids() {
        let base = temp_root();
        fs::write(base.join("a.txt"), b"a").expect("a");
        fs::write(base.join("b.txt"), b"b").expect("b");
        let roots = vec![root(base.clone())];

        let r1 = move_file_to_trash_with_roots(
            &trash_request("a.txt"),
            &roots,
            &SensitivePolicyConfig::default(),
        );
        let r2 = move_file_to_trash_with_roots(
            &trash_request("b.txt"),
            &roots,
            &SensitivePolicyConfig::default(),
        );

        assert!(r1.success);
        assert!(r2.success);
        let HelperResponsePayload::TrashEntry(ref e1) = r1.payload else {
            panic!()
        };
        let HelperResponsePayload::TrashEntry(ref e2) = r2.payload else {
            panic!()
        };
        assert_ne!(e1.trash_id, e2.trash_id);
    }

    #[test]
    fn helper_restore_target_exists_returns_already_exists() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"original").expect("file");
        let roots = vec![root(base.clone())];

        let trash_resp = move_file_to_trash_with_roots(
            &trash_request("file.txt"),
            &roots,
            &SensitivePolicyConfig::default(),
        );
        assert!(trash_resp.success);
        let HelperResponsePayload::TrashEntry(ref te) = trash_resp.payload else {
            panic!()
        };
        let trash_id = te.trash_id.clone();

        // Re-create original file
        fs::write(base.join("file.txt"), b"new").expect("re-create");

        let restore_resp = restore_file_from_trash_with_roots(
            &restore_request(&trash_id),
            &roots,
            &SensitivePolicyConfig::default(),
        );

        assert!(!restore_resp.success);
        assert_eq!(restore_resp.error_kind, Some(ErrorKind::AlreadyExists));
    }

    #[test]
    fn helper_trash_missing_source_returns_not_found() {
        let base = temp_root();
        let roots = vec![root(base)];
        let response = move_file_to_trash_with_roots(
            &trash_request("missing.txt"),
            &roots,
            &SensitivePolicyConfig::default(),
        );

        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::NotFound));
    }

    #[test]
    fn helper_restore_missing_entry_returns_not_found() {
        let base = temp_root();
        let roots = vec![root(base)];
        let response = restore_file_from_trash_with_roots(
            &restore_request("00000000-0000-0000-0000-000000000000"),
            &roots,
            &SensitivePolicyConfig::default(),
        );

        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::NotFound));
    }

    #[test]
    fn helper_trash_rejects_uid_zero() {
        let base = temp_root();
        let roots = vec![root(base)];
        let mut request = trash_request("file.txt");
        request.uid = 0;
        let response =
            move_file_to_trash_with_roots(&request, &roots, &SensitivePolicyConfig::default());

        assert_eq!(response.error_kind, Some(ErrorKind::PermissionDenied));
    }

    #[test]
    fn helper_trash_response_does_not_expose_absolute_path() {
        let base = temp_root();
        fs::write(base.join("secret.txt"), b"data").expect("file");
        let roots = vec![root(base.clone())];
        let response = move_file_to_trash_with_roots(
            &trash_request("secret.txt"),
            &roots,
            &SensitivePolicyConfig::default(),
        );

        assert!(response.success);
        let debug_str = format!("{response:?}");
        assert!(!debug_str.contains(base.to_string_lossy().as_ref()));
    }
}
