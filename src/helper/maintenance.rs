//! Bounded expected-state cleanup. This module runs after permanent user ID drop.
use crate::{
    accounts::{
        journal::OperationRecord,
        store::{Account, StoreError},
    },
    config::{AllowedRoot, AppConfig, RelativePath},
    helper::observation::FileState,
};
use std::{io, os::fd::AsRawFd};
struct Node {
    path: RelativePath,
    state: FileState,
}
fn manifest(
    root: &AllowedRoot,
    path: &RelativePath,
    nodes: &mut Vec<Node>,
    depth: usize,
    uid: u32,
    device: &str,
) -> io::Result<()> {
    if depth > 64 || nodes.len() >= 10000 {
        return Err(io::Error::other("maintenance bound"));
    }
    let file = super::sandbox::open(root, path, libc::O_PATH, 0)?;
    let state = FileState::metadata(&file.metadata()?).finish()?;
    if state.uid != uid
        || state.device != device
        || !["file", "directory"].contains(&state.kind.as_str())
    {
        return Err(io::Error::from(io::ErrorKind::PermissionDenied));
    }
    let directory = state.kind == "directory";
    nodes.push(Node {
        path: path.clone(),
        state,
    });
    if directory {
        for entry in std::fs::read_dir(root.base_path.join(&path.0))? {
            let name = entry?
                .file_name()
                .into_string()
                .map_err(|_| io::Error::other("non UTF8 entry"))?;
            manifest(
                root,
                &RelativePath::new(format!("{}/{}", path.0, name)),
                nodes,
                depth + 1,
                uid,
                device,
            )?;
        }
    }
    Ok(())
}
fn remove_nodes(
    root: &AllowedRoot,
    nodes: Vec<Node>,
    logical_parent: &RelativePath,
) -> io::Result<()> {
    for node in nodes.into_iter().rev() {
        let pinned = super::sandbox::open(root, &node.path, libc::O_PATH, 0)?;
        let current = FileState::metadata(&pinned.metadata()?).finish()?;
        let directory = node.state.kind == "directory";
        if if directory {
            !current.same_inode(&node.state)
                || current.uid != node.state.uid
                || current.gid != node.state.gid
                || current.mode != node.state.mode
        } else {
            !current.same_metadata(&node.state)
        } {
            return Err(io::Error::other("maintenance target changed"));
        }
        let (parent, name) = node
            .path
            .0
            .rsplit_once('/')
            .ok_or_else(|| io::Error::other("private target required"))?;
        let open = || {
            super::sandbox::open(
                root,
                &RelativePath::new(parent),
                libc::O_PATH | libc::O_DIRECTORY,
                0,
            )
        };
        let parent = if parent == ".tealdrive-trash/files" {
            super::access::with_alias(RelativePath::new(parent), logical_parent.clone(), open)?
        } else {
            open()?
        };
        let name = std::ffi::CString::new(name).map_err(io::Error::other)?;
        if unsafe {
            libc::unlinkat(
                parent.as_raw_fd(),
                name.as_ptr(),
                if directory { libc::AT_REMOVEDIR } else { 0 },
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}
pub(crate) fn expire_trash(
    config: &AppConfig,
    actor: &Account,
    root: &AllowedRoot,
    operation: &OperationRecord,
    path: &RelativePath,
) -> Result<(), StoreError> {
    if unsafe { libc::geteuid() } == 0 || unsafe { libc::getuid() } != actor.uid {
        return Err(StoreError::Forbidden);
    }
    let expected = operation.after.as_ref().ok_or(StoreError::InvalidInput)?;
    for action in [
        crate::policy::access::Access::Discover,
        crate::policy::access::Access::Read,
        crate::policy::access::Access::Delete,
    ] {
        config
            .access_policy
            .check(
                &actor.username,
                &root.root_id,
                &operation.source_path,
                action,
            )
            .map_err(|_| StoreError::Forbidden)?;
    }
    let request = super::types::HelperRequest {
        request_id: crate::protocol::RequestId::new(),
        username: actor.username.clone(),
        uid: actor.uid,
        gid: actor.gid,
        command: super::types::HelperCommand::DeletePermanently,
        root_id: root.root_id.clone(),
        relative_path: operation.source_path.clone(),
        target_relative_path: None,
        policy_context: super::types::HelperPolicyContext {
            read_only_root: root.read_only,
            is_web_root: root.is_web_root,
        },
        limits: super::types::HelperLimits {
            max_directory_page_size: config.limits.max_directory_page_size,
            max_text_edit_size: config.limits.max_text_edit_size,
            max_chunk_size: config.limits.max_chunk_size,
            max_download_helper_bytes: config.limits.max_chunk_size,
        },
        list_options: None,
        write_options: None,
        chmod_mode: None,
        archive_source_paths: None,
        search_options: None,
    };
    super::policy::authorize(&request, config).map_err(|_| StoreError::Forbidden)?;
    super::access::with_context(&request, config, || {
        super::access::with_alias(path.clone(), operation.source_path.clone(), || {
            let mut nodes = Vec::new();
            match manifest(root, path, &mut nodes, 0, actor.uid, &expected.device) {
                Ok(()) => {
                    let current = super::observation::capture(root, path)?;
                    if current != *expected || (current.kind != "file" && !current.fully_observed())
                    {
                        return Err(StoreError::Conflict);
                    }
                    let parent = operation
                        .source_path
                        .0
                        .rsplit_once('/')
                        .map_or("", |(parent, _)| parent);
                    remove_nodes(root, nodes, &RelativePath::new(parent))?;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound && nodes.is_empty() => {}
                Err(error) => return Err(error.into()),
            }
            let id = path.0.rsplit('/').next().ok_or(StoreError::InvalidInput)?;
            uuid::Uuid::parse_str(id).map_err(|_| StoreError::InvalidInput)?;
            let parent = operation
                .source_path
                .0
                .rsplit_once('/')
                .map_or("", |(parent, _)| parent);
            super::access::with_alias(
                RelativePath::new(".tealdrive-trash/info"),
                RelativePath::new(parent),
                || -> io::Result<()> {
                    let directory = super::sandbox::open(
                        root,
                        &RelativePath::new(".tealdrive-trash/info"),
                        libc::O_PATH | libc::O_DIRECTORY,
                        0,
                    )?;
                    let name = std::ffi::CString::new(format!("{id}.msgpack"))
                        .map_err(io::Error::other)?;
                    if unsafe { libc::unlinkat(directory.as_raw_fd(), name.as_ptr(), 0) } != 0 {
                        let error = io::Error::last_os_error();
                        if error.kind() != io::ErrorKind::NotFound {
                            return Err(error);
                        }
                    }
                    Ok(())
                },
            )?;
            Ok(())
        })
    })
}
