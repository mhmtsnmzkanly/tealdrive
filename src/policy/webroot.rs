use crate::config::WebRootPolicyConfig;
use crate::errors::PolicyReason;
use crate::fs::filename::SafeFileName;
use crate::policy::decision::PolicyDecision;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebRootPolicy {
    pub block_executable_uploads: bool,
}

pub fn check_webroot_upload(
    is_webroot: bool,
    filename: &SafeFileName,
    config: &WebRootPolicyConfig,
) -> PolicyDecision {
    if !is_webroot || !config.block_executable_uploads {
        return PolicyDecision::Allow;
    }
    let extension = filename
        .0
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase());
    if extension.as_deref().is_some_and(|ext| {
        config
            .blocked_extensions
            .iter()
            .any(|blocked| blocked == ext)
    }) {
        PolicyDecision::Deny(PolicyReason::WebRootExecutableUpload)
    } else {
        PolicyDecision::Allow
    }
}

pub fn check_webroot_chmod_executable(is_webroot: bool, mode: u32) -> PolicyDecision {
    if is_webroot && mode & 0o111 != 0 {
        PolicyDecision::Deny(PolicyReason::WebRootExecutableUpload)
    } else {
        PolicyDecision::Allow
    }
}

pub fn check_webroot_archive_extract(is_webroot: bool) -> PolicyDecision {
    if is_webroot {
        PolicyDecision::Deny(PolicyReason::WebRootExecutableUpload)
    } else {
        PolicyDecision::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn php_upload_denied_in_webroot() {
        assert_eq!(
            check_webroot_upload(
                true,
                &SafeFileName::parse("index.php").unwrap(),
                &WebRootPolicyConfig::default()
            ),
            PolicyDecision::Deny(PolicyReason::WebRootExecutableUpload)
        );
    }

    #[test]
    fn sh_upload_denied_in_webroot() {
        assert_eq!(
            check_webroot_upload(
                true,
                &SafeFileName::parse("deploy.sh").unwrap(),
                &WebRootPolicyConfig::default()
            ),
            PolicyDecision::Deny(PolicyReason::WebRootExecutableUpload)
        );
    }

    #[test]
    fn text_upload_allowed_in_webroot_if_write_allowed() {
        assert_eq!(
            check_webroot_upload(
                true,
                &SafeFileName::parse("readme.txt").unwrap(),
                &WebRootPolicyConfig::default()
            ),
            PolicyDecision::Allow
        );
    }
}
