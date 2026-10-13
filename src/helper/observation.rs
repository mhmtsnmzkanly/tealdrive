//! Bounded expected-state fingerprints, observed only after permanent user privilege drop.
use super::types::{HelperCommand, HelperRequest, HelperResponse, HelperResponsePayload};
use crate::config::{AllowedRoot, AppConfig, RelativePath};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read, Seek},
    os::unix::fs::MetadataExt,
};
const MAX_NODES: usize = 10000;
const MAX_BYTES: u64 = 64 * 1024 * 1024;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileState {
    pub digest: String,
    pub device: String,
    pub inode: String,
    pub kind: String,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub links: u64,
    pub mtime_seconds: i64,
    pub mtime_nanos: i64,
    pub ctime_seconds: i64,
    pub ctime_nanos: i64,
    pub content_digest: Option<String>,
    pub tree_digest: Option<String>,
}
impl FileState {
    pub(super) fn metadata(metadata: &std::fs::Metadata) -> Self {
        Self {
            digest: String::new(),
            device: metadata.dev().to_string(),
            inode: metadata.ino().to_string(),
            kind: if metadata.is_file() {
                "file"
            } else if metadata.is_dir() {
                "directory"
            } else {
                "other"
            }
            .into(),
            mode: metadata.mode() & 0o7777,
            uid: metadata.uid(),
            gid: metadata.gid(),
            size: metadata.len(),
            links: metadata.nlink(),
            mtime_seconds: metadata.mtime(),
            mtime_nanos: metadata.mtime_nsec(),
            ctime_seconds: metadata.ctime(),
            ctime_nanos: metadata.ctime_nsec(),
            content_digest: None,
            tree_digest: None,
        }
    }
    pub(super) fn finish(mut self) -> io::Result<Self> {
        self.digest.clear();
        self.digest = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&self).map_err(io::Error::other)?)
        );
        Ok(self)
    }
    pub(super) fn same_metadata(&self, other: &Self) -> bool {
        let mut a = self.clone();
        let mut b = other.clone();
        for state in [&mut a, &mut b] {
            state.digest.clear();
            state.content_digest = None;
            state.tree_digest = None;
        }
        a == b
    }
    pub fn same_inode(&self, other: &Self) -> bool {
        self.device == other.device && self.inode == other.inode && self.kind == other.kind
    }
    pub fn fully_observed(&self) -> bool {
        match self.kind.as_str() {
            "file" => self.content_digest.is_some(),
            "directory" => self.tree_digest.is_some(),
            _ => false,
        }
    }
}
#[derive(Default)]
struct Budget {
    nodes: usize,
    bytes: u64,
}
pub(super) fn capture_readable_file(file: &std::fs::File) -> io::Result<FileState> {
    let before = file.metadata()?;
    let mut state = FileState::metadata(&before);
    if !before.is_file() || before.len() > MAX_BYTES {
        return state.finish();
    }
    let mut file = file.try_clone()?;
    file.rewind()?;
    let mut hash = Sha256::new();
    let mut buffer = zeroize::Zeroizing::new(vec![0u8; 65536]);
    let mut bytes = 0u64;
    loop {
        super::cancellation::check()?;
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes += count as u64;
        if bytes > before.len() {
            return Err(io::Error::other("file grew during observation"));
        }
        hash.update(&buffer[..count]);
    }
    if FileState::metadata(&file.metadata()?) != state || bytes != before.len() {
        return Err(io::Error::other("file changed during observation"));
    }
    state.content_digest = Some(format!("{:x}", hash.finalize()));
    state.finish()
}
pub fn capture(root: &AllowedRoot, path: &RelativePath) -> io::Result<FileState> {
    capture_inner(root, path, &mut Budget::default(), 0)
}
fn capture_inner(
    root: &AllowedRoot,
    path: &RelativePath,
    budget: &mut Budget,
    depth: usize,
) -> io::Result<FileState> {
    super::cancellation::check()?;
    budget.nodes += 1;
    if budget.nodes > MAX_NODES || depth > 64 {
        return Err(io::Error::other("observation limit"));
    }
    let pinned = super::sandbox::open(root, path, libc::O_PATH, 0)?;
    let metadata = pinned.metadata()?;
    let mut state = FileState::metadata(&metadata);
    if metadata.is_file() {
        if budget
            .bytes
            .checked_add(metadata.len())
            .is_none_or(|bytes| bytes > MAX_BYTES)
        {
            return state.finish();
        }
        if let Ok(file) = super::sandbox::open(root, path, libc::O_RDONLY | libc::O_NONBLOCK, 0) {
            if FileState::metadata(&file.metadata()?) != state {
                return Err(io::Error::other("file changed before observation"));
            }
            let observed = capture_readable_file(&file)?;
            if !state.same_metadata(&observed) {
                return Err(io::Error::other("file changed during observation"));
            }
            budget.bytes += metadata.len();
            state.content_digest = observed.content_digest;
        }
    } else if metadata.is_dir() {
        let tree = (|| {
            super::access::check(path, crate::policy::access::Access::Read)
                .map_err(io::Error::other)?;
            let mut names = Vec::new();
            for entry in std::fs::read_dir(root.base_path.join(&path.0))? {
                if names.len() + budget.nodes >= MAX_NODES {
                    return Err(io::Error::other("observation limit"));
                }
                names.push(
                    entry?
                        .file_name()
                        .into_string()
                        .map_err(|_| io::Error::other("non-UTF8 entry"))?,
                );
            }
            names.sort();
            let mut hash = Sha256::new();
            for name in names {
                let child = RelativePath::new(if path.0.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", path.0, name)
                });
                let child = capture_inner(root, &child, budget, depth + 1)?;
                if !child.fully_observed() {
                    return Err(io::Error::other("incomplete child state"));
                }
                hash.update(name.as_bytes());
                hash.update([0]);
                hash.update(child.digest.as_bytes());
                hash.update([0]);
            }
            if FileState::metadata(&pinned.metadata()?) != state {
                return Err(io::Error::other("directory changed during observation"));
            }
            Ok(format!("{:x}", hash.finalize()))
        })();
        state.tree_digest = tree.ok();
    }
    state.finish()
}
pub fn execute(request: &HelperRequest, config: &AppConfig) -> HelperResponse {
    use HelperCommand::*;
    if !config.journal.enabled || !matches!(request.command, Rename | Move | MoveToTrash | Chmod) {
        return super::transaction::execute(request, config);
    }
    let Some(root) = config
        .roots
        .iter()
        .find(|root| root.root_id == request.root_id)
    else {
        return HelperResponse::permission_denied(request, "Unknown root.");
    };
    // Chmod does not overwrite contents: pinned inode metadata is its expected state.
    if request.command == Chmod {
        return super::transaction::execute(request, config);
    }
    let before = capture(root, &request.relative_path).ok();
    if super::cancellation::check().is_err() {
        return HelperResponse::permission_denied(request, "Operation cancelled before execution.");
    }
    let mut response = super::transaction::execute(request, config);
    let target = match request.command {
        Rename => request.target_relative_path.as_ref().map(|name| {
            RelativePath::new(match request.relative_path.0.rsplit_once('/') {
                Some((parent, _)) if !parent.is_empty() => format!("{parent}/{}", name.0),
                _ => name.0.clone(),
            })
        }),
        Move => request.target_relative_path.clone(),
        MoveToTrash => match &response.payload {
            HelperResponsePayload::TrashEntry(entry) => Some(RelativePath::new(format!(
                ".tealdrive-trash/files/{}",
                entry.trash_id
            ))),
            _ => None,
        },
        Chmod => Some(request.relative_path.clone()),
        _ => None,
    };
    let after = if response.success {
        target.as_ref().and_then(|target| {
            if request.command == MoveToTrash {
                super::access::with_alias(target.clone(), request.relative_path.clone(), || {
                    capture(root, target).ok()
                })
            } else {
                capture(root, target).ok()
            }
        })
    } else {
        None
    };
    if let Some(metadata) = response.audit_metadata.as_mut() {
        metadata.before = before;
        metadata.after = after;
    }
    response
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deep_content_and_same_size_edits_change_expected_state() {
        let home = std::env::temp_dir().join(format!("td-state-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&home).unwrap();
        std::fs::create_dir(home.join("nested")).unwrap();
        std::fs::write(home.join("nested/file"), b"one").unwrap();
        let root = AllowedRoot {
            base_path: home.clone(),
            ..Default::default()
        };
        let first = capture(&root, &RelativePath::new("nested")).unwrap();
        assert!(first.fully_observed());
        std::fs::write(home.join("nested/file"), b"two").unwrap();
        let second = capture(&root, &RelativePath::new("nested")).unwrap();
        assert_ne!(first.digest, second.digest);
        std::fs::remove_file(home.join("nested/file")).unwrap();
        std::fs::remove_dir(home.join("nested")).unwrap();
        std::fs::remove_dir(home).unwrap();
    }
    #[test]
    fn hidden_descendant_is_never_read_for_tree_fingerprint() {
        let home = std::env::temp_dir().join(format!("td-hidden-state-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&home).unwrap();
        std::fs::create_dir(home.join(".ssh")).unwrap();
        std::fs::write(home.join(".ssh/key"), b"hidden credential").unwrap();
        let root = AllowedRoot {
            base_path: home.clone(),
            ..Default::default()
        };
        let config = AppConfig {
            roots: vec![root.clone()],
            ..Default::default()
        };
        let mut request =
            super::super::ipc::tests_support::sample_request(HelperCommand::FileMetadata);
        request.relative_path = RelativePath::new("");
        super::super::access::with_context(&request, &config, || {
            assert!(capture(&root, &RelativePath::new(".ssh/key")).is_err());
            let state = capture(&root, &RelativePath::new("")).unwrap();
            assert!(state.tree_digest.is_none());
            assert!(!state.fully_observed());
        });
        std::fs::remove_file(home.join(".ssh/key")).unwrap();
        std::fs::remove_dir(home.join(".ssh")).unwrap();
        std::fs::remove_dir(home).unwrap();
    }
}
