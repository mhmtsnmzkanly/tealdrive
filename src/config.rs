use arc_swap::ArcSwap;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::errors::TealDriveError;
use crate::limits::Limits;
use crate::policy::feature_gate::FeatureFlags;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct RootId(pub String);

impl RootId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl From<&str> for RootId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for RootId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct RelativePath(pub String);

impl RelativePath {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl From<&str> for RelativePath {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for RelativePath {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserContext {
    pub username: String,
    pub uid: u32,
    pub gid: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub audit: AuditConfig,
    pub session: SessionConfig,
    pub security: SecurityConfig,
    pub roots: Vec<AllowedRoot>,
    pub sensitive_policy: SensitivePolicyConfig,
    pub webroot_policy: WebRootPolicyConfig,
    pub account_policy: AccountPolicyConfig,
    pub features: FeatureFlags,
    pub limits: Limits,
    #[serde(default)]
    pub trash_retention_days: Option<u32>,
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
    #[serde(default)]
    pub helper: HelperConfig,
    #[serde(default)]
    pub search: SearchConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            audit: AuditConfig::default(),
            session: SessionConfig::default(),
            security: SecurityConfig::default(),
            roots: vec![AllowedRoot::default()],
            sensitive_policy: SensitivePolicyConfig::default(),
            webroot_policy: WebRootPolicyConfig::default(),
            account_policy: AccountPolicyConfig::default(),
            features: FeatureFlags::default(),
            limits: Limits::default(),
            trash_retention_days: None,
            rate_limit: RateLimitConfig::default(),
            helper: HelperConfig::default(),
            search: SearchConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn allowed_roots(&self) -> &[AllowedRoot] {
        &self.roots
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, TealDriveError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|_| TealDriveError::NotFound)?;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let config: Self = if ext == "json" {
            serde_json::from_str(&content).map_err(|_| TealDriveError::Validation)?
        } else {
            toml::from_str(&content).map_err(|_| TealDriveError::Validation)?
        };
        config.validate()?;
        Ok(config)
    }

    pub fn resolve_config_path(explicit: Option<&Path>) -> Option<PathBuf> {
        if let Some(path) = explicit {
            return Some(path.to_path_buf());
        }
        if let Ok(path) = std::env::var("TEALDRIVE_CONFIG") {
            if !path.trim().is_empty() {
                return Some(path.trim().into());
            }
        }
        [
            "tealdrive.toml",
            "config.toml",
            "/etc/tealdrive/config.toml",
            "/etc/tealdrive/tealdrive.toml",
        ]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
    }
    pub fn load_or_default(explicit_path: Option<&Path>) -> Result<Self, TealDriveError> {
        match Self::resolve_config_path(explicit_path) {
            Some(path) => Self::from_file(path),
            None => {
                let config = Self::default();
                config.validate()?;
                Ok(config)
            }
        }
    }

    pub fn validate(&self) -> Result<(), TealDriveError> {
        if self.roots.is_empty() {
            return Err(TealDriveError::Validation);
        }
        if self
            .server
            .bind_addr
            .parse::<std::net::SocketAddr>()
            .is_err()
        {
            return Err(TealDriveError::Validation);
        }
        if self.audit.max_file_bytes < 1024
            || self.audit.max_file_bytes > 1024 * 1024 * 1024
            || self.audit.max_archives > 1000
            || self
                .audit
                .retention_days
                .is_some_and(|days| days == 0 || days > 36500)
            || self.audit.path.as_ref().is_some_and(|path| {
                !path.is_absolute()
                    || path.file_name().is_none()
                    || path
                        .components()
                        .any(|p| matches!(p, std::path::Component::ParentDir))
            })
        {
            return Err(TealDriveError::Validation);
        }
        let l = &self.limits;
        if l.max_chunk_size == 0
            || l.max_chunk_size > crate::limits::MAX_CHUNK_SIZE
            || l.max_frame_payload == 0
            || l.max_frame_payload > crate::limits::MAX_FRAME_PAYLOAD
            || l.max_control_payload == 0
            || l.max_control_payload > l.max_frame_payload
            || l.max_chunk_size + crate::protocol::chunk::RAW_CHUNK_HEADER_LEN > l.max_frame_payload
            || l.max_text_edit_size == 0
            || l.max_text_edit_size > l.max_control_payload / 2
            || l.max_in_flight_upload == 0
            || l.max_in_flight_upload > 3 * crate::limits::MIB
            || l.max_directory_page_size == 0
            || l.max_directory_page_size > crate::limits::MAX_DIRECTORY_PAGE_SIZE
            || l.default_directory_page_size == 0
            || l.default_directory_page_size > l.max_directory_page_size
            || self.session.idle_timeout_seconds == 0
            || self.session.absolute_timeout_seconds < self.session.idle_timeout_seconds
            || self.rate_limit.max_login_failures == 0
            || self.rate_limit.lockout_duration_seconds == 0
            || self.helper.timeout_seconds == 0
            || self.helper.timeout_seconds > 3600
            || self.search.default_max_results == 0
            || self.search.default_max_results > self.search.max_results_limit
            || self.search.max_results_limit > 10000
            || self.search.max_depth > 128
        {
            return Err(TealDriveError::Validation);
        }
        let mut ids = std::collections::HashSet::new();
        for root in &self.roots {
            if root.root_id.0.is_empty()
                || !root
                    .root_id
                    .0
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                || !ids.insert(&root.root_id)
                || !root.base_path.is_absolute()
                || root
                    .base_path
                    .components()
                    .any(|p| matches!(p, std::path::Component::ParentDir))
            {
                return Err(TealDriveError::Validation);
            }
        }
        for origin in &self.security.allowed_origins {
            let uri: axum::http::Uri = origin.parse().map_err(|_| TealDriveError::Validation)?;
            if !matches!(uri.scheme_str(), Some("https" | "http"))
                || uri.authority().is_none()
                || uri.query().is_some()
                || uri.path() != "/"
            {
                return Err(TealDriveError::Validation);
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AuditConfig {
    pub path: Option<PathBuf>,
    pub max_file_bytes: u64,
    pub max_archives: usize,
    #[serde(
        serialize_with = "audit_retention_serialize",
        deserialize_with = "audit_retention_deserialize"
    )]
    pub retention_days: Option<u32>,
}
fn audit_retention_serialize<S: serde::Serializer>(
    value: &Option<u32>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_u32(value.unwrap_or(0))
}
fn audit_retention_deserialize<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<u32>, D::Error> {
    Option::<u32>::deserialize(deserializer).map(|value| value.filter(|days| *days != 0))
}
impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            path: None,
            max_file_bytes: 10 * 1024 * 1024,
            max_archives: 5,
            retention_days: Some(30),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    pub bind_addr: String,
    pub behind_tls_proxy: bool,
    #[serde(default)]
    pub static_dir: Option<PathBuf>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:3000".to_owned(),
            behind_tls_proxy: true,
            static_dir: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SessionConfig {
    pub cookie_name: String,
    pub idle_timeout_seconds: u64,
    pub absolute_timeout_seconds: u64,
    pub secure_cookies: bool,
    pub same_site: SameSitePolicy,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            cookie_name: "tealdrive_session".to_owned(),
            idle_timeout_seconds: 30 * 60,
            absolute_timeout_seconds: 12 * 60 * 60,
            secure_cookies: true,
            same_site: SameSitePolicy::Strict,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SameSitePolicy {
    Strict,
    Lax,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SecurityConfig {
    pub production_wire_protocol: WireProtocol,
    pub allow_plain_ws_localhost_only: bool,
    #[serde(default)]
    pub allowed_origins: Vec<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            production_wire_protocol: WireProtocol::Tdrv1Binary,
            allow_plain_ws_localhost_only: true,
            allowed_origins: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireProtocol {
    Tdrv1Binary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllowedRoot {
    pub root_id: RootId,
    pub base_path: PathBuf,
    pub read_only: bool,
    pub uploads_allowed: bool,
    pub hidden_files_allowed: bool,
    pub is_web_root: bool,
    #[serde(default)]
    pub quota_bytes: Option<u64>,
}

impl Default for AllowedRoot {
    fn default() -> Self {
        Self {
            root_id: RootId::new("home"),
            base_path: PathBuf::from("/home"),
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
            quota_bytes: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SensitivePolicyConfig {
    pub deny_edit_patterns: Vec<String>,
    pub warn_read_patterns: Vec<String>,
}

impl Default for SensitivePolicyConfig {
    fn default() -> Self {
        Self {
            deny_edit_patterns: vec![
                ".env".to_owned(),
                ".env.*".to_owned(),
                "id_rsa".to_owned(),
                "id_dsa".to_owned(),
                "id_ecdsa".to_owned(),
                "id_ed25519".to_owned(),
                "*.pem".to_owned(),
                "*.key".to_owned(),
                "authorized_keys".to_owned(),
                "known_hosts".to_owned(),
                "*.sql".to_owned(),
                "*.dump".to_owned(),
            ],
            warn_read_patterns: vec![
                ".env".to_owned(),
                ".env.*".to_owned(),
                "*.pem".to_owned(),
                "*.key".to_owned(),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WebRootPolicyConfig {
    pub block_executable_uploads: bool,
    pub blocked_extensions: Vec<String>,
}

impl Default for WebRootPolicyConfig {
    fn default() -> Self {
        Self {
            block_executable_uploads: true,
            blocked_extensions: vec![
                "php".to_owned(),
                "phtml".to_owned(),
                "cgi".to_owned(),
                "pl".to_owned(),
                "py".to_owned(),
                "rb".to_owned(),
                "sh".to_owned(),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AccountPolicyConfig {
    pub reject_root: bool,
    pub reject_system_accounts: bool,
    pub reject_disabled_shells: bool,
    pub minimum_regular_uid: u32,
    #[serde(default)]
    pub admin_users: Vec<String>,
}

impl Default for AccountPolicyConfig {
    fn default() -> Self {
        Self {
            reject_root: true,
            reject_system_accounts: true,
            reject_disabled_shells: true,
            minimum_regular_uid: 1000,
            admin_users: Vec::new(),
        }
    }
}

impl AccountPolicyConfig {
    pub fn is_admin(&self, username: &str) -> bool {
        self.admin_users.iter().any(|user| user == username)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RateLimitConfig {
    pub max_login_failures: u32,
    pub lockout_duration_seconds: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_login_failures: 5,
            lockout_duration_seconds: 300,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HelperConfig {
    pub timeout_seconds: u64,
}

impl Default for HelperConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 10,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SearchConfig {
    pub max_depth: usize,
    pub default_max_results: usize,
    pub max_results_limit: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            max_depth: 20,
            default_max_results: 100,
            max_results_limit: 1000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit: Option<crate::config_patch::AuditPatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub features: Option<crate::config_patch::FeaturesPatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<crate::config_patch::LimitsPatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "crate::config_patch::nullable")]
    pub trash_retention_days: Option<Option<u32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roots: Option<Vec<AllowedRoot>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_edit_patterns: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warn_read_patterns: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "crate::config_patch::nullable")]
    pub static_dir: Option<Option<PathBuf>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<crate::config_patch::SessionPatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<crate::config_patch::SecurityPatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<crate::config_patch::RateLimitPatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helper: Option<crate::config_patch::HelperPatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<crate::config_patch::SearchPatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_users: Option<Vec<String>>,
}

impl ConfigPatch {
    pub fn apply_to(&self, config: &mut AppConfig) {
        if let Some(audit) = &self.audit {
            audit.apply_to(&mut config.audit);
        }
        if let Some(ref features) = self.features {
            features.apply_to(&mut config.features);
        }
        if let Some(ref limits) = self.limits {
            limits.apply_to(&mut config.limits);
        }
        if let Some(trash_retention_days) = self.trash_retention_days {
            config.trash_retention_days = trash_retention_days;
        }
        if let Some(ref roots) = self.roots {
            config.roots = roots.clone();
        }
        if let Some(ref deny_edit) = self.deny_edit_patterns {
            config.sensitive_policy.deny_edit_patterns = deny_edit.clone();
        }
        if let Some(ref warn_read) = self.warn_read_patterns {
            config.sensitive_policy.warn_read_patterns = warn_read.clone();
        }
        if let Some(ref static_dir) = self.static_dir {
            config.server.static_dir = static_dir.clone();
        }
        if let Some(ref session) = self.session {
            session.apply_to(&mut config.session);
        }
        if let Some(ref security) = self.security {
            security.apply_to(&mut config.security);
        }
        if let Some(ref rate_limit) = self.rate_limit {
            rate_limit.apply_to(&mut config.rate_limit);
        }
        if let Some(ref helper) = self.helper {
            helper.apply_to(&mut config.helper);
        }
        if let Some(ref search) = self.search {
            search.apply_to(&mut config.search);
        }
        if let Some(ref admin_users) = self.admin_users {
            config.account_policy.admin_users = admin_users.clone();
        }
    }
}

#[derive(Clone, Debug)]
pub struct DynamicConfig {
    inner: Arc<ArcSwap<AppConfig>>,
    config_path: Option<PathBuf>,
    mutation: Arc<std::sync::Mutex<()>>,
    audit_sink: Option<Arc<crate::audit::sink::JsonlAuditSink>>,
}

impl DynamicConfig {
    pub fn new(config: AppConfig, path: Option<PathBuf>) -> Self {
        Self {
            inner: Arc::new(ArcSwap::from_pointee(config)),
            config_path: path,
            mutation: Arc::new(std::sync::Mutex::new(())),
            audit_sink: None,
        }
    }

    pub fn with_audit_sink(mut self, sink: Arc<crate::audit::sink::JsonlAuditSink>) -> Self {
        self.audit_sink = Some(sink);
        self
    }
    fn prepare_audit(
        &self,
        config: &mut AppConfig,
    ) -> Result<Option<crate::audit::sink::PreparedAudit>, TealDriveError> {
        if config.audit.path.is_none() {
            if let Some(sink) = &self.audit_sink {
                config.audit.path = Some(sink.path());
            }
        }
        if config
            .audit
            .path
            .as_ref()
            .is_some_and(|path| self.config_path.as_ref() == Some(path))
        {
            return Err(TealDriveError::Validation);
        }
        self.audit_sink
            .as_ref()
            .map(|sink| sink.prepare(&config.audit))
            .transpose()
            .map_err(|_| TealDriveError::Internal)
    }
    fn commit_audit(&self, prepared: Option<crate::audit::sink::PreparedAudit>) {
        if let (Some(sink), Some(prepared)) = (&self.audit_sink, prepared) {
            sink.apply(prepared);
        }
    }
    pub fn load(&self) -> arc_swap::Guard<Arc<AppConfig>> {
        self.inner.load()
    }

    pub fn load_full(&self) -> Arc<AppConfig> {
        self.inner.load_full()
    }

    pub fn config_path(&self) -> Option<&Path> {
        self.config_path.as_deref()
    }

    pub fn set_config_path(&mut self, path: PathBuf) {
        self.config_path = Some(path);
    }

    pub fn store(&self, mut new_config: AppConfig) -> Result<Arc<AppConfig>, TealDriveError> {
        let _lock = self.mutation.lock().map_err(|_| TealDriveError::Internal)?;
        new_config.validate()?;
        let prepared = self.prepare_audit(&mut new_config)?;
        let arc = Arc::new(new_config);
        self.commit_audit(prepared);
        self.inner.store(arc.clone());
        Ok(arc)
    }

    pub fn reload(&self) -> Result<Arc<AppConfig>, TealDriveError> {
        let _lock = self.mutation.lock().map_err(|_| TealDriveError::Internal)?;
        let path = self.config_path.as_ref().ok_or(TealDriveError::NotFound)?;
        let mut loaded = AppConfig::from_file(path)?;
        let prepared = self.prepare_audit(&mut loaded)?;
        let loaded = Arc::new(loaded);
        self.commit_audit(prepared);
        self.inner.store(loaded.clone());
        Ok(loaded)
    }

    pub fn apply_patch(&self, patch: ConfigPatch) -> Result<Arc<AppConfig>, TealDriveError> {
        self.apply_patch_saved(patch, false)
    }
    pub fn apply_patch_saved(
        &self,
        patch: ConfigPatch,
        save: bool,
    ) -> Result<Arc<AppConfig>, TealDriveError> {
        let _lock = self.mutation.lock().map_err(|_| TealDriveError::Internal)?;
        let mut config = (*self.load_full()).clone();
        patch.apply_to(&mut config);
        config.validate()?;
        let prepared = self.prepare_audit(&mut config)?;
        if save {
            self.persist(&config)?;
        }
        let config = Arc::new(config);
        self.commit_audit(prepared);
        self.inner.store(config.clone());
        Ok(config)
    }
    pub fn save_to_file(&self) -> Result<(), TealDriveError> {
        let _lock = self.mutation.lock().map_err(|_| TealDriveError::Internal)?;
        self.persist(&self.load_full())
    }
    fn persist(&self, config: &AppConfig) -> Result<(), TealDriveError> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let path = self.config_path.as_ref().ok_or(TealDriveError::NotFound)?;
        let content = if path.extension().and_then(|p| p.to_str()) == Some("json") {
            serde_json::to_string_pretty(config).map_err(|_| TealDriveError::Internal)?
        } else {
            toml::to_string_pretty(config).map_err(|_| TealDriveError::Internal)?
        };
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let temporary = parent.join(format!(".tealdrive-config-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| -> std::io::Result<()> {
            let mut file = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&temporary)?;
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
            std::fs::rename(&temporary, path)?;
            std::fs::File::open(parent)?.sync_all()
        })();
        let _ = std::fs::remove_file(temporary);
        result.map_err(|_| TealDriveError::Internal)
    }
}

impl Default for DynamicConfig {
    fn default() -> Self {
        Self::new(AppConfig::default(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_config_default_is_safe() {
        let config = AppConfig::default();

        assert_eq!(
            config.security.production_wire_protocol,
            WireProtocol::Tdrv1Binary
        );
        assert!(config.account_policy.reject_root);
        assert!(config.account_policy.reject_system_accounts);
        assert!(config.session.secure_cookies);
        assert_eq!(config.roots[0].root_id, RootId::new("home"));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn toml_roundtrip_success() {
        let original = AppConfig::default();
        let toml_str = toml::to_string(&original).expect("toml serialize");
        let decoded: AppConfig = toml::from_str(&toml_str).expect("toml deserialize");
        assert_eq!(decoded.server.bind_addr, original.server.bind_addr);
        assert_eq!(decoded.roots.len(), original.roots.len());
    }

    #[test]
    fn from_file_toml_and_json_test() {
        let dir = std::env::temp_dir().join(format!("tealdrive-cfg-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        // 1. TOML file
        let toml_path = dir.join("tealdrive.toml");
        let toml_content = toml::to_string(&AppConfig::default()).unwrap();
        std::fs::write(&toml_path, toml_content).unwrap();

        let loaded = AppConfig::from_file(&toml_path).expect("loaded toml");
        assert_eq!(loaded.server.bind_addr, "127.0.0.1:3000");

        // 2. JSON file
        let json_path = dir.join("tealdrive.json");
        let json_content = serde_json::to_string(&AppConfig::default()).unwrap();
        std::fs::write(&json_path, json_content).unwrap();

        let loaded_json = AppConfig::from_file(&json_path).expect("loaded json");
        assert_eq!(loaded_json.server.bind_addr, "127.0.0.1:3000");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn validate_rejects_empty_roots() {
        let mut config = AppConfig::default();
        config.roots.clear();
        assert_eq!(config.validate(), Err(TealDriveError::Validation));
    }

    #[test]
    fn validate_rejects_invalid_bind_addr() {
        let mut config = AppConfig::default();
        config.server.bind_addr = "not-an-ip-or-port".to_owned();
        assert_eq!(config.validate(), Err(TealDriveError::Validation));
    }

    #[test]
    fn dynamic_config_patch_and_hot_reload_test() {
        let dir = std::env::temp_dir().join(format!("tealdrive-dyn-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg_file = dir.join("config.toml");
        let initial_cfg = AppConfig::default();
        std::fs::write(&cfg_file, toml::to_string(&initial_cfg).unwrap()).unwrap();

        let dynamic = DynamicConfig::new(initial_cfg, Some(cfg_file.clone()));
        assert!(!dynamic.load().features.delete_permanently);

        // Apply patch in memory
        let patch = ConfigPatch {
            features: Some(
                FeatureFlags {
                    delete_permanently: true,
                    ..Default::default()
                }
                .into(),
            ),
            trash_retention_days: Some(Some(14)),
            ..Default::default()
        };
        dynamic.apply_patch(patch).expect("apply patch");
        assert!(dynamic.load().features.delete_permanently);
        assert_eq!(dynamic.load().trash_retention_days, Some(14));

        // Save to file
        dynamic.save_to_file().expect("save to file");

        // External modification on disk
        let mut on_disk: AppConfig =
            toml::from_str(&std::fs::read_to_string(&cfg_file).unwrap()).unwrap();
        on_disk.trash_retention_days = Some(30);
        std::fs::write(&cfg_file, toml::to_string(&on_disk).unwrap()).unwrap();

        // Hot reload from disk
        dynamic.reload().expect("reload from disk");
        assert_eq!(dynamic.load().trash_retention_days, Some(30));

        let _ = std::fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    #[test]
    fn partial_updates_preserve_siblings_and_null_clears_optionals() {
        let mut config = AppConfig::default();
        config.features.chmod_file = true;
        config.session.secure_cookies = false;
        config.trash_retention_days = Some(30);
        config.server.static_dir = Some("/tmp/assets".into());
        let patch:ConfigPatch=serde_json::from_str(r#"{"features":{"save_text_file":true},"session":{"idle_timeout_seconds":60},"trash_retention_days":null,"static_dir":null}"#).unwrap();
        patch.apply_to(&mut config);
        assert!(config.features.chmod_file);
        assert!(config.features.save_text_file);
        assert!(!config.session.secure_cookies);
        assert_eq!(config.session.idle_timeout_seconds, 60);
        assert_eq!(config.trash_retention_days, None);
        assert_eq!(config.server.static_dir, None);
        assert!(serde_json::from_str::<ConfigPatch>(r#"{"features":{"upload":true}}"#).is_err());
        assert!(toml::from_str::<AppConfig>("unknown_setting = true").is_err());
    }
    #[test]
    fn administrator_membership_is_explicit_and_case_sensitive() {
        let mut policy = AccountPolicyConfig::default();
        assert!(!policy.is_admin("alice"));
        policy.admin_users = vec!["Alice".into()];
        assert!(policy.is_admin("Alice"));
        assert!(!policy.is_admin("alice"));
    }
    #[test]
    fn atomic_json_save_roundtrips_and_failure_preserves_live_config() {
        let dir = std::env::temp_dir().join(format!("tdrv-config-save-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("config.json");
        let state = DynamicConfig::new(AppConfig::default(), Some(path.clone()));
        let patch: ConfigPatch =
            serde_json::from_str(r#"{"features":{"chmod_file":true}}"#).unwrap();
        state.apply_patch_saved(patch.clone(), true).unwrap();
        assert!(AppConfig::from_file(&path).unwrap().features.chmod_file);
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let failed =
            DynamicConfig::new(AppConfig::default(), Some(dir.join("missing/config.toml")));
        assert!(failed.apply_patch_saved(patch, true).is_err());
        assert!(!failed.load().features.chmod_file);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn nested_hidden_paths_and_invalid_limits_are_rejected() {
        assert!(crate::fs::path::is_hidden_relative(&RelativePath::new(
            "visible/.private/file"
        )));
        assert!(!crate::fs::path::is_hidden_relative(&RelativePath::new(
            "visible/file.txt"
        )));
        let mut config = AppConfig::default();
        config.limits.max_text_edit_size = 1024 * 1024;
        assert!(config.validate().is_err());
        config = AppConfig::default();
        config.security.allowed_origins = vec!["*".into()];
        assert!(config.validate().is_err());
        config = AppConfig::default();
        config.roots.push(config.roots[0].clone());
        assert!(config.validate().is_err());
    }
}
