use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
pub struct AppConfig {
    pub server: ServerConfig,
    pub session: SessionConfig,
    pub security: SecurityConfig,
    pub roots: Vec<AllowedRoot>,
    pub sensitive_policy: SensitivePolicyConfig,
    pub webroot_policy: WebRootPolicyConfig,
    pub account_policy: AccountPolicyConfig,
    pub features: FeatureFlags,
    pub limits: Limits,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            session: SessionConfig::default(),
            security: SecurityConfig::default(),
            roots: vec![AllowedRoot::default()],
            sensitive_policy: SensitivePolicyConfig::default(),
            webroot_policy: WebRootPolicyConfig::default(),
            account_policy: AccountPolicyConfig::default(),
            features: FeatureFlags::default(),
            limits: Limits::default(),
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

    pub fn load_or_default(explicit_path: Option<&Path>) -> Result<Self, TealDriveError> {
        if let Some(path) = explicit_path {
            return Self::from_file(path);
        }

        if let Ok(env_path) = std::env::var("TEALDRIVE_CONFIG") {
            let trimmed = env_path.trim();
            if !trimmed.is_empty() {
                return Self::from_file(Path::new(trimmed));
            }
        }

        let candidates = [
            "tealdrive.toml",
            "config.toml",
            "/etc/tealdrive/config.toml",
            "/etc/tealdrive/tealdrive.toml",
        ];

        for candidate in &candidates {
            let path = Path::new(candidate);
            if path.exists() {
                return Self::from_file(path);
            }
        }

        let default_config = Self::default();
        default_config.validate()?;
        Ok(default_config)
    }

    pub fn validate(&self) -> Result<(), TealDriveError> {
        if self.roots.is_empty() {
            return Err(TealDriveError::Validation);
        }
        if self.server.bind_addr.parse::<std::net::SocketAddr>().is_err() {
            return Err(TealDriveError::Validation);
        }
        if self.limits.max_chunk_size == 0 || self.limits.max_text_edit_size == 0 {
            return Err(TealDriveError::Validation);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
pub struct SecurityConfig {
    pub production_wire_protocol: WireProtocol,
    pub allow_plain_ws_localhost_only: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            production_wire_protocol: WireProtocol::Tdrv1Binary,
            allow_plain_ws_localhost_only: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireProtocol {
    Tdrv1Binary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllowedRoot {
    pub root_id: RootId,
    pub base_path: PathBuf,
    pub read_only: bool,
    pub uploads_allowed: bool,
    pub hidden_files_allowed: bool,
    pub is_web_root: bool,
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
pub struct AccountPolicyConfig {
    pub reject_root: bool,
    pub reject_system_accounts: bool,
    pub reject_disabled_shells: bool,
    pub minimum_regular_uid: u32,
}

impl Default for AccountPolicyConfig {
    fn default() -> Self {
        Self {
            reject_root: true,
            reject_system_accounts: true,
            reject_disabled_shells: true,
            minimum_regular_uid: 1000,
        }
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
}
