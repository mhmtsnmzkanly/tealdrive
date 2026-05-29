use serde::{Deserialize, Serialize};

use crate::config::{RelativePath, SensitivePolicyConfig};
use crate::fs::path::is_hidden_relative;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensitiveMatch {
    pub is_sensitive: bool,
    pub matched_pattern: Option<String>,
    pub is_hidden: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SensitivePolicyDecision {
    Allow,
    Warn,
    Deny,
}

pub fn detect_sensitive(path: &RelativePath, config: &SensitivePolicyConfig) -> SensitiveMatch {
    let file_name = path
        .0
        .rsplit('/')
        .next()
        .unwrap_or(path.0.as_str())
        .to_ascii_lowercase();
    let patterns = config
        .deny_edit_patterns
        .iter()
        .chain(config.warn_read_patterns.iter());
    for pattern in patterns {
        if pattern_matches(&file_name, &pattern.to_ascii_lowercase()) {
            return SensitiveMatch {
                is_sensitive: true,
                matched_pattern: Some(pattern.clone()),
                is_hidden: is_hidden_relative(path),
            };
        }
    }
    SensitiveMatch {
        is_sensitive: false,
        matched_pattern: None,
        is_hidden: is_hidden_relative(path),
    }
}

pub fn hidden_policy_decision(
    include_hidden: bool,
    path: &RelativePath,
) -> SensitivePolicyDecision {
    if is_hidden_relative(path) && !include_hidden {
        SensitivePolicyDecision::Deny
    } else {
        SensitivePolicyDecision::Allow
    }
}

pub fn sensitive_read_decision(
    path: &RelativePath,
    config: &SensitivePolicyConfig,
    warning_only: bool,
) -> SensitivePolicyDecision {
    let matched = detect_sensitive(path, config);
    if matched.is_sensitive {
        if warning_only {
            SensitivePolicyDecision::Warn
        } else {
            SensitivePolicyDecision::Deny
        }
    } else {
        SensitivePolicyDecision::Allow
    }
}

pub fn sensitive_edit_decision(
    path: &RelativePath,
    config: &SensitivePolicyConfig,
) -> SensitivePolicyDecision {
    if detect_sensitive(path, config).is_sensitive {
        SensitivePolicyDecision::Deny
    } else {
        SensitivePolicyDecision::Allow
    }
}

fn pattern_matches(file_name: &str, pattern: &str) -> bool {
    if let Some(suffix) = pattern.strip_prefix("*.") {
        file_name.ends_with(&format!(".{suffix}"))
    } else if let Some(prefix) = pattern.strip_suffix(".*") {
        file_name == prefix || file_name.starts_with(&format!("{prefix}."))
    } else {
        file_name == pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_detected_sensitive() {
        assert!(
            detect_sensitive(
                &RelativePath::new(".env"),
                &SensitivePolicyConfig::default()
            )
            .is_sensitive
        );
    }

    #[test]
    fn private_key_names_detected() {
        assert!(
            detect_sensitive(
                &RelativePath::new(".ssh/id_ed25519"),
                &SensitivePolicyConfig::default()
            )
            .is_sensitive
        );
    }

    #[test]
    fn pem_and_key_detected() {
        let config = SensitivePolicyConfig::default();
        assert!(detect_sensitive(&RelativePath::new("cert.pem"), &config).is_sensitive);
        assert!(detect_sensitive(&RelativePath::new("private.key"), &config).is_sensitive);
    }

    #[test]
    fn normal_file_not_sensitive() {
        assert!(
            !detect_sensitive(
                &RelativePath::new("notes.txt"),
                &SensitivePolicyConfig::default()
            )
            .is_sensitive
        );
    }

    #[test]
    fn hidden_files_detected() {
        assert!(
            detect_sensitive(
                &RelativePath::new(".ssh"),
                &SensitivePolicyConfig::default()
            )
            .is_hidden
        );
        let env = detect_sensitive(
            &RelativePath::new(".env"),
            &SensitivePolicyConfig::default(),
        );
        assert!(env.is_hidden);
        assert!(env.is_sensitive);
        assert!(
            !detect_sensitive(
                &RelativePath::new("file.txt"),
                &SensitivePolicyConfig::default()
            )
            .is_hidden
        );
    }
}
