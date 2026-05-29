use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{AllowedRoot, RelativePath, RootId};
use crate::errors::TealDriveError;
use crate::fs::filename::SafeFileName;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathRequest {
    pub root_id: RootId,
    pub relative_path: RelativePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathValidationError {
    InvalidRootId,
    AbsolutePathRejected,
    TraversalRejected,
    NullByteRejected,
    InvalidFilename,
    ReservedNameRejected,
    PathEscapesRoot,
    SymlinkDenied,
    SymlinkEscape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRoot {
    pub root_id: RootId,
    pub canonical_base: PathBuf,
    pub root: AllowedRoot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingPathResolution {
    pub root: ResolvedRoot,
    pub relative_path: RelativePath,
    pub resolved_path: PathBuf,
    pub is_symlink: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTargetResolution {
    pub root: ResolvedRoot,
    pub parent_relative_path: RelativePath,
    pub filename: SafeFileName,
    pub resolved_parent: PathBuf,
    pub resolved_path: PathBuf,
}

pub fn validate_relative_path(path: &RelativePath) -> Result<RelativePath, TealDriveError> {
    let value = path.0.trim_matches('/');
    if path.0.contains('\0') {
        return Err(TealDriveError::NullByteRejected);
    }
    let raw_path = Path::new(&path.0);
    if raw_path.is_absolute() {
        return Err(TealDriveError::AbsolutePathRejected);
    }

    let mut parts = Vec::new();
    for component in Path::new(value).components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir => return Err(TealDriveError::TraversalRejected),
            Component::RootDir | Component::Prefix(_) => {
                return Err(TealDriveError::AbsolutePathRejected)
            }
        }
    }

    Ok(RelativePath(parts.join("/")))
}

pub fn is_hidden_relative(path: &RelativePath) -> bool {
    validate_relative_path(path)
        .ok()
        .and_then(|validated| {
            validated
                .0
                .split('/')
                .find(|part| !part.is_empty())
                .map(|part| part.starts_with('.'))
        })
        .unwrap_or(false)
}

pub fn resolve_root(root: &AllowedRoot) -> Result<ResolvedRoot, TealDriveError> {
    let canonical_base = root
        .base_path
        .canonicalize()
        .map_err(|_| TealDriveError::InvalidRootId)?;
    Ok(ResolvedRoot {
        root_id: root.root_id.clone(),
        canonical_base,
        root: root.clone(),
    })
}

pub fn resolve_existing_path(
    root: &AllowedRoot,
    relative_path: &RelativePath,
    follow_symlinks: bool,
) -> Result<ExistingPathResolution, TealDriveError> {
    let resolved_root = resolve_root(root)?;
    let validated = validate_relative_path(relative_path)?;
    let candidate = if validated.0.is_empty() {
        resolved_root.canonical_base.clone()
    } else {
        resolved_root.canonical_base.join(&validated.0)
    };

    let symlink_meta =
        std::fs::symlink_metadata(&candidate).map_err(|_| TealDriveError::InvalidPath)?;
    let is_symlink = symlink_meta.file_type().is_symlink();
    if is_symlink && !follow_symlinks {
        return Err(TealDriveError::SymlinkDenied);
    }

    let canonical = candidate
        .canonicalize()
        .map_err(|_| TealDriveError::InvalidPath)?;
    if !canonical.starts_with(&resolved_root.canonical_base) {
        return Err(if is_symlink {
            TealDriveError::SymlinkEscape
        } else {
            TealDriveError::PathEscapesRoot
        });
    }

    Ok(ExistingPathResolution {
        root: resolved_root,
        relative_path: validated,
        resolved_path: canonical,
        is_symlink,
    })
}

pub fn resolve_new_target(
    root: &AllowedRoot,
    parent_relative_path: &RelativePath,
    filename: &str,
) -> Result<NewTargetResolution, TealDriveError> {
    let resolved_root = resolve_root(root)?;
    let parent_relative_path = validate_relative_path(parent_relative_path)?;
    let filename = SafeFileName::parse(filename)?;
    let parent = if parent_relative_path.0.is_empty() {
        resolved_root.canonical_base.clone()
    } else {
        resolved_root.canonical_base.join(&parent_relative_path.0)
    };
    let resolved_parent = parent
        .canonicalize()
        .map_err(|_| TealDriveError::InvalidPath)?;
    if !resolved_parent.starts_with(&resolved_root.canonical_base) {
        return Err(TealDriveError::PathEscapesRoot);
    }

    let resolved_path = resolved_parent.join(&filename.0);
    if !resolved_path.starts_with(&resolved_root.canonical_base) {
        return Err(TealDriveError::PathEscapesRoot);
    }

    Ok(NewTargetResolution {
        root: resolved_root,
        parent_relative_path,
        filename,
        resolved_parent,
        resolved_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    fn temp_root() -> PathBuf {
        let path = std::env::temp_dir().join(format!("tealdrive-policy-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temp root");
        path
    }

    fn root(path: PathBuf) -> AllowedRoot {
        AllowedRoot {
            root_id: RootId::new("test"),
            base_path: path,
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
        }
    }

    #[test]
    fn absolute_path_rejected() {
        assert_eq!(
            validate_relative_path(&RelativePath::new("/etc/passwd")),
            Err(TealDriveError::AbsolutePathRejected)
        );
    }

    #[test]
    fn traversal_rejected() {
        assert_eq!(
            validate_relative_path(&RelativePath::new("../etc")),
            Err(TealDriveError::TraversalRejected)
        );
    }

    #[test]
    fn nul_byte_rejected() {
        assert_eq!(
            validate_relative_path(&RelativePath::new("bad\0path")),
            Err(TealDriveError::NullByteRejected)
        );
    }

    #[test]
    fn empty_relative_path_allowed_as_root() {
        assert_eq!(
            validate_relative_path(&RelativePath::new("")),
            Ok(RelativePath::new(""))
        );
    }

    #[test]
    fn normal_relative_path_accepted() {
        assert_eq!(
            validate_relative_path(&RelativePath::new("docs/file.txt")),
            Ok(RelativePath::new("docs/file.txt"))
        );
    }

    #[test]
    fn repeated_separators_are_normalized() {
        assert_eq!(
            validate_relative_path(&RelativePath::new("docs//file.txt")),
            Ok(RelativePath::new("docs/file.txt"))
        );
    }

    #[test]
    fn canonical_path_inside_root_accepted() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"ok").expect("file");
        let resolution =
            resolve_existing_path(&root(base.clone()), &RelativePath::new("file.txt"), false)
                .expect("resolved");

        assert!(resolution
            .resolved_path
            .starts_with(base.canonicalize().unwrap()));
    }

    #[test]
    fn new_target_parent_canonicalization_works() {
        let base = temp_root();
        fs::create_dir_all(base.join("docs")).expect("docs");
        let resolution =
            resolve_new_target(&root(base.clone()), &RelativePath::new("docs"), "new.txt")
                .expect("target");

        assert_eq!(resolution.filename, SafeFileName("new.txt".to_owned()));
        assert!(resolution.resolved_path.ends_with("new.txt"));
    }

    #[test]
    fn new_target_traversal_rejected() {
        let base = temp_root();
        assert_eq!(
            resolve_new_target(&root(base), &RelativePath::new(".."), "new.txt"),
            Err(TealDriveError::TraversalRejected)
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_rejected_when_following_enabled() {
        let base = temp_root();
        let outside = temp_root();
        fs::write(outside.join("secret"), b"secret").expect("secret");
        symlink(outside.join("secret"), base.join("link")).expect("symlink");

        assert_eq!(
            resolve_existing_path(&root(base), &RelativePath::new("link"), true),
            Err(TealDriveError::SymlinkEscape)
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_denied_by_default() {
        let base = temp_root();
        fs::write(base.join("target"), b"ok").expect("target");
        symlink(base.join("target"), base.join("link")).expect("symlink");

        assert_eq!(
            resolve_existing_path(&root(base), &RelativePath::new("link"), false),
            Err(TealDriveError::SymlinkDenied)
        );
    }
}
