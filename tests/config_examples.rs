#[test]
fn deployment_examples_validate_and_share_roots() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let server =
        tealdrive::config::AppConfig::from_file(base.join("docs/deployment/config.toml")).unwrap();
    let helper =
        tealdrive::config::AppConfig::from_file(base.join("docs/deployment/helper.toml")).unwrap();
    assert_eq!(server.roots, helper.roots);
    assert!(server.server.behind_tls_proxy);
    assert!(server.session.secure_cookies);
    assert!(!server.account_policy.admin_users.is_empty());
    tealdrive::helper::policy::restrict(
        &tealdrive::helper::policy::PolicySnapshot::new(server),
        &helper,
    )
    .unwrap();
}
