use std::os::unix::fs::PermissionsExt;

use crate::config::{AllowedRoot, SensitivePolicyConfig};
use crate::errors::{ErrorKind, TealDriveError};
use crate::fs::path::{resolve_existing_path, validate_relative_path};
use crate::helper::listing::{helper_error, helper_error_from_io, helper_error_from_tealdrive};
use crate::helper::types::{HelperCommand, HelperRequest, HelperResponse};
use crate::policy::decision::PolicyDecision;
use crate::policy::sensitive::detect_sensitive;
use crate::policy::webroot::check_webroot_chmod_executable;

pub fn chmod_file_with_roots(
    request: &HelperRequest,
    roots: &[AllowedRoot],
    sensitive_config: &SensitivePolicyConfig,
) -> HelperResponse {
    if request.command != HelperCommand::Chmod {
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

    let Some(mode) = request.chmod_mode else {
        return helper_error(
            request,
            ErrorKind::ValidationError,
            "Mode required for chmod.",
            "missing_chmod_mode",
        );
    };

    let validated_path = match validate_relative_path(&request.relative_path) {
        Ok(path) => path,
        Err(err) => return helper_error_from_tealdrive(request, err),
    };

    if validated_path.0.is_empty() {
        return helper_error(
            request,
            ErrorKind::InvalidPath,
            "Cannot chmod root directory.",
            "invalid_target",
        );
    }

    if detect_sensitive(&validated_path, sensitive_config).is_sensitive {
        return helper_error(
            request,
            ErrorKind::PolicyDenied,
            "Target path is sensitive.",
            "sensitive_path",
        );
    }

    match check_webroot_chmod_executable(root.is_web_root, mode.0) {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(_) => {
            return helper_error(
                request,
                ErrorKind::PolicyDenied,
                "Executable permissions denied in webroot.",
                "webroot_executable_denied",
            );
        }
    }

    let _target = match resolve_existing_path(root, &validated_path, false) {
        Ok(target) => target,
        Err(TealDriveError::SymlinkDenied) => {
            return helper_error(
                request,
                ErrorKind::PolicyDenied,
                "Cannot chmod symlinks.",
                "symlink_denied",
            );
        }
        Err(err) => return helper_error_from_tealdrive(request, err),
    };

    if mode.0 & 0o7000 != 0 {
        return helper_error(
            request,
            ErrorKind::PolicyDenied,
            "Special permission bits (SUID/SGID/Sticky) are forbidden.",
            "special_bits_denied",
        );
    }

    let permissions = std::fs::Permissions::from_mode(mode.0 & 0o0777);
    let file=match crate::helper::sandbox::open(root,&validated_path,libc::O_RDONLY,0) { Ok(f)=>f,Err(e)=>return helper_error_from_io(request,e) };
    match file.set_permissions(permissions) {
        Ok(()) => HelperResponse::operation_done(request, "Permissions changed."),
        Err(err) => helper_error_from_io(request, err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{RelativePath, RootId};
    use crate::helper::types::{HelperLimits, HelperPolicyContext};
    use crate::protocol::frame::RequestId;
    use crate::protocol::schema::UnixMode;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    fn temp_root() -> PathBuf {
        let path = std::env::temp_dir().join(format!("tealdrive-chmod-helper-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temp root");
        path
    }

    fn chmod_request(path: &str, mode: Option<u32>) -> HelperRequest {
        HelperRequest {
            request_id: RequestId::new(),
            username: "alice".to_owned(),
            uid: 1000,
            gid: 1000,
            command: HelperCommand::Chmod,
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(path),
            target_relative_path: None,
            policy_context: HelperPolicyContext {
                read_only_root: false,
                is_web_root: false,
            },
            limits: HelperLimits {
                max_directory_page_size: 100,
                max_text_edit_size: 1024,
                max_chunk_size: 1024,
                max_download_helper_bytes: 1024,
            },
            list_options: None,
            write_options: None,
            chmod_mode: mode.map(UnixMode),
            archive_source_paths: None,
            search_options: None,
        }
    }

    #[test]
    fn chmod_regular_file_success() {
        let base = temp_root();
        let file_path = base.join("script.sh");
        fs::write(&file_path, b"echo hello").expect("write");
        fs::set_permissions(&file_path, fs::Permissions::from_mode(0o644)).expect("initial perm");

        let root = AllowedRoot {
            root_id: RootId::new("home"),
            base_path: base.clone(),
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
                quota_bytes: None,
        };

        let request = chmod_request("script.sh", Some(0o755));
        let response = chmod_file_with_roots(&request, &[root], &SensitivePolicyConfig::default());

        assert!(response.success);
        let meta = fs::metadata(&file_path).expect("meta");
        assert_eq!(meta.permissions().mode() & 0o777, 0o755);
    }

    #[test]
    fn chmod_uid_0_refused() {
        let mut request = chmod_request("file.txt", Some(0o644));
        request.uid = 0;
        let response = chmod_file_with_roots(&request, &[], &SensitivePolicyConfig::default());

        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::PermissionDenied));
    }

    #[test]
    fn chmod_read_only_root_denied() {
        let base = temp_root();
        let root = AllowedRoot {
            root_id: RootId::new("home"),
            base_path: base,
            read_only: true,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
                quota_bytes: None,
        };

        let request = chmod_request("file.txt", Some(0o644));
        let response = chmod_file_with_roots(&request, &[root], &SensitivePolicyConfig::default());

        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::PolicyDenied));
    }

    #[test]
    fn chmod_missing_mode_rejected() {
        let base = temp_root();
        let root = AllowedRoot {
            root_id: RootId::new("home"),
            base_path: base,
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
                quota_bytes: None,
        };

        let request = chmod_request("file.txt", None);
        let response = chmod_file_with_roots(&request, &[root], &SensitivePolicyConfig::default());

        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::ValidationError));
    }

    #[test]
    fn chmod_sensitive_file_denied() {
        let base = temp_root();
        fs::write(base.join(".env"), b"SECRET=true").expect("write");
        let root = AllowedRoot {
            root_id: RootId::new("home"),
            base_path: base,
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
                quota_bytes: None,
        };

        let request = chmod_request(".env", Some(0o600));
        let response = chmod_file_with_roots(&request, &[root], &SensitivePolicyConfig::default());

        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::PolicyDenied));
    }

    #[test]
    fn chmod_webroot_executable_denied() {
        let base = temp_root();
        fs::write(base.join("exploit.php"), b"<?php").expect("write");
        let root = AllowedRoot {
            root_id: RootId::new("home"),
            base_path: base,
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: true,
                quota_bytes: None,
        };

        let request = chmod_request("exploit.php", Some(0o755));
        let response = chmod_file_with_roots(&request, &[root], &SensitivePolicyConfig::default());

        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::PolicyDenied));
    }

    #[test]
    fn chmod_webroot_non_executable_allowed() {
        let base = temp_root();
        fs::write(base.join("index.html"), b"<h1>Hi</h1>").expect("write");
        let root = AllowedRoot {
            root_id: RootId::new("home"),
            base_path: base.clone(),
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: true,
                quota_bytes: None,
        };

        let request = chmod_request("index.html", Some(0o644));
        let response = chmod_file_with_roots(&request, &[root], &SensitivePolicyConfig::default());

        assert!(response.success);
        let meta = fs::metadata(base.join("index.html")).expect("meta");
        assert_eq!(meta.permissions().mode() & 0o777, 0o644);
    }

    #[test]
    fn chmod_root_itself_denied() {
        let base = temp_root();
        let root = AllowedRoot {
            root_id: RootId::new("home"),
            base_path: base,
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
                quota_bytes: None,
        };

        let request = chmod_request("", Some(0o755));
        let response = chmod_file_with_roots(&request, &[root], &SensitivePolicyConfig::default());

        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::InvalidPath));
    }

    #[test]
    fn chmod_nonexistent_file_not_found() {
        let base = temp_root();
        let root = AllowedRoot {
            root_id: RootId::new("home"),
            base_path: base,
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
                quota_bytes: None,
        };

        let request = chmod_request("missing.txt", Some(0o644));
        let response = chmod_file_with_roots(&request, &[root], &SensitivePolicyConfig::default());

        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::NotFound));
    }

    #[test]
    fn chmod_suid_sgid_bits_rejected() {
        let base = temp_root();
        let file_path = base.join("target.sh");
        fs::write(&file_path, b"echo test").expect("write");

        let root = AllowedRoot {
            root_id: RootId::new("home"),
            base_path: base,
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
            quota_bytes: None,
        };

        // Try SUID bit 04755
        let request = chmod_request("target.sh", Some(0o4755));
        let response = chmod_file_with_roots(&request, std::slice::from_ref(&root), &SensitivePolicyConfig::default());
        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::PolicyDenied));

        // Try SGID bit 02755
        let request2 = chmod_request("target.sh", Some(0o2755));
        let response2 = chmod_file_with_roots(&request2, &[root], &SensitivePolicyConfig::default());
        assert!(!response2.success);
        assert_eq!(response2.error_kind, Some(ErrorKind::PolicyDenied));
    }
}
