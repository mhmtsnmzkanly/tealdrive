use std::path::Path;

pub struct SensitivePolicy;

impl SensitivePolicy {
    pub fn is_sensitive(path: &Path) -> bool {
        let name = match path.file_name() {
            Some(n) => n.to_string_lossy(),
            None => return false,
        };

        // Basic sensitive file rules
        name.starts_with(".env") || 
        name.starts_with(".ssh") || 
        name == "id_rsa" || 
        name == "shadow" || 
        name == "sudoers"
    }
}
