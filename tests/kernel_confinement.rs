use std::fs;
use tealdrive::config::AllowedRoot;
#[test]
fn kernel_denies_filesystem_escape() {
    if let Ok(base) = std::env::var("TDRV_SANDBOX_TEST_CHILD") {
        let root = AllowedRoot {
            base_path: base.into(),
            ..Default::default()
        };
        match tealdrive::helper::sandbox::confine(&root) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("SKIP: kernel confinement unavailable: {e}");
                std::process::exit(77);
            }
        }
        assert_eq!(fs::read(root.base_path.join("inside")).unwrap(), b"inside");
        assert!(fs::read(root.base_path.join("escape/outside")).is_err());
        assert!(fs::write(root.base_path.join("escape/outside"), b"corrupt").is_err());
        assert!(fs::read("/etc/passwd").is_err());
        return;
    }
    let base = std::env::temp_dir().join(format!("tdrv-confinement-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(base.join("root")).unwrap();
    fs::create_dir(base.join("outside")).unwrap();
    fs::write(base.join("root/inside"), b"inside").unwrap();
    fs::write(base.join("outside/outside"), b"safe").unwrap();
    std::os::unix::fs::symlink(base.join("outside"), base.join("root/escape")).unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("kernel_denies_filesystem_escape")
        .arg("--nocapture")
        .env("TDRV_SANDBOX_TEST_CHILD", base.join("root"))
        .output()
        .unwrap();
    if output.status.code() == Some(77) {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
    } else {
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(fs::read(base.join("outside/outside")).unwrap(), b"safe");
    fs::remove_dir_all(base).unwrap();
}
