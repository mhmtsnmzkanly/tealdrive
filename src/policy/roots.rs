use crate::config::{AllowedRoot, RootId};

pub fn find_allowed_root<'a>(
    roots: &'a [AllowedRoot],
    root_id: &RootId,
) -> Option<&'a AllowedRoot> {
    roots.iter().find(|root| &root.root_id == root_id)
}
