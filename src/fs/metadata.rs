use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetadata {
    pub name: String,
    pub size: u64,
    pub is_hidden: bool,
    pub is_sensitive: bool,
    pub is_symlink: bool,
}
