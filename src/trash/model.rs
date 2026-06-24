use serde::{Deserialize, Serialize};

use crate::config::{RelativePath, RootId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrashEntry {
    pub trash_id: String,
    pub original_root_id: RootId,
    pub original_relative_path: RelativePath,
    pub trash_relative_path: RelativePath,
    pub display_name: String,
    pub deleted_at: u64,
    pub username: String,
    pub uid: u32,
}
