use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::limits::Limits;
use crate::policy::feature_gate::FeatureFlags;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RootId(pub String);

impl RootId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelativePath(pub String);

impl RelativePath {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind_addr: String,
    pub behind_tls_proxy: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:3000".to_owned(),
            behind_tls_proxy: true,
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
                "id_rsa".to_owned(),
                "id_ed25519".to_owned(),
                "*.pem".to_owned(),
                "*.key".to_owned(),
            ],
            warn_read_patterns: vec![".env".to_owned(), "*.pem".to_owned(), "*.key".to_owned()],
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
                "cgi".to_owned(),
                "pl".to_owned(),
                "py".to_owned(),
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
    }
}
