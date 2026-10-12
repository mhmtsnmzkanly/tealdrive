//! Identity-derived home profiles. Callers must obtain the account through the
//! protected gateway, which checks its generation and current NSS identity.
use crate::accounts::store::{Account, AccountKind, AccountStatus};
use crate::config::{AllowedRoot, AppConfig, RootId};
use crate::errors::TealDriveError;
use std::path::{Component, Path};

/// Resolve legacy profiles without broadening them, or materialize an explicitly
/// selected Root template. The template path is never used as the user's home.
pub fn resolve(
    config: &AppConfig,
    account: &Account,
    template: Option<&AllowedRoot>,
) -> Result<AppConfig, TealDriveError> {
    if account.status != AccountStatus::Active
        || !account.login_enabled
        || account.uid == 0
        || account.generation == 0
        || !safe_home(&account.home, account.kind)
    {
        return Err(TealDriveError::PermissionDenied);
    }
    let mut effective = config.clone();
    effective.home_template = None;
    effective
        .roots
        .retain(|root| root.base_path == account.home);
    if let Some(template) = template {
        // Mixing static and generated profiles needs an explicit migration;
        // silently replacing a more restrictive legacy profile is unsafe.
        if !effective.roots.is_empty() {
            return Err(TealDriveError::Validation);
        }
        let mut root = template.clone();
        root.base_path = account.home.clone();
        effective.roots.push(root);
    }
    if effective.roots.is_empty() {
        return Err(TealDriveError::PermissionDenied);
    }
    if account.kind == AccountKind::Virtual {
        let allocation = account.quota_bytes.unwrap_or(0);
        for root in &mut effective.roots {
            root.quota_bytes = Some(root.quota_bytes.map_or(allocation, |q| q.min(allocation)));
        }
    }
    effective.validate()?;
    Ok(effective)
}

fn safe_home(path: &Path, kind: AccountKind) -> bool {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        || path.as_os_str().as_encoded_bytes().contains(&0)
        || path == Path::new("/")
    {
        return false;
    }
    // Reserved application state and operating-system trees cannot become file
    // roots even if an administrator accidentally sets such an NSS home.
    let virtual_parent = Path::new("/home/tealdrive/virtual-homes");
    if path.starts_with("/home/tealdrive") {
        return kind == AccountKind::Virtual && path.parent() == Some(virtual_parent);
    }
    ![
        "/etc",
        "/proc",
        "/sys",
        "/dev",
        "/run",
        "/boot",
        "/usr",
        "/bin",
        "/sbin",
        "/lib",
        "/lib64",
        "/root",
        "/home/tealdrive",
    ]
    .iter()
    .any(|reserved| path.starts_with(reserved))
}

/// A suggested template only; it is never automatically applied to legacy roots.
pub fn default_template() -> AllowedRoot {
    AllowedRoot {
        root_id: RootId::new("home"),
        hidden_files_allowed: true,
        ..AllowedRoot::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::store::Role;

    fn account() -> Account {
        Account {
            id: "acct_alice".into(),
            username: "alice".into(),
            uid: 1000,
            gid: 1000,
            home: "/home/alice".into(),
            kind: AccountKind::Linux,
            role: Role::User,
            owner_admin_id: None,
            status: AccountStatus::Active,
            login_enabled: true,
            generation: 1,
            password_enrolled: true,
            quota_bytes: None,
            undo_hours: None,
            created_at: 1,
            updated_at: 1,
            deleted_at: None,
            deny_enrollment: false,
        }
    }

    #[test]
    fn template_uses_verified_home_and_preserves_constraints() {
        let config = AppConfig::default();
        let mut template = default_template();
        template.base_path = "/etc".into();
        template.read_only = true;
        template.uploads_allowed = false;
        template.quota_bytes = Some(100);
        let effective = resolve(&config, &account(), Some(&template)).unwrap();
        assert_eq!(effective.roots[0].base_path, account().home);
        assert!(effective.roots[0].read_only);
        assert!(!effective.roots[0].uploads_allowed);
        assert_eq!(effective.roots[0].quota_bytes, Some(100));
        assert_eq!(effective.access_policy, config.access_policy);
        assert_eq!(effective.sensitive_policy, config.sensitive_policy);
    }

    #[test]
    fn legacy_migration_cannot_silently_replace_constraints() {
        let mut config = AppConfig::default();
        config.roots[0].base_path = account().home;
        config.roots[0].read_only = true;
        assert!(resolve(&config, &account(), None).unwrap().roots[0].read_only);
        assert_eq!(
            resolve(&config, &account(), Some(&default_template())),
            Err(TealDriveError::Validation)
        );
        config.roots[0].base_path = "/home/bob".into();
        assert_eq!(
            resolve(&config, &account(), None),
            Err(TealDriveError::PermissionDenied)
        );
    }

    #[test]
    fn unsafe_homes_and_inactive_accounts_are_rejected() {
        for path in [
            "/",
            "/etc",
            "/etc/users",
            "/home/tealdrive/auth",
            "/proc/self",
            "/root",
            "home/alice",
            "/home/../etc",
            "/home/alice\0",
        ] {
            let mut user = account();
            user.home = path.into();
            assert!(
                resolve(&AppConfig::default(), &user, Some(&default_template())).is_err(),
                "{path:?}"
            );
        }
        for status in [
            AccountStatus::Suspended,
            AccountStatus::Revoked,
            AccountStatus::Deleted,
        ] {
            let mut user = account();
            user.status = status;
            assert!(resolve(&AppConfig::default(), &user, Some(&default_template())).is_err());
        }
        let mut user = account();
        user.login_enabled = false;
        assert!(resolve(&AppConfig::default(), &user, Some(&default_template())).is_err());
    }

    #[test]
    fn virtual_budget_cannot_be_widened() {
        let mut user = account();
        user.kind = AccountKind::Virtual;
        user.home = "/home/tealdrive/virtual-homes/acct_alice".into();
        for (allocation, ceiling, expected) in [
            (None, None, 0),
            (Some(200), Some(100), 100),
            (Some(50), Some(100), 50),
        ] {
            user.quota_bytes = allocation;
            let mut template = default_template();
            template.quota_bytes = ceiling;
            let effective = resolve(&AppConfig::default(), &user, Some(&template)).unwrap();
            assert_eq!(effective.roots[0].quota_bytes, Some(expected));
        }
    }

    #[test]
    fn automatic_mode_requires_explicit_root_migration_and_web_accounts() {
        let mut config = AppConfig {
            home_template: Some(default_template()),
            ..AppConfig::default()
        };
        assert!(config.validate().is_err());
        config.roots.clear();
        assert!(config.validate().is_err());
        config.web_accounts.enabled = true;
        config.web_accounts.server_uid = Some(995);
        config.validate().unwrap();
        let effective = resolve(&config, &account(), config.home_template.as_ref()).unwrap();
        assert_eq!(effective.roots[0].base_path, account().home);
        assert!(effective.home_template.is_none());
        let decoded: AppConfig = toml::from_str(&toml::to_string(&config).unwrap()).unwrap();
        assert_eq!(decoded, config);
        config.home_template.as_mut().unwrap().root_id = RootId::new("../bad");
        assert!(config.validate().is_err());
        assert!(serde_json::from_str::<crate::config::ConfigPatch>(
            r#"{"home_template":{"root_id":"home"}}"#
        )
        .is_err());
    }

    #[test]
    fn same_root_id_does_not_allow_another_account_snapshot() {
        let config = AppConfig::default();
        let ceiling = resolve(&config, &account(), Some(&default_template())).unwrap();
        let mut other = account();
        other.username = "bob".into();
        other.uid = 1001;
        other.gid = 1001;
        other.home = "/home/bob".into();
        let snapshot = crate::helper::policy::PolicySnapshot::new(
            resolve(&config, &other, Some(&default_template())).unwrap(),
        );
        assert_eq!(
            crate::helper::policy::restrict(&snapshot, &ceiling),
            Err(TealDriveError::PermissionDenied)
        );
    }
}
