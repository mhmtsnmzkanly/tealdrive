use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub entry_type: EntryType,
    pub size: u64,
    pub modified: u64,
    pub owner: String,
    pub group: String,
    pub mode: u32,
    pub permissions: String, // String representation e.g. "rw-r--r--"
    pub capabilities: Vec<String>,
    pub is_sensitive: bool,
    pub is_protected: bool,
    pub is_hidden: bool,
    pub is_symlink: bool,
    pub symlink_target: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EntryType {
    File,
    Dir,
    Symlink,
}
