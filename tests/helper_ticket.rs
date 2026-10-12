//! Opt-in acceptance against a dedicated installed setuid helper and PAM test account.
//! Credentials travel over private pipes, never command arguments or test output.
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output, Stdio};
use tealdrive::auth::AuthRequest;
use tealdrive::helper::ipc::{encode_invocation, HelperInvocation};
use tealdrive::helper::types::*;

fn exchange(mut command: Command, input: &[u8]) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}
fn stage(executable: &std::path::Path, phase: &str) -> Command {
    let mut command = Command::new(executable);
    command.args([
        "--ignored",
        "--exact",
        "bound_ticket_rejects_another_process_of_the_same_uid",
        "--test-threads=1",
    ]);
    command.env("TEALDRIVE_TICKET_STAGE", phase);
    command
}
#[test]
#[ignore = "requires explicit installed setuid helper, root, and dedicated PAM account"]
fn bound_ticket_rejects_another_process_of_the_same_uid() {
    let helper = std::env::var("TEALDRIVE_TEST_HELPER").expect("dedicated helper required");
    if let Ok(phase) = std::env::var("TEALDRIVE_TICKET_STAGE") {
        let mut input = zeroize::Zeroizing::new(Vec::new());
        std::io::stdin().read_to_end(&mut input).unwrap();
        if phase == "replay" {
            assert!(
                !exchange(Command::new(helper), &input).status.success(),
                "cross-process replay accepted"
            );
            return;
        }
        let (request, config, uid, gid): (AuthRequest, tealdrive::config::AppConfig, u32, u32) =
            serde_json::from_slice(&input).unwrap();
        assert_ne!(uid, 0);
        assert_ne!(gid, 0);
        assert_eq!(unsafe { libc::setgroups(0, std::ptr::null()) }, 0);
        assert_eq!(unsafe { libc::setgid(gid) }, 0);
        assert_eq!(unsafe { libc::setuid(uid) }, 0);
        let auth_bytes = zeroize::Zeroizing::new(
            encode_invocation(&HelperInvocation::Authenticate(request)).unwrap(),
        );
        let auth_output = exchange(Command::new(&helper), &auth_bytes);
        assert!(auth_output.status.success());
        let auth = tealdrive::helper::ipc::read_authentication(&mut auth_output.stdout.as_slice())
            .unwrap();
        let (identity, ticket) = auth.result.expect("PAM fixture authentication failed");
        let binding = ticket
            .caller
            .as_ref()
            .expect("ticket must be process-bound");
        assert_eq!(binding.uid, uid);
        assert_eq!(binding.pid, std::process::id());
        let root = &config.roots[0];
        let operation = HelperRequest {
            request_id: tealdrive::protocol::frame::RequestId::new(),
            username: identity.username,
            uid: identity.uid,
            gid: identity.primary_gid,
            command: HelperCommand::QuotaUsage,
            root_id: root.root_id.clone(),
            relative_path: tealdrive::config::RelativePath::new(""),
            target_relative_path: None,
            policy_context: HelperPolicyContext {
                read_only_root: root.read_only,
                is_web_root: root.is_web_root,
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
        let bytes = zeroize::Zeroizing::new(
            encode_invocation(&HelperInvocation::Operation {
                grant: None,
                request: Box::new(operation),
                ticket: Some(ticket),
                policy: Some(Box::new(tealdrive::helper::policy::PolicySnapshot::new(
                    config,
                ))),
            })
            .unwrap(),
        );
        let good = exchange(Command::new(&helper), &bytes);
        assert!(good.status.success(), "legitimate driver rejected");
        let response =
            tealdrive::helper::ipc::read_helper_response(&mut good.stdout.as_slice()).unwrap();
        assert!(response.success, "legitimate operation failed");
        let replay = exchange(stage(&std::env::current_exe().unwrap(), "replay"), &bytes);
        assert!(replay.status.success(), "same-UID replay rejection failed");
        return;
    }
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let password_path = std::env::var("TEALDRIVE_TEST_PASSWORD_FILE").unwrap();
    let password = zeroize::Zeroizing::new(std::fs::read_to_string(password_path).unwrap());
    let request = AuthRequest::new(
        std::env::var("TEALDRIVE_TEST_USERNAME").unwrap(),
        password.trim(),
    );
    let uid: u32 = std::env::var("TEALDRIVE_TEST_SERVICE_UID")
        .unwrap()
        .parse()
        .unwrap();
    let gid: u32 = std::env::var("TEALDRIVE_TEST_SERVICE_GID")
        .unwrap()
        .parse()
        .unwrap();
    let config = tealdrive::helper::security::trusted_config().unwrap();
    let fixture = std::env::temp_dir().join(format!("tealdrive-ticket-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&fixture).unwrap();
    let executable = fixture.join("test");
    std::fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
    std::fs::set_permissions(&fixture, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
    let input = zeroize::Zeroizing::new(serde_json::to_vec(&(request, config, uid, gid)).unwrap());
    let result = exchange(stage(&executable, "driver"), &input);
    std::fs::remove_dir_all(fixture).unwrap();
    assert!(
        result.status.success(),
        "real helper process-binding acceptance failed"
    );
}
