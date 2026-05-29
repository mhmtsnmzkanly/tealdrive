use std::path::{Path, PathBuf};
use crate::fs::metadata::{FileEntry, EntryType};
use crate::policy::{RootPolicy, SensitivePolicy};
use std::fs;
use std::time::UNIX_EPOCH;
use mime_guess;

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
            
            // Basic metadata extraction
            // In a real system, we'd get owner/group names
            let entry = FileEntry {
                id: path.to_string_lossy().to_string(), // In V1, use path as ID
                name,
                entry_type,
                size: metadata.len(),
                modified,
                permissions: 0o644, // Placeholder
                owner: "user".to_string(), // Placeholder
                group: "group".to_string(), // Placeholder
                mime: mime_guess::from_path(&path).first_or_octet_stream().to_string(),
                is_sensitive,
                capabilities: vec!["READ".to_string()],
            };
            
            entries.push(entry);
        }

        Ok(entries)
    }
}
