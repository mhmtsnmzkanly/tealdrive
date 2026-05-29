use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub id: String,
    pub name: String,
    pub entry_type: EntryType,
    pub size: u64,
    pub modified: u64,
    pub permissions: u32,
    pub owner: String,
    pub group: String,
    pub mime: String,
    pub is_sensitive: bool,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EntryType {
    File,
    Dir,
    Symlink,
}
