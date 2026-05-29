use std::path::Path;
use crate::fs::metadata::{FileEntry, EntryType};
use crate::policy::{RootPolicy, SensitivePolicy};
use std::fs;
use std::time::UNIX_EPOCH;
use mime_guess;
use std::os::linux::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;

pub struct FsOperations;

impl FsOperations {
    pub fn list_directory(
        path_str: &str,
        root_policy: &RootPolicy,
    ) -> anyhow::Result<Vec<FileEntry>> {
        let path = Path::new(path_str);
        
        // 1. Canonicalize
        let canonical_path = fs::canonicalize(path)?;

        // 2. Policy Check
        if !root_policy.is_allowed(&canonical_path) {
            return Err(anyhow::anyhow!("Access denied by policy"));
        }

        // 3. Read Dir
        let mut entries = Vec::new();
        for entry in fs::read_dir(&canonical_path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            let path = entry.path();
            
            let name = entry.file_name().to_string_lossy().to_string();
            let entry_type = if metadata.is_dir() {
                EntryType::Dir
            } else if metadata.is_symlink() {
                EntryType::Symlink
            } else {
                EntryType::File
            };

            let modified = metadata.modified()?
                .duration_since(UNIX_EPOCH)?
                .as_secs();

            let is_sensitive = SensitivePolicy::is_sensitive(&path);
            let is_hidden = name.starts_with('.');
            
            // Octal mode and permissions string
            let mode = metadata.permissions().mode();
            let permissions = format!("{:o}", mode & 0o777);

            let entry = FileEntry {
                name,
                entry_type,
                size: metadata.len(),
                modified,
                owner: "user".to_string(), // Placeholder, real implementation would use nix or users crate
                group: "group".to_string(), // Placeholder
                mode: mode & 0o777,
                permissions,
                capabilities: vec!["read".to_string()],
                is_sensitive,
                is_protected: false,
                is_hidden,
                is_symlink: metadata.is_symlink(),
                symlink_target: if metadata.is_symlink() {
                    fs::read_link(&path).ok().map(|p| p.to_string_lossy().to_string())
                } else {
                    None
                },
            };
            
            entries.push(entry);
        }

        Ok(entries)
    }
}
