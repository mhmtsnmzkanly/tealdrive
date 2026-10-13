//! Root-only acceptance of private delegation, tamper rejection and irreversible user access.
use std::{
    io::Write,
    os::unix::fs::PermissionsExt,
    process::{Command, Stdio},
};
use tealdrive::{
    config::{AllowedRoot, AppConfig, RelativePath, RootId},
    helper::{
        ipc::{encode_invocation, read_helper_response, HelperInvocation},
        types::*,
        worker::WorkerInvocation,
    },
    protocol::RequestId,
};
#[test]
#[ignore = "requires Root and installed private helper key"]
fn private_worker_rejects_forgery_and_cannot_read_root_owned_content() {
    if std::env::var_os("TEALDRIVE_WORKER_CHILD").is_some() {
        let invocation: WorkerInvocation = serde_json::from_reader(std::io::stdin()).unwrap();
        let uid = invocation.caller_uid;
        assert_eq!(unsafe { libc::setgroups(0, std::ptr::null()) }, 0);
        assert_eq!(unsafe { libc::setresgid(uid, 0, 0) }, 0);
        assert_eq!(unsafe { libc::setresuid(uid, 0, 0) }, 0);
        let mut invocation = invocation;
        invocation.parent_pid = std::process::id();
        let mode = std::env::var("TEALDRIVE_WORKER_TAMPER").unwrap_or_default();
        if mode == "expired" {
            invocation.expires_at = 1;
        }
        let payload = worker_payload(&invocation);
        let key = zeroize::Zeroizing::new(std::fs::read(tealdrive::layout::HELPER_KEY).unwrap());
        use hmac::{Hmac, Mac};
        let mut mac = Hmac::<sha2::Sha256>::new_from_slice(&key).unwrap();
        mac.update(&payload);
        invocation.mac = mac.finalize().into_bytes().into();
        if mode == "path" {
            invocation.request.relative_path = RelativePath::new("different.txt");
        }
        if mode == "uid" {
            invocation.request.uid = 0;
        }
        if mode == "unprivileged" {
            assert_eq!(unsafe { libc::setresgid(uid, uid, uid) }, 0);
            assert_eq!(unsafe { libc::setresuid(uid, uid, uid) }, 0);
        }
        let executable = if mode == "unprivileged" {
            "/home/tealdrive/bin/tealdrive-helper"
        } else {
            env!("CARGO_BIN_EXE_tealdrive-helper")
        };
        let mut child = Command::new(executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        input
            .write_all(&encode_invocation(&HelperInvocation::Worker(invocation)).unwrap())
            .unwrap();
        let mut output = child.stdout.take().unwrap();
        let response = read_helper_response(&mut output);
        let status = child.wait().unwrap();
        if std::env::var_os("TEALDRIVE_WORKER_TAMPER").is_some() {
            assert!(!status.success());
            assert!(response.is_err());
        } else {
            assert!(status.success());
            assert!(
                !response.unwrap().success,
                "worker read Root600 file after UID drop"
            );
        }
        drop(input);
        return;
    }
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let cfg = tealdrive::helper::security::trusted_config().unwrap();
    let service_uid = cfg.web_accounts.server_uid.expect("installed service UID");
    let home = std::path::Path::new("/home").join(format!(".td-worker-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&home).unwrap();
    std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o755)).unwrap();
    let secret = home.join("secret.txt");
    std::fs::write(&secret, b"Root-only fixture content").unwrap();
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o600)).unwrap();
    let request = HelperRequest {
        request_id: RequestId::new(),
        username: "nobody".into(),
        uid: 65534,
        gid: 65534,
        command: HelperCommand::ReadTextFile,
        root_id: RootId::new("home"),
        relative_path: RelativePath::new("secret.txt"),
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
        chmod_mode: None,
        archive_source_paths: None,
        search_options: None,
    };
    let config = AppConfig {
        roots: vec![AllowedRoot {
            base_path: home.clone(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let invocation = WorkerInvocation {
        undo: None,
        operation_id: None,
        request: Box::new(request),
        config: Box::new(config),
        available: None,
        parent_pid: 1,
        caller_uid: service_uid,
        expires_at: tealdrive::session::store::InMemorySessionStore::now_epoch_seconds() + 300,
        nonce: *uuid::Uuid::new_v4().as_bytes(),
        mac: [0; 32],
    };
    for mode in ["", "path", "uid", "expired", "unprivileged"] {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--ignored",
                "--exact",
                "private_worker_rejects_forgery_and_cannot_read_root_owned_content",
                "--test-threads=1",
            ])
            .env("TEALDRIVE_WORKER_CHILD", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        if !mode.is_empty() {
            command.env("TEALDRIVE_WORKER_TAMPER", mode);
        }
        let mut child = command.spawn().unwrap();
        serde_json::to_writer(child.stdin.take().unwrap(), &invocation).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    std::fs::remove_file(secret).unwrap();
    std::fs::remove_dir(home).unwrap();
}
fn worker_payload(i: &WorkerInvocation) -> Vec<u8> {
    let mut data = b"tealdrive-internal-worker-v1\0".to_vec();
    data.extend(
        rmp_serde::to_vec_named(&(
            &i.request,
            &i.config,
            i.available,
            i.parent_pid,
            i.caller_uid,
            i.expires_at,
            i.nonce,
            &i.undo,
            &i.operation_id,
        ))
        .unwrap(),
    );
    data
}
