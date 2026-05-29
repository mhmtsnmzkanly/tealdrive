use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootConfig {
    pub id: String,
    pub path: PathBuf,
    pub name: String,
    pub readonly: bool,
}

pub struct RootPolicy {
    pub roots: Vec<RootConfig>,
}

impl RootPolicy {
    pub fn new(username: &str) -> Self {
        let mut roots = Vec::new();
        
        // Default home root
        roots.push(RootConfig {
            id: "home".to_string(),
            path: PathBuf::from(format!("/home/{}", username)),
            name: "Home".to_string(),
            readonly: false,
        });

        Self { roots }
    }

    pub fn is_allowed(&self, path: &Path) -> bool {
        self.roots.iter().any(|root| path.starts_with(&root.path))
    }

    pub fn get_root_for_path(&self, path: &Path) -> Option<&RootConfig> {
        self.roots.iter().find(|root| path.starts_with(&root.path))
    }
}
