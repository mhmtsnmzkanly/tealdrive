//! Actual unprivileged owner can recover mode 000 without granting root filesystem work.
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::process::Command;
use tealdrive::config::{AllowedRoot, RelativePath, RootId, SensitivePolicyConfig};
use tealdrive::helper::types::*;
#[test]
#[ignore = "requires root fixture ownership setup and Linux 6.6+ for mode-000 recovery"]
fn owner_recovers_unreadable_permissions_after_uid_drop() {
    if let Ok(directory) = std::env::var("TEALDRIVE_CHMOD_OWNER_CHILD") {
        assert_eq!(unsafe { libc::setgroups(0, std::ptr::null()) }, 0);
        assert_eq!(unsafe { libc::setgid(65534) }, 0);
        assert_eq!(unsafe { libc::setuid(65534) }, 0);
        let root = AllowedRoot {
            root_id: RootId::new("home"),
            base_path: directory.into(),
            is_web_root: false,
            ..Default::default()
        };
        assert!(std::fs::File::open(root.base_path.join("owned")).is_err());
        let mut request = HelperRequest {
            request_id: tealdrive::protocol::frame::RequestId::new(),
            username: "nobody".into(),
            uid: 65534,
            gid: 65534,
            command: HelperCommand::Chmod,
            root_id: root.root_id.clone(),
            relative_path: RelativePath::new("owned"),
            target_relative_path: None,
            policy_context: HelperPolicyContext {
                read_only_root: false,
                is_web_root: false,
            },
            limits: HelperLimits {
                max_directory_page_size: 100,
                max_text_edit_size: 1024,
                max_chunk_size: 1024,
                max_download_helper_bytes: 1024,
            },
            list_options: None,
            write_options: None,
            chmod_mode: Some(tealdrive::protocol::schema::UnixMode(0o600)),
            archive_source_paths: None,
            search_options: None,
        };
        tealdrive::helper::sandbox::confine(&root).unwrap();
        let response = tealdrive::helper::chmod::chmod_file_with_roots(
            &request,
            std::slice::from_ref(&root),
            &SensitivePolicyConfig::default(),
        );
        assert!(response.success, "owner mode-000 recovery failed");
        assert_eq!(
            std::fs::metadata(root.base_path.join("owned"))
                .unwrap()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::read_to_string(root.base_path.join("owned")).unwrap(),
            "preserved-content"
        );
        request.relative_path = RelativePath::new("other-owner");
        assert!(
            !tealdrive::helper::chmod::chmod_file_with_roots(
                &request,
                std::slice::from_ref(&root),
                &SensitivePolicyConfig::default()
            )
            .success
        );
        request.relative_path = RelativePath::new("alias");
        assert!(
            !tealdrive::helper::chmod::chmod_file_with_roots(
                &request,
                &[root],
                &SensitivePolicyConfig::default()
            )
            .success
        );
        return;
    }
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let fixture = tealdrive::test_support::scratch_dir()
        .join(format!("tealdrive-chmod-owner-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&fixture).unwrap();
    std::fs::set_permissions(&fixture, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(fixture.join("owned"), "preserved-content").unwrap();
    let path =
        std::ffi::CString::new(fixture.join("owned").as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::chown(path.as_ptr(), 65534, 65534) }, 0);
    for filename in ["owned", "other-owner"] {
        if filename == "other-owner" {
            std::fs::write(fixture.join(filename), "root-content").unwrap();
        }
        std::fs::set_permissions(
            fixture.join(filename),
            std::fs::Permissions::from_mode(0o000),
        )
        .unwrap();
    }
    std::os::unix::fs::symlink("other-owner", fixture.join("alias")).unwrap();
    let executable = fixture.join("test");
    std::fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
    let result = Command::new(executable)
        .args([
            "--exact",
            "owner_recovers_unreadable_permissions_after_uid_drop",
            "--ignored",
            "--test-threads=1",
        ])
        .env("TEALDRIVE_CHMOD_OWNER_CHILD", &fixture)
        .output()
        .unwrap();
    let root_mode = std::fs::metadata(fixture.join("other-owner"))
        .unwrap()
        .mode()
        & 0o777;
    std::fs::remove_dir_all(fixture).unwrap();
    assert!(
        result.status.success(),
        "unprivileged chmod acceptance failed: {}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert_eq!(root_mode, 0);
}
