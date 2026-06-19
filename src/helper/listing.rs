use std::cmp::Reverse;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::config::{AllowedRoot, RelativePath, RootId, SensitivePolicyConfig};
use crate::errors::{ErrorKind, TealDriveError};
use crate::fs::path::resolve_existing_path;
use crate::helper::types::{
    HelperCommand, HelperDirectoryEntry, HelperDirectoryList, HelperRequest, HelperResponse,
};
use crate::policy::sensitive::detect_sensitive;
use crate::protocol::schema::{
    CapabilitySet, DirectoryCursor, FileFilter, FileKind, FilterMode, GroupName, OwnerName,
    SortMode, UnixMode,
};

pub fn list_directory_with_roots(
    request: &HelperRequest,
    roots: &[AllowedRoot],
    sensitive_config: &SensitivePolicyConfig,
) -> HelperResponse {
    if request.command != HelperCommand::ListDirectory {
        return HelperResponse::not_implemented(request);
    }
    if request.uid == 0 {
        return HelperResponse::permission_denied(request, "Helper refuses uid 0.");
    }

    let Some(root) = roots.iter().find(|root| root.root_id == request.root_id) else {
        return helper_error(
            request,
            ErrorKind::InvalidPath,
            "Unknown root id.",
            "invalid_root_id",
        );
    };
    let options = request
        .list_options
        .clone()
        .unwrap_or_else(|| default_list_options(request));
    let limit = options.limit.min(request.limits.max_directory_page_size);

    let resolution = match resolve_existing_path(root, &request.relative_path, false) {
        Ok(resolution) => resolution,
        Err(error) => return helper_error_from_tealdrive(request, error),
    };
    let metadata = match fs::metadata(&resolution.resolved_path) {
        Ok(metadata) => metadata,
        Err(error) => return helper_error_from_io(request, error),
    };
    if !metadata.is_dir() {
        return helper_error(
            request,
            ErrorKind::InvalidPath,
            "Path is not a directory.",
            "not_directory",
        );
    }

    let mut entries = match collect_entries(
        &resolution.resolved_path,
        root,
        sensitive_config,
        &options.filter,
        options.include_hidden,
    ) {
        Ok(entries) => entries,
        Err(error) => return helper_error_from_io(request, error),
    };
    sort_entries(&mut entries, &options.sort);

    let offset = parse_cursor_offset(options.cursor.as_ref()).unwrap_or(0);
    let total_estimate = entries.len();
    let paged: Vec<_> = entries.into_iter().skip(offset).take(limit + 1).collect();
    let has_more = paged.len() > limit;
    let mut entries = paged;
    if has_more {
        entries.truncate(limit);
    }
    let next_cursor = if has_more {
        Some(DirectoryCursor(format!(
            "offset:{}",
            offset + entries.len()
        )))
    } else {
        None
    };

    HelperResponse::directory_list(
        request,
        HelperDirectoryList {
            entries,
            has_more,
            next_cursor,
            total_estimate: Some(total_estimate),
        },
    )
}

pub fn allowed_roots_from_env() -> Vec<AllowedRoot> {
    let Ok(value) = std::env::var("TEALDRIVE_HELPER_ROOTS") else {
        return Vec::new();
    };
    value
        .split(';')
        .filter_map(|entry| {
            let (root_id, path) = entry.split_once('=')?;
            if root_id.is_empty() || path.is_empty() {
                return None;
            }
            Some(AllowedRoot {
                root_id: RootId::new(root_id),
                base_path: PathBuf::from(path),
                read_only: false,
                uploads_allowed: false,
                hidden_files_allowed: false,
                is_web_root: false,
            })
        })
        .collect()
}

fn collect_entries(
    directory: &Path,
    root: &AllowedRoot,
    sensitive_config: &SensitivePolicyConfig,
    filter: &FilterMode,
    include_hidden: bool,
) -> io::Result<Vec<HelperDirectoryEntry>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        let is_hidden = name.starts_with('.');
        if is_hidden && !include_hidden {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        let is_symlink = metadata.file_type().is_symlink();
        let file_type = if is_symlink {
            FileKind::Symlink
        } else if metadata.is_dir() {
            FileKind::Directory
        } else if metadata.is_file() {
            FileKind::File
        } else {
            FileKind::Other
        };
        if !matches_filter(&name, file_type, filter, is_hidden, is_symlink) {
            continue;
        }
        entries.push(directory_entry_from_metadata(
            name,
            &metadata,
            root,
            sensitive_config,
        ));
    }
    Ok(entries)
}

pub(crate) fn directory_entry_from_metadata(
    name: String,
    metadata: &fs::Metadata,
    root: &AllowedRoot,
    sensitive_config: &SensitivePolicyConfig,
) -> HelperDirectoryEntry {
    let relative = RelativePath::new(name.clone());
    let is_hidden = name.starts_with('.');
    let is_symlink = metadata.file_type().is_symlink();
    let file_type = if is_symlink {
        FileKind::Symlink
    } else if metadata.is_dir() {
        FileKind::Directory
    } else if metadata.is_file() {
        FileKind::File
    } else {
        FileKind::Other
    };
    let sensitive = detect_sensitive(&relative, sensitive_config);
    HelperDirectoryEntry {
        name,
        file_type,
        size: if metadata.is_file() {
            metadata.len()
        } else {
            0
        },
        modified: metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs().to_string()),
        owner: owner_name(metadata),
        group: group_name(metadata),
        mode: unix_mode(metadata),
        permissions: permission_string(metadata),
        is_hidden,
        is_sensitive: sensitive.is_sensitive,
        is_protected: false,
        is_symlink,
        symlink_target: None,
        is_web_root: root.is_web_root,
        is_world_writable: is_world_writable(metadata),
        capabilities: CapabilitySet {
            can_list: metadata.is_dir() && !is_symlink,
            can_download: metadata.is_file() && !is_symlink,
            can_upload: root.uploads_allowed && !root.read_only,
            can_create_folder: !root.read_only,
            can_rename: !root.read_only,
            can_move_to_trash: !root.read_only,
            can_restore_from_trash: !root.read_only,
        },
    }
}

fn matches_filter(
    name: &str,
    file_type: FileKind,
    filter: &FilterMode,
    is_hidden: bool,
    is_symlink: bool,
) -> bool {
    if let Some(query) = &filter.query {
        if !name
            .to_ascii_lowercase()
            .contains(&query.to_ascii_lowercase())
        {
            return false;
        }
    }
    if let Some(kind) = filter.file_kind {
        if kind != file_type {
            return false;
        }
    }
    match filter.filter.as_ref().unwrap_or(&FileFilter::All) {
        FileFilter::All => true,
        FileFilter::Folders => file_type == FileKind::Directory,
        FileFilter::Hidden => is_hidden,
        FileFilter::Symlinks => is_symlink,
        FileFilter::Code => has_ext(
            name,
            &["rs", "js", "ts", "php", "py", "rb", "sh", "html", "css"],
        ),
        FileFilter::Images => has_ext(name, &["png", "jpg", "jpeg", "gif", "webp", "svg"]),
        FileFilter::Media => has_ext(name, &["mp3", "mp4", "mkv", "mov", "wav", "flac"]),
        FileFilter::Documents => has_ext(name, &["txt", "md", "pdf", "doc", "docx", "odt"]),
        FileFilter::Archives => has_ext(name, &["zip", "tar", "gz", "tgz", "xz", "7z"]),
        FileFilter::Writable => true,
    }
}

fn has_ext(name: &str, exts: &[&str]) -> bool {
    name.rsplit_once('.')
        .map(|(_, ext)| {
            exts.iter()
                .any(|candidate| ext.eq_ignore_ascii_case(candidate))
        })
        .unwrap_or(false)
}

fn sort_entries(entries: &mut [HelperDirectoryEntry], sort: &SortMode) {
    match sort {
        SortMode::NameAsc => entries.sort_by(|a, b| a.name.cmp(&b.name)),
        SortMode::NameDesc => entries.sort_by(|a, b| b.name.cmp(&a.name)),
        SortMode::ModifiedAsc => entries.sort_by(|a, b| a.modified.cmp(&b.modified)),
        SortMode::ModifiedDesc => entries.sort_by(|a, b| b.modified.cmp(&a.modified)),
        SortMode::SizeAsc => entries.sort_by_key(|entry| entry.size),
        SortMode::SizeDesc => entries.sort_by_key(|entry| Reverse(entry.size)),
        SortMode::TypeAsc => entries.sort_by_key(|entry| file_kind_order(entry.file_type)),
        SortMode::TypeDesc => {
            entries.sort_by_key(|entry| Reverse(file_kind_order(entry.file_type)))
        }
    }
}

fn file_kind_order(kind: FileKind) -> u8 {
    match kind {
        FileKind::Directory => 0,
        FileKind::File => 1,
        FileKind::Symlink => 2,
        FileKind::Other => 3,
    }
}

fn parse_cursor_offset(cursor: Option<&DirectoryCursor>) -> Option<usize> {
    cursor?.0.strip_prefix("offset:")?.parse().ok()
}

fn default_list_options(
    request: &HelperRequest,
) -> crate::helper::types::HelperListDirectoryOptions {
    crate::helper::types::HelperListDirectoryOptions {
        cursor: None,
        limit: request.limits.max_directory_page_size.min(200),
        include_hidden: false,
        sort: SortMode::NameAsc,
        filter: FilterMode {
            query: None,
            file_kind: None,
            filter: Some(FileFilter::All),
        },
    }
}

pub(crate) fn helper_error_from_tealdrive(
    request: &HelperRequest,
    error: TealDriveError,
) -> HelperResponse {
    match error {
        TealDriveError::SymlinkDenied | TealDriveError::SymlinkEscape => helper_error(
            request,
            ErrorKind::PolicyDenied,
            "Symlink access denied.",
            "symlink_denied",
        ),
        TealDriveError::InvalidPath | TealDriveError::PathEscapesRoot => helper_error(
            request,
            ErrorKind::InvalidPath,
            "Invalid path.",
            "invalid_path",
        ),
        TealDriveError::InvalidTarget => helper_error(
            request,
            ErrorKind::InvalidTarget,
            "Invalid target.",
            "invalid_target",
        ),
        TealDriveError::InvalidFilename
        | TealDriveError::NullByteRejected
        | TealDriveError::ReservedNameRejected => helper_error(
            request,
            ErrorKind::ValidationError,
            "Invalid filename.",
            "invalid_filename",
        ),
        TealDriveError::NotFound => {
            helper_error(request, ErrorKind::NotFound, "Path not found.", "not_found")
        }
        TealDriveError::TraversalRejected => helper_error(
            request,
            ErrorKind::PolicyDenied,
            "Path traversal rejected.",
            "path_traversal",
        ),
        _ => helper_error(
            request,
            ErrorKind::InternalError,
            "Helper path validation failed.",
            "path_validation_failed",
        ),
    }
}

pub(crate) fn helper_error_from_io(request: &HelperRequest, error: io::Error) -> HelperResponse {
    let (kind, message, reason) = match error.raw_os_error() {
        Some(13) | Some(1) => (
            ErrorKind::PermissionDenied,
            "Permission denied.",
            "permission_denied",
        ),
        Some(2) => (ErrorKind::NotFound, "Path not found.", "not_found"),
        Some(20) => (ErrorKind::InvalidPath, "Invalid path.", "not_directory"),
        Some(40) => (ErrorKind::SymlinkLoop, "Symlink loop.", "symlink_loop"),
        Some(36) => (ErrorKind::NameTooLong, "Name too long.", "name_too_long"),
        Some(30) => (
            ErrorKind::ReadOnlyFilesystem,
            "Read-only filesystem.",
            "read_only_filesystem",
        ),
        _ => (
            ErrorKind::InternalError,
            "Filesystem error.",
            "filesystem_error",
        ),
    };
    helper_error(request, kind, message, reason)
}

pub(crate) fn helper_error(
    request: &HelperRequest,
    error_kind: ErrorKind,
    safe_message: &'static str,
    reason: &'static str,
) -> HelperResponse {
    let mut response = HelperResponse::not_implemented(request);
    response.error_kind = Some(error_kind);
    response.safe_message = Some(safe_message.to_owned());
    if let Some(metadata) = response.audit_metadata.as_mut() {
        metadata.reason = Some(reason.to_owned());
    }
    response
}

#[cfg(unix)]
fn unix_mode(metadata: &fs::Metadata) -> Option<UnixMode> {
    use std::os::unix::fs::MetadataExt;
    Some(UnixMode(metadata.mode()))
}

#[cfg(not(unix))]
fn unix_mode(_metadata: &fs::Metadata) -> Option<UnixMode> {
    None
}

#[cfg(unix)]
fn owner_name(metadata: &fs::Metadata) -> Option<OwnerName> {
    use std::os::unix::fs::MetadataExt;
    Some(OwnerName(metadata.uid().to_string()))
}

#[cfg(not(unix))]
fn owner_name(_metadata: &fs::Metadata) -> Option<OwnerName> {
    None
}

#[cfg(unix)]
fn group_name(metadata: &fs::Metadata) -> Option<GroupName> {
    use std::os::unix::fs::MetadataExt;
    Some(GroupName(metadata.gid().to_string()))
}

#[cfg(not(unix))]
fn group_name(_metadata: &fs::Metadata) -> Option<GroupName> {
    None
}

#[cfg(unix)]
fn permission_string(metadata: &fs::Metadata) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    let mode = metadata.mode();
    let mut out = String::new();
    for bit in [
        0o400, 0o200, 0o100, 0o040, 0o020, 0o010, 0o004, 0o002, 0o001,
    ] {
        out.push(match bit {
            0o400 | 0o040 | 0o004 => {
                if mode & bit != 0 {
                    'r'
                } else {
                    '-'
                }
            }
            0o200 | 0o020 | 0o002 => {
                if mode & bit != 0 {
                    'w'
                } else {
                    '-'
                }
            }
            _ => {
                if mode & bit != 0 {
                    'x'
                } else {
                    '-'
                }
            }
        });
    }
    Some(out)
}

#[cfg(not(unix))]
fn permission_string(_metadata: &fs::Metadata) -> Option<String> {
    None
}

#[cfg(unix)]
fn is_world_writable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    metadata.mode() & 0o002 != 0
}

#[cfg(not(unix))]
fn is_world_writable(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    use crate::helper::ipc::tests_support::sample_request;
    use crate::helper::types::HelperResponsePayload;

    fn temp_root() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("tealdrive-helper-list-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temp root");
        path
    }

    fn root(path: PathBuf) -> AllowedRoot {
        AllowedRoot {
            root_id: RootId::new("home"),
            base_path: path,
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
        }
    }

    #[test]
    fn helper_list_directory_returns_temp_entries() {
        let base = temp_root();
        fs::write(base.join("a.txt"), b"hello").expect("file");
        fs::create_dir(base.join("folder")).expect("folder");
        let mut request = sample_request(HelperCommand::ListDirectory);
        request.relative_path = RelativePath::new("");
        let response =
            list_directory_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());

        assert!(response.success);
        let HelperResponsePayload::DirectoryList(list) = response.payload else {
            panic!("expected directory list");
        };
        assert!(list.entries.iter().any(|entry| entry.name == "a.txt"));
        assert!(list
            .entries
            .iter()
            .any(|entry| entry.file_type == FileKind::Directory));
    }

    #[test]
    fn helper_does_not_include_absolute_paths() {
        let base = temp_root();
        fs::write(base.join("a.txt"), b"hello").expect("file");
        let mut request = sample_request(HelperCommand::ListDirectory);
        request.relative_path = RelativePath::new("");
        let response = list_directory_with_roots(
            &request,
            &[root(base.clone())],
            &SensitivePolicyConfig::default(),
        );
        let serialized = format!("{response:?}");

        assert!(!serialized.contains(base.to_string_lossy().as_ref()));
    }

    #[cfg(unix)]
    #[test]
    fn helper_does_not_follow_symlink_by_default() {
        let base = temp_root();
        fs::write(base.join("target"), b"hello").expect("file");
        symlink(base.join("target"), base.join("link")).expect("symlink");
        let mut request = sample_request(HelperCommand::ListDirectory);
        request.relative_path = RelativePath::new("");
        let response =
            list_directory_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());
        let HelperResponsePayload::DirectoryList(list) = response.payload else {
            panic!("expected directory list");
        };
        let link = list
            .entries
            .iter()
            .find(|entry| entry.name == "link")
            .expect("link");

        assert!(link.is_symlink);
        assert_eq!(link.file_type, FileKind::Symlink);
        assert!(link.symlink_target.is_none());
    }

    #[test]
    fn helper_missing_directory_maps_not_found() {
        let base = temp_root();
        let mut request = sample_request(HelperCommand::ListDirectory);
        request.relative_path = RelativePath::new("missing");
        let response =
            list_directory_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());

        assert_eq!(response.error_kind, Some(ErrorKind::NotFound));
    }

    #[test]
    fn helper_file_as_directory_maps_invalid_path() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"hello").expect("file");
        let mut request = sample_request(HelperCommand::ListDirectory);
        request.relative_path = RelativePath::new("file.txt");
        let response =
            list_directory_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());

        assert_eq!(response.error_kind, Some(ErrorKind::InvalidPath));
    }
}
