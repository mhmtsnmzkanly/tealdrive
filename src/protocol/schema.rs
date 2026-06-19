use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySet {
    pub can_list: bool,
    pub can_download: bool,
    pub can_upload: bool,
    pub can_create_folder: bool,
    pub can_rename: bool,
    pub can_move_to_trash: bool,
    pub can_restore_from_trash: bool,
}

impl Default for CapabilitySet {
    fn default() -> Self {
        Self {
            can_list: true,
            can_download: true,
            can_upload: true,
            can_create_folder: true,
            can_rename: true,
            can_move_to_trash: true,
            can_restore_from_trash: true,
        }
    }
}

pub type FileCapabilities = CapabilitySet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileKind {
    File,
    Directory,
    Symlink,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileFilter {
    All,
    Folders,
    Code,
    Images,
    Media,
    Documents,
    Archives,
    Hidden,
    Writable,
    Symlinks,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortMode {
    #[default]
    NameAsc,
    NameDesc,
    ModifiedAsc,
    ModifiedDesc,
    SizeAsc,
    SizeDesc,
    TypeAsc,
    TypeDesc,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilterMode {
    pub query: Option<String>,
    pub file_kind: Option<FileKind>,
    pub filter: Option<FileFilter>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnixMode(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerName(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupName(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryCursor(pub String);
