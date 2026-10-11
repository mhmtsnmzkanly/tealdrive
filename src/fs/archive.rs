use crate::config::{AppConfig, UserContext};
use crate::errors::{ErrorKind, PolicyReason, TealDriveError};
use crate::fs::path::validate_relative_path;
use crate::helper::client::HelperProcessClient;
use crate::helper::types::{
    HelperCommand, HelperLimits, HelperPolicyContext, HelperRequest, HelperResponse,
};
use crate::policy::decision::{FileOperation, PolicyAuditMetadata, PolicyDecision, PolicyEngine};
use crate::policy::roots::require_allowed_root;
use crate::protocol::frame::RequestId;
use crate::protocol::payload::{
    CompressTarGzRequest, CompressZipRequest, ExtractArchiveRequest, OperationDonePayload,
};

pub trait CompressZipHelper: Send + Sync {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError>;
}

pub trait CompressTarGzHelper: Send + Sync {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError>;
}

pub trait ExtractArchiveHelper: Send + Sync {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError>;
}

impl CompressZipHelper for HelperProcessClient {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        self.execute(request)
    }
}

impl CompressTarGzHelper for HelperProcessClient {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        self.execute(request)
    }
}

impl ExtractArchiveHelper for HelperProcessClient {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        self.execute(request)
    }
}

pub fn compress_zip_request<H: CompressZipHelper>(
    user_context: &UserContext,
    request: CompressZipRequest,
    config: &AppConfig,
    policy_engine: &PolicyEngine<'_>,
    helper_client: &H,
) -> Result<OperationDonePayload, TealDriveError> {
    if request.relative_paths.is_empty() {
        return Err(TealDriveError::InvalidPath);
    }
    let validated_target = validate_relative_path(&request.target_relative_path)?;
    if validated_target.0.is_empty() {
        return Err(TealDriveError::InvalidTarget);
    }
    if !validated_target.0.to_ascii_lowercase().ends_with(".zip") {
        return Err(TealDriveError::InvalidFilename);
    }

    let root = require_allowed_root(config.allowed_roots(), &request.root_id)?;
    if root.read_only {
        return Err(TealDriveError::ReadOnlyRoot);
    }

    let mut validated_sources = Vec::with_capacity(request.relative_paths.len());
    for src in &request.relative_paths {
        let val_src = validate_relative_path(src)?;
        if val_src.0.is_empty() {
            return Err(TealDriveError::InvalidPath);
        }

        match policy_engine.check_operation(
            FileOperation::CompressZip,
            &request.root_id,
            &val_src,
            Some(&validated_target),
            None,
            root.hidden_files_allowed,
        )? {
            PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
            PolicyDecision::Deny(PolicyReason::FeatureDisabled) => {
                return Err(TealDriveError::FeatureDisabled);
            }
            PolicyDecision::Deny(PolicyReason::ReadOnlyRoot) => {
                return Err(TealDriveError::ReadOnlyRoot);
            }
            PolicyDecision::Deny(reason) => {
                let _audit = PolicyAuditMetadata {
                    action: FileOperation::CompressZip,
                    root_id: request.root_id.clone(),
                    relative_path: val_src,
                    target_relative_path: Some(validated_target),
                    reason,
                    safe_message: "CompressZip denied by TealDrive policy.".to_owned(),
                };
                return Err(TealDriveError::PolicyDenied);
            }
        }
        validated_sources.push(val_src);
    }

    let helper_request = HelperRequest {
        request_id: RequestId::new(),
        username: user_context.username.clone(),
        uid: user_context.uid,
        gid: user_context.gid,
        command: HelperCommand::CompressZip,
        root_id: request.root_id.clone(),
        relative_path: validated_target.clone(),
        target_relative_path: Some(validated_target),
        policy_context: HelperPolicyContext {
            read_only_root: root.read_only,
            is_web_root: root.is_web_root,
        },
        limits: HelperLimits {
            max_directory_page_size: config.limits.max_directory_page_size,
            max_text_edit_size: config.limits.max_text_edit_size,
            max_chunk_size: config.limits.max_chunk_size,
            max_download_helper_bytes: crate::download::transfer::MAX_DOWNLOAD_HELPER_BYTES,
        },
        list_options: None,
        write_options: None,
        chmod_mode: None,
        archive_source_paths: Some(validated_sources),
        search_options: None,
    };

    let response = helper_client.execute_helper(&helper_request)?;
    if !response.success {
        return Err(helper_response_to_error(&response));
    }

    Ok(OperationDonePayload {
        request_id: helper_request.request_id,
        operation: "COMPRESS_ZIP".to_owned(),
        safe_message: Some("Zip archive created.".to_owned()),
    })
}

pub fn compress_targz_request<H: CompressTarGzHelper>(
    user_context: &UserContext,
    request: CompressTarGzRequest,
    config: &AppConfig,
    policy_engine: &PolicyEngine<'_>,
    helper_client: &H,
) -> Result<OperationDonePayload, TealDriveError> {
    if request.relative_paths.is_empty() {
        return Err(TealDriveError::InvalidPath);
    }
    let validated_target = validate_relative_path(&request.target_relative_path)?;
    if validated_target.0.is_empty() {
        return Err(TealDriveError::InvalidTarget);
    }
    let lower_target = validated_target.0.to_ascii_lowercase();
    if !lower_target.ends_with(".tar.gz") && !lower_target.ends_with(".tgz") {
        return Err(TealDriveError::InvalidFilename);
    }

    let root = require_allowed_root(config.allowed_roots(), &request.root_id)?;
    if root.read_only {
        return Err(TealDriveError::ReadOnlyRoot);
    }

    let mut validated_sources = Vec::with_capacity(request.relative_paths.len());
    for src in &request.relative_paths {
        let val_src = validate_relative_path(src)?;
        if val_src.0.is_empty() {
            return Err(TealDriveError::InvalidPath);
        }

        match policy_engine.check_operation(
            FileOperation::CompressTarGz,
            &request.root_id,
            &val_src,
            Some(&validated_target),
            None,
            root.hidden_files_allowed,
        )? {
            PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
            PolicyDecision::Deny(PolicyReason::FeatureDisabled) => {
                return Err(TealDriveError::FeatureDisabled);
            }
            PolicyDecision::Deny(PolicyReason::ReadOnlyRoot) => {
                return Err(TealDriveError::ReadOnlyRoot);
            }
            PolicyDecision::Deny(reason) => {
                let _audit = PolicyAuditMetadata {
                    action: FileOperation::CompressTarGz,
                    root_id: request.root_id.clone(),
                    relative_path: val_src,
                    target_relative_path: Some(validated_target),
                    reason,
                    safe_message: "CompressTarGz denied by TealDrive policy.".to_owned(),
                };
                return Err(TealDriveError::PolicyDenied);
            }
        }
        validated_sources.push(val_src);
    }

    let helper_request = HelperRequest {
        request_id: RequestId::new(),
        username: user_context.username.clone(),
        uid: user_context.uid,
        gid: user_context.gid,
        command: HelperCommand::CompressTarGz,
        root_id: request.root_id.clone(),
        relative_path: validated_target.clone(),
        target_relative_path: Some(validated_target),
        policy_context: HelperPolicyContext {
            read_only_root: root.read_only,
            is_web_root: root.is_web_root,
        },
        limits: HelperLimits {
            max_directory_page_size: config.limits.max_directory_page_size,
            max_text_edit_size: config.limits.max_text_edit_size,
            max_chunk_size: config.limits.max_chunk_size,
            max_download_helper_bytes: crate::download::transfer::MAX_DOWNLOAD_HELPER_BYTES,
        },
        list_options: None,
        write_options: None,
        chmod_mode: None,
        archive_source_paths: Some(validated_sources),
        search_options: None,
    };

    let response = helper_client.execute_helper(&helper_request)?;
    if !response.success {
        return Err(helper_response_to_error(&response));
    }

    Ok(OperationDonePayload {
        request_id: helper_request.request_id,
        operation: "COMPRESS_TARGZ".to_owned(),
        safe_message: Some("Tar.gz archive created.".to_owned()),
    })
}

pub fn extract_archive_request<H: ExtractArchiveHelper>(
    user_context: &UserContext,
    request: ExtractArchiveRequest,
    config: &AppConfig,
    policy_engine: &PolicyEngine<'_>,
    helper_client: &H,
) -> Result<OperationDonePayload, TealDriveError> {
    let validated_archive = validate_relative_path(&request.archive_relative_path)?;
    if validated_archive.0.is_empty() {
        return Err(TealDriveError::InvalidPath);
    }
    let validated_target = validate_relative_path(&request.target_relative_path)?;

    let root = require_allowed_root(config.allowed_roots(), &request.root_id)?;
    if root.read_only {
        return Err(TealDriveError::ReadOnlyRoot);
    }

    match policy_engine.check_operation(
        FileOperation::ExtractArchive,
        &request.root_id,
        &validated_archive,
        Some(&validated_target),
        None,
        root.hidden_files_allowed,
    )? {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(PolicyReason::FeatureDisabled) => {
            return Err(TealDriveError::FeatureDisabled);
        }
        PolicyDecision::Deny(PolicyReason::ReadOnlyRoot) => {
            return Err(TealDriveError::ReadOnlyRoot);
        }
        PolicyDecision::Deny(reason) => {
            let _audit = PolicyAuditMetadata {
                action: FileOperation::ExtractArchive,
                root_id: request.root_id.clone(),
                relative_path: validated_archive,
                target_relative_path: Some(validated_target),
                reason,
                safe_message: "ExtractArchive denied by TealDrive policy.".to_owned(),
            };
            return Err(TealDriveError::PolicyDenied);
        }
    }

    let helper_request = HelperRequest {
        request_id: RequestId::new(),
        username: user_context.username.clone(),
        uid: user_context.uid,
        gid: user_context.gid,
        command: HelperCommand::ExtractArchive,
        root_id: request.root_id.clone(),
        relative_path: validated_archive,
        target_relative_path: Some(validated_target),
        policy_context: HelperPolicyContext {
            read_only_root: root.read_only,
            is_web_root: root.is_web_root,
        },
        limits: HelperLimits {
            max_directory_page_size: config.limits.max_directory_page_size,
            max_text_edit_size: config.limits.max_text_edit_size,
            max_chunk_size: config.limits.max_chunk_size,
            max_download_helper_bytes: crate::download::transfer::MAX_DOWNLOAD_HELPER_BYTES,
        },
        list_options: None,
        write_options: None,
        chmod_mode: None,
        archive_source_paths: None,
        search_options: None,
    };

    let response = helper_client.execute_helper(&helper_request)?;
    if !response.success {
        return Err(helper_response_to_error(&response));
    }

    Ok(OperationDonePayload {
        request_id: helper_request.request_id,
        operation: "EXTRACT_ARCHIVE".to_owned(),
        safe_message: Some("Archive extracted.".to_owned()),
    })
}

fn helper_response_to_error(response: &HelperResponse) -> TealDriveError {
    match response.error_kind {
        Some(ErrorKind::PermissionDenied) => TealDriveError::PermissionDenied,
        Some(ErrorKind::PolicyDenied) => TealDriveError::PolicyDenied,
        Some(ErrorKind::NotFound) => TealDriveError::NotFound,
        Some(ErrorKind::InvalidPath) => TealDriveError::InvalidPath,
        Some(ErrorKind::InvalidTarget) => TealDriveError::InvalidTarget,
        Some(ErrorKind::AlreadyExists) => TealDriveError::AlreadyExists,
        Some(ErrorKind::DiskFull) => TealDriveError::DiskFull,
        Some(ErrorKind::ValidationError) => TealDriveError::InvalidFilename,
        _ => TealDriveError::HelperCommandNotImplemented,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AllowedRoot, RelativePath, RootId};
    use crate::helper::archive::{
        compress_targz_with_roots, compress_zip_with_roots, extract_archive_with_roots,
    };
    use std::fs;
    use std::path::PathBuf;

    struct InProcessArchiveHelper {
        roots: Vec<AllowedRoot>,
    }

    impl CompressZipHelper for InProcessArchiveHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(compress_zip_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl CompressTarGzHelper for InProcessArchiveHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(compress_targz_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl ExtractArchiveHelper for InProcessArchiveHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(extract_archive_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    fn sample_user() -> UserContext {
        UserContext {
            username: "tester".to_owned(),
            uid: 1000,
            gid: 1000,
        }
    }

    fn temp_root() -> (PathBuf, AllowedRoot) {
        let path =
            std::env::temp_dir().join(format!("tealdrive-archive-svc-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temp dir");
        let root = AllowedRoot {
            root_id: RootId::new("home"),
            base_path: path.clone(),
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
            quota_bytes: None,
        };
        (path, root)
    }

    #[test]
    fn compress_and_extract_zip_roundtrip() {
        let (path, root) = temp_root();
        let file_path = path.join("hello.txt");
        fs::write(&file_path, b"hello archive service").unwrap();

        let config = AppConfig {
            roots: vec![root.clone()],
            features: crate::policy::feature_gate::FeatureFlags {
                extract_archive: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let policy_engine = PolicyEngine::new(&config);
        let helper = InProcessArchiveHelper { roots: vec![root] };

        let compress_req = CompressZipRequest {
            root_id: RootId::new("home"),
            relative_paths: vec![RelativePath::new("hello.txt")],
            target_relative_path: RelativePath::new("bundle.zip"),
        };

        let result = compress_zip_request(
            &sample_user(),
            compress_req,
            &config,
            &policy_engine,
            &helper,
        );
        assert!(result.is_ok());

        let extract_req = ExtractArchiveRequest {
            root_id: RootId::new("home"),
            archive_relative_path: RelativePath::new("bundle.zip"),
            target_relative_path: RelativePath::new("unpacked"),
        };

        let extract_result = extract_archive_request(
            &sample_user(),
            extract_req,
            &config,
            &policy_engine,
            &helper,
        );
        assert!(extract_result.is_ok());
        assert_eq!(
            fs::read_to_string(path.join("unpacked/hello.txt")).unwrap(),
            "hello archive service"
        );
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn compress_and_extract_targz_roundtrip() {
        let (path, root) = temp_root();
        let file_path = path.join("test.txt");
        fs::write(&file_path, b"hello tar service").unwrap();

        let config = AppConfig {
            roots: vec![root.clone()],
            features: crate::policy::feature_gate::FeatureFlags {
                extract_archive: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let policy_engine = PolicyEngine::new(&config);
        let helper = InProcessArchiveHelper { roots: vec![root] };

        let compress_req = CompressTarGzRequest {
            root_id: RootId::new("home"),
            relative_paths: vec![RelativePath::new("test.txt")],
            target_relative_path: RelativePath::new("bundle.tar.gz"),
        };

        let result = compress_targz_request(
            &sample_user(),
            compress_req,
            &config,
            &policy_engine,
            &helper,
        );
        assert!(result.is_ok());

        let extract_req = ExtractArchiveRequest {
            root_id: RootId::new("home"),
            archive_relative_path: RelativePath::new("bundle.tar.gz"),
            target_relative_path: RelativePath::new("unpacked_tar"),
        };

        let extract_result = extract_archive_request(
            &sample_user(),
            extract_req,
            &config,
            &policy_engine,
            &helper,
        );
        assert!(extract_result.is_ok());
        assert_eq!(
            fs::read_to_string(path.join("unpacked_tar/test.txt")).unwrap(),
            "hello tar service"
        );
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn extract_archive_feature_gate_denial() {
        let (path, root) = temp_root();
        let config = AppConfig {
            roots: vec![root.clone()],
            features: crate::policy::feature_gate::FeatureFlags {
                extract_archive: false, // disabled
                ..Default::default()
            },
            ..Default::default()
        };
        let policy_engine = PolicyEngine::new(&config);
        let helper = InProcessArchiveHelper { roots: vec![root] };

        let extract_req = ExtractArchiveRequest {
            root_id: RootId::new("home"),
            archive_relative_path: RelativePath::new("bundle.zip"),
            target_relative_path: RelativePath::new("unpacked"),
        };

        let result = extract_archive_request(
            &sample_user(),
            extract_req,
            &config,
            &policy_engine,
            &helper,
        );
        assert_eq!(result.err(), Some(TealDriveError::FeatureDisabled));
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn extract_archive_webroot_denial() {
        let (path, mut root) = temp_root();
        root.is_web_root = true;
        let config = AppConfig {
            roots: vec![root.clone()],
            features: crate::policy::feature_gate::FeatureFlags {
                extract_archive: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let policy_engine = PolicyEngine::new(&config);
        let helper = InProcessArchiveHelper { roots: vec![root] };

        let extract_req = ExtractArchiveRequest {
            root_id: RootId::new("home"),
            archive_relative_path: RelativePath::new("bundle.zip"),
            target_relative_path: RelativePath::new("unpacked"),
        };

        let result = extract_archive_request(
            &sample_user(),
            extract_req,
            &config,
            &policy_engine,
            &helper,
        );
        assert_eq!(result.err(), Some(TealDriveError::PolicyDenied));
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn compress_read_only_root_denied() {
        let (path, mut root) = temp_root();
        root.read_only = true;
        let config = AppConfig {
            roots: vec![root.clone()],
            ..Default::default()
        };
        let policy_engine = PolicyEngine::new(&config);
        let helper = InProcessArchiveHelper { roots: vec![root] };

        let compress_req = CompressZipRequest {
            root_id: RootId::new("home"),
            relative_paths: vec![RelativePath::new("hello.txt")],
            target_relative_path: RelativePath::new("bundle.zip"),
        };

        let result = compress_zip_request(
            &sample_user(),
            compress_req,
            &config,
            &policy_engine,
            &helper,
        );
        assert_eq!(result.err(), Some(TealDriveError::ReadOnlyRoot));
        let _ = fs::remove_dir_all(path);
    }
}
