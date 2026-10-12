//! User-identity usage measurement. Errors never mean zero usage.
use crate::config::AppConfig;
use crate::helper::types::{HelperRequest, HelperResponse, HelperResponsePayload};
pub fn measure(request: &HelperRequest, config: &AppConfig) -> HelperResponse {
    let Some(root) = config.roots.iter().find(|r| r.root_id == request.root_id) else {
        return HelperResponse::permission_denied(request, "Unknown root.");
    };
    let native = config.web_accounts.enabled
        && config.virtual_accounts.enforcement == crate::accounts::virtuals::Enforcement::UserQuota
        && request.uid >= config.web_accounts.virtual_min_uid
        && request.uid <= config.web_accounts.virtual_max_uid;
    match if native {
        crate::helper::native_quota::read(root, request.uid).map(|usage| usage.used_bytes)
    } else {
        scan(&root.base_path)
    } {
        Ok(used) => {
            let mut response = HelperResponse::operation_done(request, "Usage measured.");
            response.payload = HelperResponsePayload::Usage {
                used_bytes: used,
                limit_bytes: root.quota_bytes,
            };
            response
        }
        Err(error) => crate::helper::listing::helper_error_from_io(request, error),
    }
}
pub fn scan(root: &std::path::Path) -> std::io::Result<u64> {
    crate::helper::quota::measure(&crate::config::AllowedRoot {
        base_path: root.to_path_buf(),
        ..Default::default()
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn usage_deduplicates_links_and_counts_internal_files() {
        let dir = std::env::temp_dir().join(format!("tdrv-usage-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("file"), b"12345").unwrap();
        std::fs::hard_link(dir.join("file"), dir.join("hardlink")).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", dir.join("outside")).unwrap();
        std::fs::create_dir(dir.join(".internal")).unwrap();
        std::fs::write(dir.join(".internal/trash"), b"123").unwrap();
        assert_eq!(scan(&dir).unwrap(), 8);
        assert!(scan(&dir.join("missing")).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
