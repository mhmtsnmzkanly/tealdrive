//! Exercise permission confirmation, invalid modes and login secret lifetime.
#[test]
fn permission_confirmation_and_login_secret_lifetime() {
    let script = format!(
        "{}\n{}\nconst application = {};\n{}",
        tealdrive::protocol::JAVASCRIPT_CONTRACT,
        include_str!("../src/ui/protocol.js"),
        serde_json::to_string(tealdrive::ui::assets::APP_JS).unwrap(),
        include_str!("browser_permissions.js")
    );
    use std::io::Write;
    let mut node = std::process::Command::new("node")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Node.js is required for browser tests");
    node.stdin
        .take()
        .unwrap()
        .write_all(script.as_bytes())
        .unwrap();
    let result = node.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
