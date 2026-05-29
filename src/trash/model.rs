use serde::{Deserialize, Serialize};

use crate::config::{RelativePath, RootId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrashEntry {
    pub original_root_id: RootId,
    pub original_relative_path: RelativePath,
    pub trash_relative_path: RelativePath,
    pub username: String,
    pub uid: u32,
}
