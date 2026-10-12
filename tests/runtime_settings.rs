//! Real root-owned critical config, atomic runtime persistence and failed reload recovery.
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use tealdrive::config::{AppConfig, ConfigPatch, DynamicConfig};
use tealdrive::errors::TealDriveError;

#[test]
#[ignore = "requires root to exercise real trusted config ownership"]
fn root_configuration_and_runtime_overlay_are_separate_and_fail_closed() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let dir = std::env::current_dir()
        .unwrap()
        .join(format!(".settings-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let root_path = dir.join("server.toml");
    let settings_path = dir.join("settings.toml");
    let mut base = AppConfig::default();
    base.features.save_text_file = true;
    base.account_policy.admin_users = vec!["root-approved-admin".into()];
    base.audit.path = Some(dir.join("audit.jsonl"));
    let root_bytes = toml::to_string_pretty(&base).unwrap();
    std::fs::write(&root_path, &root_bytes).unwrap();
    let config = DynamicConfig::new(base.clone(), Some(root_path.clone()))
        .with_runtime_settings(settings_path.clone())
        .unwrap();
    let patch: ConfigPatch = serde_json::from_str(r#"{"quota_warning_percent":75,"features":{"save_text_file":false},"audit":{"retention_hours":36}}"#).unwrap();
    config.apply_patch_saved(patch, true).unwrap();
    assert_eq!(std::fs::read_to_string(&root_path).unwrap(), root_bytes);
    let runtime_bytes = std::fs::read_to_string(&settings_path).unwrap();
    assert!(!runtime_bytes.contains("admin_users"));
    assert!(!runtime_bytes.contains("allowed_users"));
    assert_eq!(
        std::fs::metadata(&settings_path).unwrap().mode() & 0o777,
        0o600
    );
    let restarted = DynamicConfig::new(base.clone(), Some(root_path.clone()))
        .with_runtime_settings(settings_path.clone())
        .unwrap();
    assert_eq!(restarted.load().quota_warning_percent, 75);
    assert!(!restarted.load().features.save_text_file);
    for forbidden in [
        r#"{"admin_users":["attacker"]}"#,
        r#"{"allowed_users":["attacker"]}"#,
        r#"{"security":{"allowed_origins":["https://attacker.test"]}}"#,
        r#"{"features":{"chmod_file":true}}"#,
        r#"{"audit":{"path":"/etc/passwd"}}"#,
    ] {
        let patch = serde_json::from_str(forbidden).unwrap();
        assert_eq!(
            config.apply_patch_saved(patch, true).unwrap_err(),
            TealDriveError::PermissionDenied
        );
        assert_eq!(
            std::fs::read_to_string(&settings_path).unwrap(),
            runtime_bytes
        );
        assert_eq!(std::fs::read_to_string(&root_path).unwrap(), root_bytes);
    }
    std::fs::write(&settings_path, "admin_users = ['attacker']").unwrap();
    assert!(config.reload().is_err());
    assert_eq!(config.load().quota_warning_percent, 75);
    assert_eq!(
        config.load().account_policy.admin_users,
        ["root-approved-admin"]
    );
    std::fs::write(&settings_path, &runtime_bytes).unwrap();
    base.account_policy.admin_users = vec!["new-root-admin".into()];
    std::fs::write(&root_path, toml::to_string(&base).unwrap()).unwrap();
    config.reload().unwrap();
    assert_eq!(config.load().account_policy.admin_users, ["new-root-admin"]);
    base.server.bind_addr = "127.0.0.1:3999".into();
    std::fs::write(&root_path, toml::to_string(&base).unwrap()).unwrap();
    assert!(config.reload().is_err());
    assert_ne!(config.load().server.bind_addr, base.server.bind_addr);
    base.server.bind_addr = config.load().server.bind_addr.clone();
    base.audit.retention_hours = None;
    std::fs::write(&root_path, toml::to_string(&base).unwrap()).unwrap();
    config.reload().unwrap();
    let patch = serde_json::from_str(r#"{"audit":{"retention_hours":null}}"#).unwrap();
    config.apply_patch_saved(patch, true).unwrap();
    assert!(std::fs::read_to_string(&settings_path)
        .unwrap()
        .contains("retention_hours = 0"));
    let restarted = DynamicConfig::new(base, Some(root_path))
        .with_runtime_settings(settings_path)
        .unwrap();
    assert_eq!(restarted.load().audit.effective_retention_hours(), None);
    std::fs::remove_dir_all(dir).unwrap();
}
