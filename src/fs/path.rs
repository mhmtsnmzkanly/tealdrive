use crate::config::{RelativePath, RootId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathRequest {
    pub root_id: RootId,
    pub relative_path: RelativePath,
}
