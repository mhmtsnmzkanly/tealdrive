//! Per-operation helper context, including translation of private staging paths to real targets.
use crate::config::{AppConfig, RelativePath};
use crate::errors::TealDriveError;
use crate::helper::types::{HelperCommand, HelperRequest};
use crate::policy::access::{Access, AccessPolicy};
use std::cell::RefCell;
use std::os::unix::fs::MetadataExt;
use std::sync::Arc;

#[derive(Clone)]
struct Context {
    policy: AccessPolicy,
    username: String,
    root_id: crate::config::RootId,
    aliases: Vec<(RelativePath, RelativePath)>,
}
thread_local! {static CONTEXT: RefCell<Option<Arc<Context>>> = const {RefCell::new(None)};}
struct Reset(Option<Arc<Context>>);
impl Drop for Reset {
    fn drop(&mut self) {
        CONTEXT.with(|c| *c.borrow_mut() = self.0.take());
    }
}
pub fn with_context<T>(request: &HelperRequest, config: &AppConfig, f: impl FnOnce() -> T) -> T {
    let previous = CONTEXT.with(|c| c.borrow().clone());
    let aliases = previous
        .as_ref()
        .filter(|c| c.username == request.username && c.root_id == request.root_id)
        .map(|c| c.aliases.clone())
        .unwrap_or_default();
    let context = Context {
        policy: config.access_policy.clone(),
        username: request.username.clone(),
        root_id: request.root_id.clone(),
        aliases,
    };
    let _reset = Reset(CONTEXT.with(|c| c.replace(Some(Arc::new(context)))));
    f()
}
pub fn with_alias<T>(source: RelativePath, target: RelativePath, f: impl FnOnce() -> T) -> T {
    let previous = CONTEXT.with(|c| c.borrow().clone());
    let Some(mut context) = previous.as_deref().cloned() else {
        return f();
    };
    context.aliases.push((source, target));
    let _reset = Reset(CONTEXT.with(|c| c.replace(Some(Arc::new(context)))));
    f()
}
fn translated(context: &Context, path: &RelativePath) -> RelativePath {
    for (source, target) in context.aliases.iter().rev() {
        if path == source {
            return target.clone();
        }
        if let Some(suffix) = path.0.strip_prefix(&format!("{}/", source.0)) {
            return RelativePath::new(format!("{}/{suffix}", target.0));
        }
    }
    path.clone()
}
pub fn check(path: &RelativePath, action: Access) -> Result<(), TealDriveError> {
    CONTEXT.with(|c| match c.borrow().as_ref() {
        Some(context) => context.policy.check(
            &context.username,
            &context.root_id,
            &translated(context, path),
            action,
        ),
        None => Ok(()),
    })
}
pub fn check_metadata(metadata: &std::fs::Metadata) -> Result<(), TealDriveError> {
    if metadata.is_file()
        && metadata.nlink() > 1
        && CONTEXT.with(|c| {
            c.borrow()
                .as_ref()
                .is_some_and(|c| c.policy.reject_file_hardlinks)
        })
    {
        Err(TealDriveError::NotFound)
    } else {
        Ok(())
    }
}
pub fn visible(path: &RelativePath, metadata: &std::fs::Metadata) -> bool {
    check(path, Access::Discover).is_ok() && check_metadata(metadata).is_ok()
}
pub fn operation_access(command: HelperCommand) -> Access {
    use HelperCommand::*;
    match command {
        ListDirectory | FileMetadata | SearchFiles | ListTrash | QuotaUsage => Access::Discover,
        DownloadFile | ReadFileRange | ReadTextFile | ReadFile | Copy | CompressZip
        | CompressTarGz | ExtractArchive => Access::Read,
        WriteFile | UploadCommit | CreateFolder | RestoreFromTrash => Access::Write,
        Rename | Move => Access::Move,
        MoveToTrash | DeletePermanently => Access::Delete,
        Chmod => Access::Chmod,
    }
}
pub fn authorize(request: &HelperRequest, config: &AppConfig) -> Result<(), TealDriveError> {
    use HelperCommand::*;
    let action = operation_access(request.command);
    let check_path = |path: &RelativePath, access| {
        let path = CONTEXT.with(|c| {
            c.borrow()
                .as_ref()
                .map(|c| translated(c, path))
                .unwrap_or_else(|| path.clone())
        });
        config
            .access_policy
            .check(&request.username, &request.root_id, &path, access)
    };
    let internal_delete = request.command == DeletePermanently
        && crate::helper::trash::internal_trash_id(&request.relative_path).is_some();
    if !matches!(request.command, ListTrash | RestoreFromTrash | QuotaUsage) && !internal_delete {
        check_path(&request.relative_path, action)?;
    }
    if let Some(target) = &request.target_relative_path {
        let target = if request.command == Rename {
            let parent = request
                .relative_path
                .0
                .rsplit_once('/')
                .map_or("", |(p, _)| p);
            RelativePath::new(if parent.is_empty() {
                target.0.clone()
            } else {
                format!("{parent}/{}", target.0)
            })
        } else {
            target.clone()
        };
        check_path(&target, Access::Write)?;
    }
    for path in request.archive_source_paths.iter().flatten() {
        check_path(path, Access::Read)?;
    }
    Ok(())
}
/// Whole-tree moves/deletions cannot carry restricted descendants into a new namespace.
/// Read-only recursive operations instead omit inaccessible children individually.
pub fn preflight(request: &HelperRequest, config: &AppConfig) -> Result<(), TealDriveError> {
    use HelperCommand::*;
    if !matches!(
        request.command,
        Rename | Move | MoveToTrash | DeletePermanently
    ) {
        return Ok(());
    }
    let root = config
        .roots
        .iter()
        .find(|r| r.root_id == request.root_id)
        .ok_or(TealDriveError::InvalidRootId)?;
    let target = request.target_relative_path.as_ref().map(|target| {
        if request.command == Rename {
            let parent = request
                .relative_path
                .0
                .rsplit_once('/')
                .map_or("", |(p, _)| p);
            RelativePath::new(if parent.is_empty() {
                target.0.clone()
            } else {
                format!("{parent}/{}", target.0)
            })
        } else {
            target.clone()
        }
    });
    preflight_tree_inner(
        root,
        &request.relative_path,
        operation_access(request.command),
        if matches!(request.command, Rename | Move) {
            target.as_ref()
        } else {
            None
        },
    )
}
pub fn preflight_tree(
    root: &crate::config::AllowedRoot,
    path: &RelativePath,
    action: Access,
) -> Result<(), TealDriveError> {
    preflight_tree_inner(root, path, action, None)
}
fn preflight_tree_inner(
    root: &crate::config::AllowedRoot,
    source: &RelativePath,
    action: Access,
    target: Option<&RelativePath>,
) -> Result<(), TealDriveError> {
    let mut stack = vec![(source.clone(), 0usize)];
    let mut count = 0usize;
    while let Some((path, depth)) = stack.pop() {
        crate::helper::cancellation::check().map_err(|_| TealDriveError::PermissionDenied)?;
        count += 1;
        if count > 100000 || depth > 128 {
            return Err(TealDriveError::PolicyDenied);
        }
        check(&path, action)?;
        if let Some(target) = target {
            check(&path, Access::Read)?;
            let suffix = path
                .0
                .strip_prefix(&source.0)
                .ok_or(TealDriveError::InvalidPath)?;
            check(
                &RelativePath::new(format!("{}{suffix}", target.0)),
                Access::Write,
            )?;
        }
        let file = crate::helper::sandbox::open(root, &path, libc::O_PATH, 0).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                TealDriveError::NotFound
            } else {
                TealDriveError::PolicyDenied
            }
        })?;
        let metadata = file.metadata().map_err(|_| TealDriveError::PolicyDenied)?;
        check_metadata(&metadata)?;
        if metadata.is_dir() {
            use std::os::fd::AsRawFd;
            for entry in std::fs::read_dir(format!("/proc/self/fd/{}", file.as_raw_fd()))
                .map_err(|_| TealDriveError::PolicyDenied)?
            {
                let entry = entry.map_err(|_| TealDriveError::PolicyDenied)?;
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| TealDriveError::PolicyDenied)?;
                stack.push((
                    RelativePath::new(if path.0.is_empty() {
                        name
                    } else {
                        format!("{}/{name}", path.0)
                    }),
                    depth + 1,
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::ErrorKind;
    use crate::helper::ipc::tests_support::sample_request;
    use crate::helper::types::{HelperResponse, HelperResponsePayload};
    use crate::policy::access::{Rule, UserRules};
    fn fixture() -> (std::path::PathBuf, AppConfig) {
        let base = crate::test_support::scratch_dir()
            .join(format!("tealdrive-access-{}", uuid::Uuid::new_v4()));
        for dir in [".ssh", "private", "docs", "source/private"] {
            std::fs::create_dir_all(base.join(dir)).unwrap();
        }
        for (path, data) in [
            (".ssh/config", "ssh-secret"),
            ("private/key", "hidden-secret"),
            (".env", "project-env"),
            ("public.txt", "public"),
            ("docs/app.txt", "app"),
            ("source/public.txt", "public"),
            ("source/private/key", "hidden-secret"),
        ] {
            std::fs::write(base.join(path), data).unwrap();
        }
        let mut config = AppConfig::default();
        config.roots[0].base_path = base.clone();
        config.roots[0].hidden_files_allowed = true;
        config.features.save_text_file = true;
        config.features.chmod_file = true;
        config.features.extract_archive = true;
        config.sensitive_policy.deny_edit_patterns.clear();
        config.sensitive_policy.warn_read_patterns.clear();
        config.access_policy.root_rules = vec![
            Rule {
                path: RelativePath::new("private"),
                root_id: None,
                recursive: true,
                deny: vec![Access::Discover],
            },
            Rule {
                path: RelativePath::new("source/private"),
                root_id: None,
                recursive: true,
                deny: vec![Access::Discover],
            },
        ];
        config.access_policy.users.insert(
            "alice".into(),
            UserRules {
                admin_rules: vec![Rule {
                    path: RelativePath::new("docs"),
                    root_id: None,
                    recursive: true,
                    deny: vec![Access::Chmod],
                }],
                ..Default::default()
            },
        );
        (base, config)
    }
    fn run(config: &AppConfig, command: HelperCommand, path: &str) -> HelperResponse {
        let mut request = sample_request(command);
        request.relative_path = RelativePath::new(path);
        request.list_options.as_mut().unwrap().include_hidden = true;
        crate::helper::transaction::execute(&request, config)
    }
    #[test]
    fn hidden_paths_have_same_errors_as_missing_and_are_filtered_before_paging() {
        let (base, config) = fixture();
        for command in [
            HelperCommand::ListDirectory,
            HelperCommand::FileMetadata,
            HelperCommand::ReadTextFile,
            HelperCommand::DownloadFile,
            HelperCommand::ReadFileRange,
            HelperCommand::CreateFolder,
            HelperCommand::Chmod,
            HelperCommand::MoveToTrash,
        ] {
            for path in ["private", "private/key", ".ssh/config"] {
                assert_eq!(
                    run(&config, command, path).error_kind,
                    Some(ErrorKind::NotFound),
                    "{command:?}/{path}"
                );
            }
        }
        let listing = run(&config, HelperCommand::ListDirectory, "");
        let HelperResponsePayload::DirectoryList(list) = listing.payload else {
            panic!("{listing:?}");
        };
        assert!(list.entries.iter().any(|e| e.name == ".env"));
        assert!(!list
            .entries
            .iter()
            .any(|e| e.name == "private" || e.name == ".ssh"));
        assert!(run(&config, HelperCommand::ReadTextFile, ".env").success);
        let mut chmod = sample_request(HelperCommand::Chmod);
        chmod.relative_path = RelativePath::new("docs/app.txt");
        chmod.chmod_mode = Some(crate::protocol::schema::UnixMode(0o600));
        assert_eq!(
            crate::helper::transaction::execute(&chmod, &config).error_kind,
            Some(ErrorKind::PolicyDenied)
        );
        chmod.username = "bob".into();
        assert!(crate::helper::transaction::execute(&chmod, &config).success);
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn hardlink_and_symlink_aliases_cannot_expose_hidden_content() {
        let (base, config) = fixture();
        std::fs::hard_link(base.join("private/key"), base.join("alias.txt")).unwrap();
        std::os::unix::fs::symlink(base.join("private"), base.join("alias-dir")).unwrap();
        assert_eq!(
            run(&config, HelperCommand::FileMetadata, "alias.txt").error_kind,
            Some(ErrorKind::NotFound)
        );
        assert_eq!(
            run(&config, HelperCommand::ReadTextFile, "alias.txt").error_kind,
            Some(ErrorKind::NotFound)
        );
        assert_eq!(
            run(&config, HelperCommand::ListDirectory, "alias-dir").error_kind,
            Some(ErrorKind::NotFound)
        );
        assert_eq!(
            run(&config, HelperCommand::FileMetadata, "alias-dir").error_kind,
            Some(ErrorKind::NotFound)
        );
        let listing = run(&config, HelperCommand::ListDirectory, "");
        let HelperResponsePayload::DirectoryList(list) = listing.payload else {
            panic!("list");
        };
        assert!(!list.entries.iter().any(|e| e.name == "alias.txt"));
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn moving_parent_or_renaming_into_protected_target_is_denied() {
        let (base, config) = fixture();
        for command in [
            HelperCommand::Move,
            HelperCommand::Rename,
            HelperCommand::MoveToTrash,
        ] {
            let mut request = sample_request(command);
            request.relative_path = RelativePath::new("source");
            if command != HelperCommand::MoveToTrash {
                request.target_relative_path = Some(RelativePath::new("moved"));
            }
            assert_eq!(
                crate::helper::transaction::execute(&request, &config).error_kind,
                Some(ErrorKind::NotFound)
            );
            assert!(base.join("source/private/key").exists());
            assert!(!base.join("moved").exists());
        }
        let mut request = sample_request(HelperCommand::Rename);
        request.relative_path = RelativePath::new("docs/app.txt");
        request.target_relative_path = Some(RelativePath::new("secret.txt"));
        let mut config = config;
        config.access_policy.root_rules.push(Rule {
            path: RelativePath::new("docs/secret.txt"),
            root_id: None,
            recursive: false,
            deny: vec![Access::Write],
        });
        assert_eq!(
            crate::helper::transaction::execute(&request, &config).error_kind,
            Some(ErrorKind::PolicyDenied)
        );
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn recursive_archive_omits_hidden_content_and_private_staging_is_not_a_bypass() {
        use std::io::Read;
        let (base, mut config) = fixture();
        let mut request = sample_request(HelperCommand::CompressZip);
        request.relative_path = RelativePath::new("");
        request.archive_source_paths = Some(vec![RelativePath::new("source")]);
        request.target_relative_path = Some(RelativePath::new("output.zip"));
        assert!(crate::helper::transaction::execute(&request, &config).success);
        let mut zip =
            zip::ZipArchive::new(std::fs::File::open(base.join("output.zip")).unwrap()).unwrap();
        assert!(zip.file_names().all(|n| !n.contains("private")));
        let mut text = String::new();
        zip.by_name("source/public.txt")
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, "public");
        drop(zip);
        let mut write = sample_request(HelperCommand::WriteFile);
        write.relative_path = RelativePath::new("private/new.txt");
        write.write_options = Some(crate::helper::types::HelperWriteOptions {
            stream_size: None,
            data: b"secret".to_vec(),
            offset: 0,
            truncate: true,
            expected_revision: Some("missing".into()),
        });
        assert!(!crate::helper::transaction::execute(&write, &config).success);
        assert!(!base.join("private/new.txt").exists());
        assert_eq!(
            run(
                &config,
                HelperCommand::ReadTextFile,
                ".tdrv-transaction-guessed"
            )
            .error_kind,
            Some(ErrorKind::NotFound)
        );
        // The extraction alias must enforce the final destination rules, not only staging names.
        config.access_policy.root_rules.push(Rule {
            path: RelativePath::new("destination/source/public.txt"),
            root_id: None,
            recursive: false,
            deny: vec![Access::Write],
        });
        request.command = HelperCommand::ExtractArchive;
        request.relative_path = RelativePath::new("output.zip");
        request.target_relative_path = Some(RelativePath::new("destination"));
        request.archive_source_paths = None;
        assert_eq!(
            crate::helper::transaction::execute(&request, &config).error_kind,
            Some(ErrorKind::PolicyDenied)
        );
        assert!(!base.join("destination").exists());
        assert!(!std::fs::read_dir(&base).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".tdrv-transaction-")));
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn search_trash_restore_and_delete_recheck_current_policy() {
        let (base, mut config) = fixture();
        config.features.delete_permanently = true;
        let mut search = sample_request(HelperCommand::SearchFiles);
        search.relative_path = RelativePath::new("");
        search.search_options = Some(crate::helper::types::HelperSearchOptions {
            query: "key".into(),
            extension: None,
            max_results: Some(100),
            include_hidden: true,
            max_depth: Some(20),
        });
        let response = crate::helper::transaction::execute(&search, &config);
        let HelperResponsePayload::SearchResults(found) = response.payload else {
            panic!("{response:?}");
        };
        assert!(found.matches.is_empty());
        let response = run(&config, HelperCommand::MoveToTrash, "public.txt");
        let HelperResponsePayload::TrashEntry(entry) = response.payload else {
            panic!("{response:?}");
        };
        let mut restore = sample_request(HelperCommand::RestoreFromTrash);
        restore.relative_path = RelativePath::new(&entry.trash_id);
        config.access_policy.root_rules.push(Rule {
            path: RelativePath::new("public.txt"),
            root_id: None,
            recursive: false,
            deny: vec![Access::Discover],
        });
        let response = run(&config, HelperCommand::ListTrash, "");
        let HelperResponsePayload::TrashList(list) = response.payload else {
            panic!("{response:?}");
        };
        assert!(list.items.is_empty());
        assert_eq!(
            crate::helper::transaction::execute(&restore, &config).error_kind,
            Some(ErrorKind::NotFound)
        );
        let mut delete = sample_request(HelperCommand::DeletePermanently);
        delete.relative_path =
            RelativePath::new(format!(".tealdrive-trash/files/{}", entry.trash_id));
        assert_eq!(
            crate::helper::transaction::execute(&delete, &config).error_kind,
            Some(ErrorKind::NotFound)
        );
        assert!(base.join(&delete.relative_path.0).exists());
        config.access_policy.root_rules.pop();
        assert!(crate::helper::transaction::execute(&restore, &config).success);
        assert_eq!(std::fs::read(base.join("public.txt")).unwrap(), b"public");
        let response = run(&config, HelperCommand::MoveToTrash, "public.txt");
        let HelperResponsePayload::TrashEntry(entry) = response.payload else {
            panic!("{response:?}");
        };
        delete.relative_path =
            RelativePath::new(format!(".tealdrive-trash/files/{}", entry.trash_id));
        assert!(crate::helper::transaction::execute(&delete, &config).success);
        assert!(!base.join(&delete.relative_path.0).exists());
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn requested_hidden_flag_does_not_override_root_ceiling() {
        let (base, mut config) = fixture();
        config.roots[0].hidden_files_allowed = false;
        let response = run(&config, HelperCommand::ListDirectory, "");
        let HelperResponsePayload::DirectoryList(list) = response.payload else {
            panic!("{response:?}");
        };
        assert!(list.entries.iter().all(|e| !e.name.starts_with('.')));
        std::fs::remove_dir_all(base).unwrap();
    }
}
