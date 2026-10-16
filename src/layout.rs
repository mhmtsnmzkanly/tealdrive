//! Standard installation paths. Privileged helper locations never come from an environment variable.
use std::path::PathBuf;
pub const HOME: &str = "/home/tealdrive";
pub const HELPER_CONFIG: &str = "/home/tealdrive/policy/helper.toml";
pub const HELPER_KEY: &str = "/home/tealdrive/policy/helper.key";
pub const SERVER_CONFIG: &str = "/home/tealdrive/policy/server.toml";
pub const SETTINGS: &str = "/home/tealdrive/config/settings.toml";
pub const AUDIT_LOG: &str = "/home/tealdrive/logs/server/audit.jsonl";
pub const ACCOUNTS: &str = "/home/tealdrive/auth/accounts.db";
pub const VIRTUAL_HOMES: &str = "/home/tealdrive/virtual-homes";
#[derive(Debug, Clone)]
pub struct Layout {
    pub home: PathBuf,
}
impl Default for Layout {
    fn default() -> Self {
        Self { home: HOME.into() }
    }
}
impl Layout {
    pub fn bin(&self) -> PathBuf {
        self.home.join("bin")
    }
    pub fn policy(&self) -> PathBuf {
        self.home.join("policy")
    }
    pub fn auth(&self) -> PathBuf {
        self.home.join("auth")
    }
    pub fn settings(&self) -> PathBuf {
        self.home.join("config/settings.toml")
    }
    pub fn server_config(&self) -> PathBuf {
        self.policy().join("server.toml")
    }
    pub fn helper_config(&self) -> PathBuf {
        self.policy().join("helper.toml")
    }
    pub fn helper_key(&self) -> PathBuf {
        self.policy().join("helper.key")
    }
    pub fn accounts(&self) -> PathBuf {
        self.auth().join("accounts.db")
    }
    pub fn server_log(&self) -> PathBuf {
        self.home.join("logs/server/server.jsonl")
    }
    pub fn virtual_homes(&self) -> PathBuf {
        self.home.join("virtual-homes")
    }
}
/// The helper's root-owned policy and key. There is no fallback location.
pub fn trusted_helper_paths() -> (PathBuf, PathBuf) {
    (HELPER_CONFIG.into(), HELPER_KEY.into())
}
