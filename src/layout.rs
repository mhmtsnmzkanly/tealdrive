//! Standard installation paths. Privileged helper locations never come from an environment variable.
use std::path::{Path, PathBuf};
pub const HOME: &str = "/home/tealdrive";
pub const HELPER_CONFIG: &str = "/home/tealdrive/policy/helper.toml";
pub const HELPER_KEY: &str = "/home/tealdrive/policy/helper.key";
pub const LEGACY_HELPER_CONFIG: &str = "/etc/tealdrive/helper.toml";
pub const LEGACY_HELPER_KEY: &str = "/etc/tealdrive/helper.key";
pub const SERVER_CONFIG: &str = "/home/tealdrive/policy/server.toml";
pub const SETTINGS: &str = "/home/tealdrive/config/settings.toml";
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
// Only absence selects legacy. A present corrupt/symlink/unreadable primary must fail closed.
fn preferred(primary: &Path, legacy: &Path) -> PathBuf {
    match std::fs::symlink_metadata(primary) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => legacy.to_path_buf(),
        _ => primary.to_path_buf(),
    }
}
pub fn trusted_helper_paths() -> (PathBuf, PathBuf) {
    let config = preferred(Path::new(HELPER_CONFIG), Path::new(LEGACY_HELPER_CONFIG));
    let key = if config == Path::new(HELPER_CONFIG) {
        HELPER_KEY
    } else {
        LEGACY_HELPER_KEY
    };
    (config, key.into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corrupt_or_symlink_primary_is_never_a_legacy_fallback() {
        let dir = std::env::temp_dir().join(format!("tealdrive-layout-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let primary = dir.join("helper.toml");
        let legacy = dir.join("old.toml");
        std::fs::write(&legacy, "valid legacy").unwrap();
        assert_eq!(preferred(&primary, &legacy), legacy);
        std::fs::write(&primary, "corrupt").unwrap();
        assert_eq!(preferred(&primary, &legacy), primary);
        std::fs::remove_file(&primary).unwrap();
        std::os::unix::fs::symlink(dir.join("missing"), &primary).unwrap();
        assert_eq!(preferred(&primary, &legacy), primary);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
