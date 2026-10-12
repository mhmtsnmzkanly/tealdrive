#[test]
fn sdk_transport_state_filters_quota_and_transfers() {
    let script = format!(
        "{}\n{}\nconst contractSource={};\nconst sdkSource={};\n{}",
        tealdrive::protocol::JAVASCRIPT_CONTRACT,
        include_str!("../src/ui/protocol.js"),
        serde_json::to_string(tealdrive::protocol::JAVASCRIPT_CONTRACT).unwrap(),
        serde_json::to_string(include_str!("../src/ui/sdk/client.js")).unwrap(),
        include_str!("sdk_client.js")
    );
    use std::io::Write;
    let mut node = std::process::Command::new("node")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
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
