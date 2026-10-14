#[test]
fn browser_runtime_initialization_is_singleton_and_preserves_rust_handles() {
    let output = std::process::Command::new("node")
        .arg("tests/noise_runtime.cjs")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
