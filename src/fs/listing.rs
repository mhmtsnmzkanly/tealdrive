use crate::config::{AppConfig, RelativePath, UserContext};
use crate::errors::{ErrorKind, TealDriveError};
use crate::fs::path::validate_relative_path;
use crate::helper::client::HelperProcessClient;
use crate::helper::types::{
    HelperCommand, HelperDirectoryEntry, HelperLimits, HelperListDirectoryOptions,
    HelperPolicyContext, HelperRequest, HelperResponse, HelperResponsePayload,
};
use crate::policy::decision::{FileOperation, PolicyAuditMetadata, PolicyDecision, PolicyEngine};
use crate::policy::roots::require_allowed_root;
use crate::protocol::frame::RequestId;
use crate::protocol::payload::{
    DirectoryEntryPayload, DirectoryListResponse, ListDirectoryRequest,
};
use crate::protocol::schema::{CapabilitySet, FileCapabilities};

pub trait DirectoryListingHelper {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError>;
}

impl DirectoryListingHelper for HelperProcessClient {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        self.execute(request)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryListingAudit {
    pub request_id: RequestId,
    pub username: String,
    pub action: FileOperation,
    pub root_id: crate::config::RootId,
    pub relative_path: RelativePath,
    pub status: String,
    pub reason: Option<String>,
}

pub fn list_directory_request<H: DirectoryListingHelper>(
    user_context: &UserContext,
    request: ListDirectoryRequest,
    config: &AppConfig,
    policy_engine: &PolicyEngine<'_>,
    helper_client: &H,
) -> Result<DirectoryListResponse, TealDriveError> {
    let effective_limit = effective_limit(request.limit, config);
    let request = ListDirectoryRequest {
        limit: effective_limit,
        ..request
    };
    request.validate(&config.limits)?;
    let relative_path = validate_relative_path(&request.relative_path)?;
    let root = require_allowed_root(config.allowed_roots(), &request.root_id)?;
    match policy_engine.check_operation(
        FileOperation::ListDirectory,
        &request.root_id,
        &relative_path,
        None,
        None,
        request.include_hidden,
    )? {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(reason) => {
            let _audit = PolicyAuditMetadata {
                action: FileOperation::ListDirectory,
                root_id: request.root_id.clone(),
                relative_path: relative_path.clone(),
                target_relative_path: None,
                reason,
                safe_message: "Directory listing denied by TealDrive policy.".to_owned(),
            };
            return Err(TealDriveError::PolicyDenied);
        }
    }

    let helper_request = HelperRequest {
        request_id: RequestId::new(),
        username: user_context.username.clone(),
        uid: user_context.uid,
        gid: user_context.gid,
        command: HelperCommand::ListDirectory,
        root_id: request.root_id.clone(),
        relative_path: relative_path.clone(),
        target_relative_path: None,
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
        list_options: Some(HelperListDirectoryOptions {
            cursor: request.cursor.clone(),
            limit: effective_limit,
            include_hidden: request.include_hidden,
            sort: request.sort.clone(),
            filter: request.filter.clone(),
        }),
    };

    let response = helper_client.execute_helper(&helper_request)?;
    if !response.success {
        return Err(helper_response_to_error(&response));
    }

    let HelperResponsePayload::DirectoryList(list) = response.payload else {
        return Err(TealDriveError::HelperMalformedResponse);
    };
    Ok(DirectoryListResponse {
        entries: list
            .entries
            .into_iter()
            .map(helper_entry_to_payload)
            .collect(),
        next_cursor: list.next_cursor,
        has_more: list.has_more,
        total_estimate: list.total_estimate,
        current_path: relative_path,
        capabilities: CapabilitySet {
            can_list: true,
            can_download: true,
            can_upload: root.uploads_allowed && !root.read_only,
            can_create_folder: !root.read_only,
            can_rename: !root.read_only,
            can_move_to_trash: !root.read_only,
            can_restore_from_trash: !root.read_only,
        },
    })
}

pub fn effective_limit(requested: usize, config: &AppConfig) -> usize {
    if requested == 0 {
        config.limits.default_directory_page_size
    } else {
        requested.min(config.limits.max_directory_page_size)
    }
}

pub(crate) fn helper_entry_to_payload(entry: HelperDirectoryEntry) -> DirectoryEntryPayload {
    DirectoryEntryPayload {
        name: entry.name,
        file_type: entry.file_type,
        size: entry.size,
        modified: entry.modified,
        owner: entry.owner,
        group: entry.group,
        mode: entry.mode,
        permissions: entry.permissions,
        is_hidden: entry.is_hidden,
        is_sensitive: entry.is_sensitive,
        is_protected: entry.is_protected,
        is_symlink: entry.is_symlink,
        symlink_target: entry.symlink_target,
        is_web_root: entry.is_web_root,
        is_world_writable: entry.is_world_writable,
        capabilities: FileCapabilities {
            can_list: entry.capabilities.can_list,
            can_download: entry.capabilities.can_download,
            can_upload: entry.capabilities.can_upload,
            can_create_folder: entry.capabilities.can_create_folder,
            can_rename: entry.capabilities.can_rename,
            can_move_to_trash: entry.capabilities.can_move_to_trash,
            can_restore_from_trash: entry.capabilities.can_restore_from_trash,
        },
    }
}

fn helper_response_to_error(response: &HelperResponse) -> TealDriveError {
    match response.error_kind {
        Some(ErrorKind::PermissionDenied) => TealDriveError::PermissionDenied,
        Some(ErrorKind::NotFound) => TealDriveError::InvalidPath,
        Some(ErrorKind::InvalidPath) => TealDriveError::InvalidPath,
        Some(ErrorKind::PolicyDenied) => TealDriveError::PolicyDenied,
        _ => TealDriveError::HelperCommandNotImplemented,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AllowedRoot, RootId};
    use crate::helper::listing::list_directory_with_roots;
    use crate::policy::decision::PolicyEngine;
    use crate::protocol::schema::{FileFilter, FileKind, FilterMode, SortMode};
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    struct InProcessHelper {
        roots: Vec<AllowedRoot>,
    }

    impl DirectoryListingHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(list_directory_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    fn temp_root() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("tealdrive-service-list-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temp root");
        path
    }

    fn config_for(path: PathBuf, read_only: bool) -> AppConfig {
        AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: path,
                read_only,
                uploads_allowed: true,
                hidden_files_allowed: false,
                is_web_root: false,
            }],
            ..AppConfig::default()
        }
    }

    fn request(include_hidden: bool) -> ListDirectoryRequest {
        ListDirectoryRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(""),
            cursor: None,
            limit: 200,
            include_hidden,
            sort: SortMode::NameAsc,
            filter: FilterMode {
                query: None,
                file_kind: None,
                filter: Some(FileFilter::All),
            },
        }
    }

    fn user() -> UserContext {
        UserContext {
            username: "alice".to_owned(),
            uid: 1000,
            gid: 1000,
        }
    }

    #[test]
    fn list_directory_unknown_root_rejected() {
        let config = config_for(temp_root(), false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let mut req = request(false);
        req.root_id = RootId::new("missing");

        assert_eq!(
            list_directory_request(&user(), req, &config, &engine, &helper),
            Err(TealDriveError::InvalidRootId)
        );
    }

    #[test]
    fn list_directory_rejects_absolute_and_traversal() {
        let config = config_for(temp_root(), false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let mut absolute = request(false);
        absolute.relative_path = RelativePath::new("/etc");
        let mut traversal = request(false);
        traversal.relative_path = RelativePath::new("../etc");

        assert_eq!(
            list_directory_request(&user(), absolute, &config, &engine, &helper),
            Err(TealDriveError::AbsolutePathRejected)
        );
        assert_eq!(
            list_directory_request(&user(), traversal, &config, &engine, &helper),
            Err(TealDriveError::TraversalRejected)
        );
    }

    #[test]
    fn hidden_file_excluded_by_default_and_included_when_requested() {
        let base = temp_root();
        fs::write(base.join(".hidden"), b"hidden").expect("hidden");
        fs::write(base.join("visible"), b"visible").expect("visible");
        let config = config_for(base, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        let hidden_off = list_directory_request(&user(), request(false), &config, &engine, &helper)
            .expect("list");
        let hidden_on = list_directory_request(&user(), request(true), &config, &engine, &helper)
            .expect("list");

        assert!(!hidden_off
            .entries
            .iter()
            .any(|entry| entry.name == ".hidden"));
        assert!(hidden_on
            .entries
            .iter()
            .any(|entry| entry.name == ".hidden"));
    }

    #[test]
    fn sensitive_file_flagged() {
        let base = temp_root();
        fs::write(base.join(".env"), b"secret").expect("env");
        let config = config_for(base, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let response = list_directory_request(&user(), request(true), &config, &engine, &helper)
            .expect("list");
        let env = response
            .entries
            .iter()
            .find(|entry| entry.name == ".env")
            .expect("env");

        assert!(env.is_sensitive);
        assert!(env.is_hidden);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_flagged_not_followed() {
        let base = temp_root();
        fs::write(base.join("target"), b"target").expect("target");
        symlink(base.join("target"), base.join("link")).expect("symlink");
        let config = config_for(base, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let response = list_directory_request(&user(), request(false), &config, &engine, &helper)
            .expect("list");
        let link = response
            .entries
            .iter()
            .find(|entry| entry.name == "link")
            .expect("link");

        assert!(link.is_symlink);
        assert_eq!(link.file_type, FileKind::Symlink);
    }

    #[test]
    fn read_only_root_still_allows_listing() {
        let base = temp_root();
        fs::write(base.join("file"), b"file").expect("file");
        let config = config_for(base, true);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let response = list_directory_request(&user(), request(false), &config, &engine, &helper)
            .expect("list");

        assert_eq!(response.entries.len(), 1);
        assert!(!response.capabilities.can_upload);
    }

    #[test]
    fn cursor_has_more_behavior_works() {
        let base = temp_root();
        for index in 0..3 {
            fs::write(base.join(format!("{index}.txt")), b"file").expect("file");
        }
        let config = config_for(base, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let mut req = request(false);
        req.limit = 2;
        let first =
            list_directory_request(&user(), req.clone(), &config, &engine, &helper).expect("first");
        req.cursor = first.next_cursor.clone();
        let second =
            list_directory_request(&user(), req, &config, &engine, &helper).expect("second");

        assert!(first.has_more);
        assert_eq!(first.entries.len(), 2);
        assert!(!second.has_more);
        assert_eq!(second.entries.len(), 1);
    }

    #[test]
    fn max_page_size_enforced() {
        let config = config_for(temp_root(), false);

        assert_eq!(
            effective_limit(10_000, &config),
            config.limits.max_directory_page_size
        );
        assert_eq!(
            effective_limit(0, &config),
            config.limits.default_directory_page_size
        );
    }

    #[test]
    fn directory_list_response_msgpack_roundtrip_with_real_entry() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"file").expect("file");
        let config = config_for(base, false);
        let engine = PolicyEngine::new(&config);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let response = list_directory_request(&user(), request(false), &config, &engine, &helper)
            .expect("list");
        let bytes = crate::protocol::codec::encode_msgpack(&response).expect("encoded");
        let decoded: DirectoryListResponse =
            crate::protocol::codec::decode_msgpack(&bytes).expect("decoded");

        assert_eq!(decoded, response);
    }
}
