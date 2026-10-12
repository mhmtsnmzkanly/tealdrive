//! Shared path policy. Discovery denial deliberately looks like a missing object.
use crate::config::{RelativePath, RootId};
use crate::errors::TealDriveError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    Discover,
    Read,
    Write,
    Chmod,
    Delete,
    Move,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub path: RelativePath,
    #[serde(default)]
    pub root_id: Option<RootId>,
    #[serde(default)]
    pub recursive: bool,
    pub deny: Vec<Access>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UserRules {
    pub root_rules: Vec<Rule>,
    pub admin_rules: Vec<Rule>,
    pub personal_rules: Vec<Rule>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AccessPolicy {
    pub deny_system_components: Vec<String>,
    pub reject_file_hardlinks: bool,
    pub root_rules: Vec<Rule>,
    pub users: BTreeMap<String, UserRules>,
}
impl Default for AccessPolicy {
    fn default() -> Self {
        Self {
            deny_system_components: [".ssh", ".gnupg", ".aws", ".kube"]
                .map(String::from)
                .to_vec(),
            reject_file_hardlinks: true,
            root_rules: Vec::new(),
            users: BTreeMap::new(),
        }
    }
}
impl AccessPolicy {
    pub fn validate(&self) -> Result<(), TealDriveError> {
        if self.deny_system_components.len() > 128
            || self.users.len() > 1024
            || self.deny_system_components.iter().any(|s| {
                s.is_empty()
                    || s.len() > 255
                    || s.contains(['/', '\\', '\0'])
                    || s == "."
                    || s == ".."
            })
            || self
                .users
                .keys()
                .any(|s| s.is_empty() || s.len() > 256 || s.contains(['/', '\\', '\0']))
        {
            return Err(TealDriveError::Validation);
        }
        let mut count = 0;
        for rule in self
            .root_rules
            .iter()
            .chain(self.users.values().flat_map(|u| {
                u.root_rules
                    .iter()
                    .chain(&u.admin_rules)
                    .chain(&u.personal_rules)
            }))
        {
            count += 1;
            if count > 4096
                || rule.path.0.len() > 4096
                || rule.deny.is_empty()
                || rule.deny.len() > 6
                || crate::fs::path::validate_relative_path(&rule.path)? != rule.path
                || rule.path.0.contains('\\')
                || rule.root_id.as_ref().is_some_and(|id| id.0.is_empty())
            {
                return Err(TealDriveError::Validation);
            }
        }
        Ok(())
    }
    pub fn extend_ceiling(&mut self, ceiling: &Self) {
        self.reject_file_hardlinks |= ceiling.reject_file_hardlinks;
        self.deny_system_components
            .extend(ceiling.deny_system_components.iter().cloned());
        self.deny_system_components.sort();
        self.deny_system_components.dedup();
        self.root_rules.extend(ceiling.root_rules.iter().cloned());
        self.root_rules.dedup();
        for (name, rules) in &ceiling.users {
            let current = self.users.entry(name.clone()).or_default();
            current.root_rules.extend(rules.root_rules.iter().cloned());
            current
                .admin_rules
                .extend(rules.admin_rules.iter().cloned());
            current
                .personal_rules
                .extend(rules.personal_rules.iter().cloned());
            current.root_rules.dedup();
            current.admin_rules.dedup();
            current.personal_rules.dedup();
        }
    }
    pub fn check(
        &self,
        username: &str,
        root: &RootId,
        path: &RelativePath,
        action: Access,
    ) -> Result<(), TealDriveError> {
        let path = crate::fs::path::validate_relative_path(path)?;
        if path.0.split('/').any(|component| {
            self.deny_system_components
                .iter()
                .any(|denied| denied == component)
                || component == ".tealdrive-trash"
                || component.starts_with(".tdrv-transaction-")
        }) {
            return Err(TealDriveError::NotFound);
        }
        let user = self.users.get(username);
        let rules = self.root_rules.iter().chain(user.into_iter().flat_map(|u| {
            u.root_rules
                .iter()
                .chain(&u.admin_rules)
                .chain(&u.personal_rules)
        }));
        let mut denied = false;
        for rule in rules {
            if rule.root_id.as_ref().is_some_and(|id| id != root) {
                continue;
            }
            let descendant =
                rule.path.0.is_empty() || path.0.starts_with(&format!("{}/", rule.path.0));
            // An undiscoverable ancestor cannot be bypassed by directly naming a child.
            if (path == rule.path || descendant) && rule.deny.contains(&Access::Discover) {
                return Err(TealDriveError::NotFound);
            }
            if path == rule.path || (rule.recursive && descendant) {
                denied |= rule.deny.contains(&action);
            }
        }
        if denied {
            Err(TealDriveError::PolicyDenied)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layered_denials_are_additive_and_component_boundaries_are_exact() {
        let mut policy = AccessPolicy::default();
        let root = RootId::new("home");
        policy.root_rules.push(Rule {
            path: RelativePath::new("private"),
            root_id: None,
            recursive: true,
            deny: vec![Access::Discover],
        });
        policy.users.insert(
            "alice".into(),
            UserRules {
                admin_rules: vec![Rule {
                    path: RelativePath::new("app"),
                    root_id: Some(root.clone()),
                    recursive: true,
                    deny: vec![Access::Chmod],
                }],
                ..Default::default()
            },
        );
        policy.validate().unwrap();
        assert_eq!(
            policy.check(
                "alice",
                &root,
                &RelativePath::new("private/secret"),
                Access::Read
            ),
            Err(TealDriveError::NotFound)
        );
        assert!(policy
            .check(
                "alice",
                &root,
                &RelativePath::new("private-public"),
                Access::Read
            )
            .is_ok());
        assert_eq!(
            policy.check(
                "alice",
                &root,
                &RelativePath::new("app/file"),
                Access::Chmod
            ),
            Err(TealDriveError::PolicyDenied)
        );
        assert!(policy
            .check("bob", &root, &RelativePath::new("app/file"), Access::Chmod)
            .is_ok());
        assert!(policy
            .check("alice", &root, &RelativePath::new(".env"), Access::Read)
            .is_ok());
        assert_eq!(
            policy.check(
                "alice",
                &root,
                &RelativePath::new("nested/.ssh/config"),
                Access::Discover
            ),
            Err(TealDriveError::NotFound)
        );
        policy.root_rules[0].recursive = false;
        assert_eq!(
            policy.check(
                "alice",
                &root,
                &RelativePath::new("private/child"),
                Access::Read
            ),
            Err(TealDriveError::NotFound)
        );
        let mut request = AccessPolicy {
            deny_system_components: vec![],
            reject_file_hardlinks: false,
            root_rules: vec![],
            users: BTreeMap::new(),
        };
        request.extend_ceiling(&policy);
        assert!(request.reject_file_hardlinks);
        assert_eq!(
            request.check("alice", &root, &RelativePath::new("private"), Access::Write),
            Err(TealDriveError::NotFound)
        );
    }
    #[test]
    fn malformed_rules_fail_closed() {
        for path in ["../x", "/etc", "a/../x", "a//x", "a\\x"] {
            let policy = AccessPolicy {
                root_rules: vec![Rule {
                    path: RelativePath::new(path),
                    root_id: None,
                    recursive: true,
                    deny: vec![Access::Discover],
                }],
                ..Default::default()
            };
            assert!(policy.validate().is_err(), "{path}");
        }
    }
}
