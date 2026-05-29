use crate::config::{AllowedRoot, RootId};
use crate::errors::TealDriveError;

pub fn find_allowed_root<'a>(
    roots: &'a [AllowedRoot],
    root_id: &RootId,
) -> Option<&'a AllowedRoot> {
    roots.iter().find(|root| &root.root_id == root_id)
}

pub fn require_allowed_root<'a>(
    roots: &'a [AllowedRoot],
    root_id: &RootId,
) -> Result<&'a AllowedRoot, TealDriveError> {
    find_allowed_root(roots, root_id).ok_or(TealDriveError::InvalidRootId)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn unknown_root_id_rejected() {
        let roots = vec![AllowedRoot {
            root_id: RootId::new("home"),
            base_path: PathBuf::from("/home"),
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
        }];

        assert_eq!(
            require_allowed_root(&roots, &RootId::new("missing")),
            Err(TealDriveError::InvalidRootId)
        );
    }
}
