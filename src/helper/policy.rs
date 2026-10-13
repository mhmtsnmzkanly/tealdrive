//! Each operation carries an immutable policy snapshot. The privileged helper's
//! root-owned configuration remains the ceiling that a network process cannot widen.
use crate::config::AppConfig;
use crate::errors::TealDriveError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
thread_local! { static SNAPSHOT: RefCell<Option<PolicySnapshot>> = const { RefCell::new(None) }; }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySnapshot {
    pub revision: [u8; 32],
    pub config: AppConfig,
}
impl PolicySnapshot {
    pub fn new(config: AppConfig) -> Self {
        let revision =
            Sha256::digest(rmp_serde::to_vec_named(&config).expect("serializable policy")).into();
        Self { revision, config }
    }
    pub fn validate(&self) -> Result<(), TealDriveError> {
        self.config.validate()?;
        if Self::new(self.config.clone()).revision != self.revision {
            return Err(TealDriveError::Validation);
        }
        Ok(())
    }
}
pub fn current() -> Option<PolicySnapshot> {
    SNAPSHOT.with(|s| s.borrow().clone())
}
pub fn with_snapshot<T>(config: &AppConfig, f: impl FnOnce() -> T) -> T {
    struct Reset(Option<PolicySnapshot>);
    impl Drop for Reset {
        fn drop(&mut self) {
            SNAPSHOT.with(|s| *s.borrow_mut() = self.0.take());
        }
    }
    let _reset = Reset(SNAPSHOT.with(|s| s.replace(Some(PolicySnapshot::new(config.clone())))));
    f()
}
pub fn restrict(
    snapshot: &PolicySnapshot,
    ceiling: &AppConfig,
) -> Result<AppConfig, TealDriveError> {
    snapshot.validate()?;
    let mut config = snapshot.config.clone();
    config.virtual_accounts = ceiling.virtual_accounts.clone();
    config.web_accounts = ceiling.web_accounts.clone();
    config.journal = ceiling.journal.clone();
    config.lifecycle = ceiling.lifecycle.clone();
    config.control_audit = ceiling.control_audit.clone();
    config.access_policy.extend_ceiling(&ceiling.access_policy);
    config.access_policy.validate()?;
    for root in &mut config.roots {
        let trusted = ceiling
            .roots
            .iter()
            .find(|r| r.root_id == root.root_id && r.base_path == root.base_path)
            .ok_or(TealDriveError::PermissionDenied)?;
        root.read_only |= trusted.read_only;
        root.uploads_allowed &= trusted.uploads_allowed;
        root.hidden_files_allowed &= trusted.hidden_files_allowed;
        root.is_web_root |= trusted.is_web_root;
        root.quota_bytes = match (root.quota_bytes, trusted.quota_bytes) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, None) => a,
            (None, b) => b,
        };
    }
    config
        .sensitive_policy
        .deny_edit_patterns
        .extend(ceiling.sensitive_policy.deny_edit_patterns.clone());
    config
        .sensitive_policy
        .warn_read_patterns
        .extend(ceiling.sensitive_policy.warn_read_patterns.clone());
    config.webroot_policy.block_executable_uploads |=
        ceiling.webroot_policy.block_executable_uploads;
    config
        .webroot_policy
        .blocked_extensions
        .extend(ceiling.webroot_policy.blocked_extensions.clone());
    config.features.save_text_file &= ceiling.features.save_text_file;
    config.features.chmod_file &= ceiling.features.chmod_file;
    config.features.extract_archive &= ceiling.features.extract_archive;
    config.features.delete_permanently &= ceiling.features.delete_permanently;
    config.features.chown &= ceiling.features.chown;
    config.limits.max_in_flight_upload = config
        .limits
        .max_in_flight_upload
        .min(ceiling.limits.max_in_flight_upload);
    config.limits.max_chunk_size = config
        .limits
        .max_chunk_size
        .min(ceiling.limits.max_chunk_size);
    config.limits.max_text_edit_size = config
        .limits
        .max_text_edit_size
        .min(ceiling.limits.max_text_edit_size);
    config.limits.max_directory_page_size = config
        .limits
        .max_directory_page_size
        .min(ceiling.limits.max_directory_page_size);
    Ok(config)
}
pub fn authorize(
    request: &crate::helper::types::HelperRequest,
    config: &AppConfig,
) -> Result<(), TealDriveError> {
    crate::helper::access::authorize(request, config)?;
    use crate::helper::types::HelperCommand::*;
    if let Some(options) = &request.write_options {
        let bound = match request.command {
            WriteFile => config.limits.max_text_edit_size,
            UploadCommit => config.limits.max_in_flight_upload,
            _ => usize::MAX,
        };
        if options.stream_size.is_some()
            && (request.command != UploadCommit || !options.data.is_empty())
        {
            return Err(TealDriveError::Validation);
        }
        if options.stream_size.unwrap_or(options.data.len() as u64) > bound as u64 {
            return Err(TealDriveError::FileTooLarge);
        }
        if matches!(request.command, WriteFile | UploadCommit)
            && (options.offset != 0 || !options.truncate)
        {
            return Err(TealDriveError::Validation);
        }
    }
    let root = config
        .roots
        .iter()
        .find(|r| r.root_id == request.root_id)
        .ok_or(TealDriveError::InvalidRootId)?;
    let read = matches!(
        request.command,
        QuotaUsage
            | ListDirectory
            | FileMetadata
            | DownloadFile
            | ReadFileRange
            | ReadTextFile
            | ReadFile
            | SearchFiles
            | ListTrash
    );
    if !read && root.read_only {
        return Err(TealDriveError::PolicyDenied);
    }
    if matches!(request.command, UploadCommit) && !root.uploads_allowed {
        return Err(TealDriveError::PolicyDenied);
    }
    let internal_trash = matches!(request.command, RestoreFromTrash | ListTrash)
        || (request.command == DeletePermanently
            && request
                .relative_path
                .0
                .starts_with(".tealdrive-trash/files/"));
    if !internal_trash
        && !root.hidden_files_allowed
        && crate::fs::path::is_hidden_relative(&request.relative_path)
    {
        return Err(TealDriveError::PolicyDenied);
    }
    for path in request
        .archive_source_paths
        .iter()
        .flatten()
        .chain(request.target_relative_path.iter())
    {
        if (!root.hidden_files_allowed && crate::fs::path::is_hidden_relative(path))
            || crate::policy::sensitive::detect_sensitive(path, &config.sensitive_policy)
                .is_sensitive
        {
            return Err(TealDriveError::PolicyDenied);
        }
    }
    if !internal_trash
        && !matches!(
            request.command,
            ListDirectory | FileMetadata | ListTrash | SearchFiles
        )
        && crate::policy::sensitive::detect_sensitive(
            &request.relative_path,
            &config.sensitive_policy,
        )
        .is_sensitive
    {
        return Err(TealDriveError::PolicyDenied);
    }
    let target = request
        .target_relative_path
        .as_ref()
        .unwrap_or(&request.relative_path);
    if !read && root.is_web_root {
        if request.command == ExtractArchive {
            return Err(TealDriveError::PolicyDenied);
        }
        if let Some(name) = std::path::Path::new(&target.0)
            .file_name()
            .and_then(|s| s.to_str())
        {
            let name = crate::fs::filename::SafeFileName::parse(name)?;
            if matches!(
                crate::policy::webroot::check_webroot_upload(true, &name, &config.webroot_policy),
                crate::policy::decision::PolicyDecision::Deny(_)
            ) {
                return Err(TealDriveError::PolicyDenied);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_cannot_widen_privileged_root() {
        let ceiling = AppConfig::default();
        let mut config = ceiling.clone();
        config.roots[0].base_path = "/etc".into();
        assert_eq!(
            restrict(&PolicySnapshot::new(config), &ceiling),
            Err(TealDriveError::PermissionDenied)
        );
    }
    #[test]
    fn snapshot_cannot_disable_or_change_protected_journal_limits() {
        let mut ceiling = AppConfig::default();
        ceiling.journal.enabled = true;
        ceiling.journal.undo_max_hours = 6;
        let mut snapshot = ceiling.clone();
        snapshot.journal.enabled = false;
        snapshot.journal.max_entries_per_account = 100;
        snapshot.journal.max_total_entries = 100;
        snapshot.journal.max_record_bytes = 32768;
        snapshot.journal.undo_max_hours = 24;
        let restricted = restrict(&PolicySnapshot::new(snapshot), &ceiling).unwrap();
        assert_eq!(restricted.journal, ceiling.journal);
    }
    #[test]
    fn privileged_policy_cannot_be_disabled_by_snapshot() {
        let mut ceiling = AppConfig::default();
        ceiling.roots[0].read_only = true;
        ceiling.roots[0].quota_bytes = Some(100);
        let config = restrict(&PolicySnapshot::new(AppConfig::default()), &ceiling).unwrap();
        assert!(config.roots[0].read_only);
        assert_eq!(config.roots[0].quota_bytes, Some(100));
    }
}
