//! Service-writable settings are a constrained overlay, never a replacement for root configuration.
use crate::config::{AppConfig, ConfigPatch};
use crate::errors::TealDriveError;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct RuntimeSettings {
    pub path: PathBuf,
    pub ceiling: AppConfig,
}
impl RuntimeSettings {
    pub fn new(path: PathBuf, ceiling: AppConfig) -> Result<Self, TealDriveError> {
        if !path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(TealDriveError::Validation);
        }
        Ok(Self { path, ceiling })
    }
    pub fn check_patch(patch: &ConfigPatch) -> Result<(), TealDriveError> {
        if patch.proxy.is_some()
            || patch.security.is_some()
            || patch.roots.is_some()
            || patch.allowed_users.is_some()
            || patch.admin_users.is_some()
            || patch.static_dir.is_some()
            || patch.deny_edit_patterns.is_some()
            || patch.warn_read_patterns.is_some()
            || patch.trash_retention_days.is_some()
            || patch.session.as_ref().is_some_and(|s| {
                s.cookie_name.is_some() || s.secure_cookies.is_some() || s.same_site.is_some()
            })
        {
            return Err(TealDriveError::PermissionDenied);
        }
        Ok(())
    }
    pub fn read(&self, base: &AppConfig) -> Result<AppConfig, TealDriveError> {
        use std::io::Read;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = match std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&self.path)
        {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(base.clone()),
            Err(_) => return Err(TealDriveError::Validation),
        };
        if !file
            .metadata()
            .map_err(|_| TealDriveError::Validation)?
            .is_file()
        {
            return Err(TealDriveError::Validation);
        }
        let mut bytes = Vec::new();
        file.by_ref()
            .take(65537)
            .read_to_end(&mut bytes)
            .map_err(|_| TealDriveError::Validation)?;
        if bytes.len() > 65536 {
            return Err(TealDriveError::Validation);
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| TealDriveError::Validation)?;
        let patch: ConfigPatch = if self.path.extension().is_some_and(|e| e == "json") {
            serde_json::from_str(text).map_err(|_| TealDriveError::Validation)?
        } else {
            toml::from_str(text).map_err(|_| TealDriveError::Validation)?
        };
        Self::check_patch(&patch)?;
        let mut config = base.clone();
        patch.apply_to(&mut config);
        self.validate(&config, base)?;
        Ok(config)
    }
    pub fn validate(&self, config: &AppConfig, base: &AppConfig) -> Result<(), TealDriveError> {
        config.validate()?;
        let mut immutable = config.clone();
        immutable.quota_warning_percent = base.quota_warning_percent;
        immutable.features = base.features.clone();
        immutable.limits = base.limits;
        immutable.audit = base.audit.clone();
        immutable.session.idle_timeout_seconds = base.session.idle_timeout_seconds;
        immutable.session.absolute_timeout_seconds = base.session.absolute_timeout_seconds;
        immutable.helper = base.helper.clone();
        immutable.search = base.search.clone();
        immutable.rate_limit = base.rate_limit.clone();
        if immutable != *base {
            return Err(TealDriveError::PermissionDenied);
        }
        let requested =
            serde_json::to_value(&config.features).map_err(|_| TealDriveError::Internal)?;
        let allowed = serde_json::to_value(&base.features).map_err(|_| TealDriveError::Internal)?;
        for (name, value) in requested.as_object().ok_or(TealDriveError::Internal)? {
            if value.as_bool() == Some(true)
                && allowed.get(name).and_then(|v| v.as_bool()) != Some(true)
            {
                return Err(TealDriveError::PermissionDenied);
            }
        }
        let requested =
            serde_json::to_value(config.limits).map_err(|_| TealDriveError::Internal)?;
        let allowed = serde_json::to_value(base.limits).map_err(|_| TealDriveError::Internal)?;
        for (name, value) in requested.as_object().ok_or(TealDriveError::Internal)? {
            if value.as_u64() > allowed.get(name).and_then(|v| v.as_u64()) {
                return Err(TealDriveError::PermissionDenied);
            }
        }
        if config.session.idle_timeout_seconds > base.session.idle_timeout_seconds
            || config.session.absolute_timeout_seconds > base.session.absolute_timeout_seconds
            || config.helper.timeout_seconds > base.helper.timeout_seconds
            || config.search.max_depth > base.search.max_depth
            || config.search.default_max_results > base.search.default_max_results
            || config.search.max_results_limit > base.search.max_results_limit
            || config.rate_limit.max_login_failures > base.rate_limit.max_login_failures
            || config.rate_limit.lockout_duration_seconds < base.rate_limit.lockout_duration_seconds
            || config.audit.max_file_bytes > base.audit.max_file_bytes
            || config.audit.max_archives > base.audit.max_archives
        {
            return Err(TealDriveError::PermissionDenied);
        }
        if let Some(max) = base.audit.effective_retention_hours() {
            if config
                .audit
                .effective_retention_hours()
                .is_none_or(|hours| hours > max)
            {
                return Err(TealDriveError::PermissionDenied);
            }
        }
        // Root chooses the log directory. Panel can choose a filename within it.
        if config.audit.path != base.audit.path {
            let Some(parent) = base.audit.path.as_deref().and_then(Path::parent) else {
                return Err(TealDriveError::PermissionDenied);
            };
            let Some(path) = config.audit.path.as_deref() else {
                return Err(TealDriveError::PermissionDenied);
            };
            if path.parent() != Some(parent) || path.file_name().is_none() {
                return Err(TealDriveError::PermissionDenied);
            }
        }
        Ok(())
    }
    pub fn project(config: &AppConfig) -> ConfigPatch {
        ConfigPatch {
            quota_warning_percent: Some(config.quota_warning_percent),
            features: Some(config.features.clone().into()),
            limits: Some(config.limits.into()),
            audit: Some(crate::config_patch::AuditPatch {
                path: config.audit.path.clone(),
                max_file_bytes: Some(config.audit.max_file_bytes),
                max_archives: Some(config.audit.max_archives),
                retention_hours: Some(config.audit.effective_retention_hours()),
                retention_days: None,
            }),
            session: Some(crate::config_patch::SessionPatch {
                idle_timeout_seconds: Some(config.session.idle_timeout_seconds),
                absolute_timeout_seconds: Some(config.session.absolute_timeout_seconds),
                ..Default::default()
            }),
            rate_limit: Some(config.rate_limit.clone().into()),
            helper: Some(config.helper.clone().into()),
            search: Some(config.search.clone().into()),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, RuntimeSettings) {
        let dir = std::env::temp_dir().join(format!("tealdrive-settings-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let mut base = AppConfig::default();
        base.features.save_text_file = true;
        base.audit.path = Some(dir.join("audit.jsonl"));
        (
            dir.clone(),
            RuntimeSettings::new(dir.join("settings.toml"), base).unwrap(),
        )
    }
    #[test]
    fn root_channel_identity_cannot_be_changed_by_overlay() {
        let (dir, mut settings) = fixture();
        settings.ceiling.server.require_tls = Some(true);
        settings.ceiling.server.bind_addr = "127.0.0.1:13000".into();
        settings.ceiling.server.behind_tls_proxy = true;
        settings.ceiling.session.secure_cookies = true;
        settings.ceiling.proxy.trusted_proxies = vec!["127.0.0.1/32".into()];
        settings.ceiling.security.allowed_origins = vec!["https://drive.test".into()];
        settings.ceiling.validate().unwrap();
        let mut requested = settings.ceiling.clone();
        requested.server.channel_key_path = Some("/home/tealdrive/tls/other.key".into());
        assert!(matches!(
            settings.validate(&requested, &settings.ceiling),
            Err(TealDriveError::PermissionDenied)
        ));
        std::fs::write(
            &settings.path,
            "[server]\nchannel_key_path='/home/tealdrive/tls/other.key'\n",
        )
        .unwrap();
        assert!(settings.read(&settings.ceiling).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn root_system_storage_budget_cannot_be_changed_by_overlay() {
        let (dir, settings) = fixture();
        let mut requested = settings.ceiling.clone();
        requested.system_storage.max_staging_bytes *= 2;
        assert!(matches!(
            settings.validate(&requested, &settings.ceiling),
            Err(TealDriveError::PermissionDenied)
        ));
        std::fs::write(
            &settings.path,
            "[system_storage]\nmax_staging_bytes=8589934592",
        )
        .unwrap();
        assert!(settings.read(&settings.ceiling).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn overlay_roundtrip_cannot_promote_admin_or_relax_limits() {
        let (dir, settings) = fixture();
        let mut chosen = settings.ceiling.clone();
        chosen.quota_warning_percent = 75;
        chosen.features.save_text_file = false;
        chosen.audit.retention_hours = Some(36);
        let patch = RuntimeSettings::project(&chosen);
        std::fs::write(&settings.path, toml::to_string(&patch).unwrap()).unwrap();
        assert_eq!(settings.read(&settings.ceiling).unwrap(), chosen);
        std::fs::write(&settings.path, "admin_users = ['attacker']").unwrap();
        assert!(settings.read(&settings.ceiling).is_err());
        for bad in [
            "[features]\nchmod_file = true",
            "[limits]\nmax_preview_size = 52428801",
            "[session]\nsecure_cookies = false",
            "[audit]\nretention_hours = 505",
            "[audit]\nretention_hours = 0",
            "[audit]\npath = '/etc/passwd'",
        ] {
            std::fs::write(&settings.path, bad).unwrap();
            assert!(settings.read(&settings.ceiling).is_err(), "{bad}");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn missing_empty_corrupt_and_symlink_settings() {
        let (dir, settings) = fixture();
        assert_eq!(settings.read(&settings.ceiling).unwrap(), settings.ceiling);
        std::fs::write(&settings.path, "").unwrap();
        assert_eq!(settings.read(&settings.ceiling).unwrap(), settings.ceiling);
        std::fs::write(&settings.path, "unknown = 1").unwrap();
        assert!(settings.read(&settings.ceiling).is_err());
        std::fs::remove_file(&settings.path).unwrap();
        std::fs::write(dir.join("target"), "quota_warning_percent = 75").unwrap();
        std::os::unix::fs::symlink(dir.join("target"), &settings.path).unwrap();
        assert!(settings.read(&settings.ceiling).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
