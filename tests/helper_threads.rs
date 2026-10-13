//! Kernel proof: threads created after confinement inherit filesystem limits and no_new_privs.
use std::{os::unix::fs::PermissionsExt, process::Command};
#[test]
#[ignore = "requires Root identity drop and Linux Landlock"]
fn descendants_inherit_user_filesystem_confinement() {
    if let Some(scope) = std::env::var_os("TEALDRIVE_THREAD_SCOPE") {
        let scope = std::path::PathBuf::from(scope);
        let outside = scope.parent().unwrap().join("outside.txt");
        tealdrive::helper::security::enter_identity(&tealdrive::config::UserContext {
            username: "nobody".into(),
            uid: 65534,
            gid: 65534,
        })
        .unwrap();
        assert!(
            std::fs::read(&outside).is_ok(),
            "baseline DAC must permit outside fixture"
        );
        let root = tealdrive::config::AllowedRoot {
            base_path: scope.clone(),
            ..Default::default()
        };
        tealdrive::helper::sandbox::confine(&root).unwrap();
        std::thread::spawn(move || {
            assert_eq!(
                unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) },
                1
            );
            assert!(std::fs::read(scope.join("inside.txt")).is_ok());
            assert!(std::fs::read(&outside).is_err());
            std::thread::spawn(move || {
                assert!(std::fs::read(outside).is_err());
            })
            .join()
            .unwrap();
        })
        .join()
        .unwrap();
        return;
    }
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let parent = std::env::temp_dir().join(format!("tealdrive-thread-{}", uuid::Uuid::new_v4()));
    let scope = parent.join("scope");
    std::fs::create_dir_all(&scope).unwrap();
    for directory in [&parent, &scope] {
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    for file in [parent.join("outside.txt"), scope.join("inside.txt")] {
        std::fs::write(&file, b"public synthetic fixture").unwrap();
        std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "descendants_inherit_user_filesystem_confinement",
            "--test-threads=1",
        ])
        .env("TEALDRIVE_THREAD_SCOPE", &scope)
        .status()
        .unwrap();
    std::fs::remove_file(parent.join("outside.txt")).unwrap();
    std::fs::remove_file(scope.join("inside.txt")).unwrap();
    std::fs::remove_dir(scope).unwrap();
    std::fs::remove_dir(parent).unwrap();
    assert!(status.success());
}
