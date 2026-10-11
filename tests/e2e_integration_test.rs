use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use tealdrive::config::{AllowedRoot, AppConfig, RelativePath, RootId};
use tealdrive::errors::ErrorKind;
use tealdrive::helper::client::HelperProcessClient;
use tealdrive::helper::types::{
    HelperCommand, HelperLimits, HelperPolicyContext, HelperRequest, HelperWriteOptions,
};
use tealdrive::protocol::frame::RequestId;

fn temp_dir() -> PathBuf {
    let path = std::env::temp_dir().join(format!("tealdrive-e2e-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&path).expect("create temp dir");
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o777)).unwrap();
    path.canonicalize().expect("canonicalize temp dir")
}

fn sample_request(
    command: HelperCommand,
    rel: &str,
    target: Option<&str>,
    write_options: Option<HelperWriteOptions>,
    sources: Option<Vec<&str>>,
) -> HelperRequest {
    HelperRequest {
        request_id: RequestId::new(),
        username: "nobody".to_owned(),
        uid: 65534,
        gid: 65534,
        command,
        root_id: RootId::new("home"),
        relative_path: RelativePath::new(rel),
        target_relative_path: target.map(RelativePath::new),
        policy_context: HelperPolicyContext {
            read_only_root: false,
            is_web_root: false,
        },
        limits: HelperLimits {
            max_directory_page_size: 100,
            max_text_edit_size: 1024 * 1024,
            max_chunk_size: 1024 * 1024,
            max_download_helper_bytes: 10 * 1024 * 1024,
        },
        list_options: None,
        write_options,
        chmod_mode: None,
        archive_source_paths: sources.map(|s| s.into_iter().map(RelativePath::new).collect()),
        search_options: None,
    }
}

#[test]
#[ignore = "requires root with CAP_SETUID/CAP_SETGID, or an unprivileged matching test identity"]
fn test_real_helper_binary_subprocess_lifecycle() {
    let helper_binary = PathBuf::from(env!("CARGO_BIN_EXE_tealdrive-helper"));
    assert!(
        helper_binary.is_file(),
        "Compiled helper binary must exist: {:?}",
        helper_binary
    );

    let base = temp_dir();
    let client = HelperProcessClient::new(helper_binary, Duration::from_secs(5));
    let mut config = AppConfig {
        roots: vec![AllowedRoot {
            base_path: base.clone(),
            ..AllowedRoot::default()
        }],
        ..AppConfig::default()
    };
    config.features.extract_archive = true;
    config.features.save_text_file = true;
    let snapshot = tealdrive::helper::policy::PolicySnapshot::new(config);
    let execute = |request: &HelperRequest| {
        tealdrive::helper::policy::with_snapshot(&snapshot.config, || client.execute(request))
    };

    // 1. Create directory
    let create_req = sample_request(
        HelperCommand::CreateFolder,
        "",
        Some("workspace"),
        None,
        None,
    );
    let create_resp = execute(&create_req).expect("helper execute create");
    assert!(
        create_resp.success,
        "create failed: {:?}",
        create_resp.safe_message
    );
    assert!(base.join("workspace").is_dir());

    // 2. Write file
    let content = b"Hello from real subprocess tealdrive-helper!\n";
    let write_opts = HelperWriteOptions {
        data: content.to_vec(),
        offset: 0,
        truncate: true,
    };
    let write_req = sample_request(
        HelperCommand::WriteFile,
        "workspace/hello.txt",
        None,
        Some(write_opts),
        None,
    );
    let write_resp = execute(&write_req).expect("helper execute write");
    assert!(
        write_resp.success,
        "write failed: {:?}",
        write_resp.safe_message
    );
    assert_eq!(
        fs::read(base.join("workspace/hello.txt")).expect("read written file"),
        content
    );

    // 3. List directory
    let list_req = sample_request(HelperCommand::ListDirectory, "workspace", None, None, None);
    let list_resp = execute(&list_req).expect("helper execute list");
    assert!(
        list_resp.success,
        "list failed: {:?}",
        list_resp.safe_message
    );
    let tealdrive::helper::types::HelperResponsePayload::DirectoryList(list_data) =
        list_resp.payload
    else {
        panic!("expected DirectoryList payload");
    };
    assert_eq!(list_data.entries.len(), 1);
    assert_eq!(list_data.entries[0].name, "hello.txt");

    // 4. Compress to zip
    let zip_req = sample_request(
        HelperCommand::CompressZip,
        "",
        Some("workspace.zip"),
        None,
        Some(vec!["workspace/hello.txt"]),
    );
    let zip_resp = execute(&zip_req).expect("helper execute zip");
    assert!(zip_resp.success, "zip failed: {:?}", zip_resp.safe_message);
    assert!(base.join("workspace.zip").is_file());

    // 5. Extract archive
    let ext_req = sample_request(
        HelperCommand::ExtractArchive,
        "workspace.zip",
        Some("unpacked_zip"),
        None,
        None,
    );
    let ext_resp = execute(&ext_req).expect("helper execute extract");
    assert!(
        ext_resp.success,
        "extract failed: {:?}",
        ext_resp.safe_message
    );
    assert!(base.join("unpacked_zip/hello.txt").is_file());
    assert_eq!(
        fs::read(base.join("unpacked_zip/hello.txt")).expect("read extracted"),
        content
    );

    let _ = fs::remove_dir_all(base);
}

#[test]
fn test_real_helper_binary_rejects_root_and_unverified_identity() {
    let helper_binary = PathBuf::from(env!("CARGO_BIN_EXE_tealdrive-helper"));
    let base = temp_dir();
    let client = HelperProcessClient::new(helper_binary, Duration::from_secs(5))
        .with_env("TEALDRIVE_HELPER_ROOTS", format!("home={}", base.display()));

    // Refuses uid 0
    let mut root_req = sample_request(HelperCommand::ListDirectory, "", None, None, None);
    root_req.uid = 0;
    let root_resp = client.execute(&root_req).expect("execute");
    assert!(!root_resp.success);
    assert_eq!(root_resp.error_kind, Some(ErrorKind::PermissionDenied));

    // An unverified identity cannot use traversal to reach a managed file.
    let traversal_req = sample_request(HelperCommand::ListDirectory, "../../etc", None, None, None);
    let traversal_resp = client.execute(&traversal_req).expect("execute");
    assert!(!traversal_resp.success);

    // Unknown roots are denied independently of identity switching.
    let mut unknown_root_req = sample_request(HelperCommand::ListDirectory, "", None, None, None);
    unknown_root_req.root_id = RootId::new("non_existent_root");
    let unknown_resp = client.execute(&unknown_root_req).expect("execute");
    assert!(!unknown_resp.success);

    let _ = fs::remove_dir_all(base);
}

// Deliberately bypasses OS impersonation only in transport tests. Real helper
// privilege and kernel confinement are tested separately.
#[derive(Clone)]
struct TestHelper {
    config: AppConfig,
}
macro_rules! backend {
    ($($trait:path),+ $(,)?) => { $(impl $trait for TestHelper {
        fn execute_helper(&self, request: &HelperRequest) -> Result<tealdrive::helper::types::HelperResponse, tealdrive::errors::TealDriveError> {
            Ok(tealdrive::helper::ipc::handle_helper_request_with_config(request, &self.config))
        }
    })+ };
}
backend!(
    tealdrive::fs::listing::DirectoryListingHelper,
    tealdrive::fs::metadata::MetadataHelper,
    tealdrive::fs::text::TextPreviewHelper,
    tealdrive::fs::text::SaveTextFileHelper,
    tealdrive::fs::create_folder::CreateFolderHelper,
    tealdrive::fs::rename::RenameHelper,
    tealdrive::fs::move_copy::MoveHelper,
    tealdrive::fs::move_copy::CopyHelper,
    tealdrive::fs::trash::MoveToTrashHelper,
    tealdrive::fs::trash::RestoreFromTrashHelper,
    tealdrive::fs::trash::DeletePermanentlyHelper,
    tealdrive::fs::trash::ListTrashHelper,
    tealdrive::fs::chmod::ChmodFileHelper,
    tealdrive::fs::archive::CompressZipHelper,
    tealdrive::fs::archive::CompressTarGzHelper,
    tealdrive::fs::archive::ExtractArchiveHelper,
    tealdrive::fs::search::SearchFilesHelper,
    tealdrive::download::transfer::DownloadHelper,
    tealdrive::upload::transfer::UploadHelper,
);
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use tower::ServiceExt;
struct Fixture {
    root: PathBuf,
    router: Router,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn fixture(admin: bool) -> Fixture {
    use tealdrive::auth::account::{MockAuthenticator, MockUser};
    use tealdrive::ws::server::{build_router, WsServerState};
    let root = temp_dir();
    let mut config = AppConfig {
        roots: vec![AllowedRoot {
            base_path: root.clone(),
            ..AllowedRoot::default()
        }],
        ..AppConfig::default()
    };
    if admin {
        config.account_policy.admin_users = vec!["alice".into()];
    }
    let auth = MockAuthenticator::new(config.account_policy.clone())
        .with_user(MockUser::regular("alice", "secret123"));
    let state = WsServerState::with_authenticator(
        config.clone(),
        tealdrive::session::store::InMemorySessionStore::new(config.session.clone()),
        TestHelper { config },
        tealdrive::audit::sink::JsonlAuditSink::open(root.join("audit.jsonl")).unwrap(),
        std::sync::Arc::new(auth),
        tealdrive::rate_limit::login::InMemoryLoginRateLimiter::new(5),
    );
    Fixture {
        root,
        router: build_router(state),
    }
}
async fn call(
    router: &Router,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
    body: &str,
) -> axum::response::Response {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "localhost")
        .header("content-type", "application/json");
    if let Some(cookie) = cookie {
        req = req.header("cookie", cookie);
    }
    if let Some(csrf) = csrf {
        req = req.header("x-csrf-token", csrf);
    }
    router
        .clone()
        .oneshot(req.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap()
}
async fn json(response: axum::response::Response) -> serde_json::Value {
    serde_json::from_slice(
        &to_bytes(response.into_body(), 8 * 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap()
}
async fn login(router: &Router) -> (String, String) {
    let response = call(
        router,
        "POST",
        "/api/login",
        None,
        None,
        r#"{"username":"alice","password":"secret123"}"#,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let data = json(response).await;
    (cookie, data["csrf_token"].as_str().unwrap().to_owned())
}
#[tokio::test]
async fn http_login_session_logout_enforces_stable_csrf() {
    let f = fixture(false);
    let (cookie, csrf) = login(&f.router).await;
    let data = json(call(&f.router, "GET", "/api/session", Some(&cookie), None, "").await).await;
    assert_eq!(data["csrf_token"], csrf);
    assert_eq!(data["is_admin"], false);
    assert_eq!(
        call(
            &f.router,
            "GET",
            "/api/admin/config",
            Some(&cookie),
            None,
            ""
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&f.router, "POST", "/api/logout", Some(&cookie), None, "")
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &f.router,
            "POST",
            "/api/logout",
            Some(&cookie),
            Some("wrong"),
            ""
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &f.router,
            "POST",
            "/api/logout",
            Some(&cookie),
            Some(&csrf),
            ""
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        json(call(&f.router, "GET", "/api/session", Some(&cookie), None, "").await).await
            ["authenticated"],
        false
    );
}
#[tokio::test]
async fn admin_partial_patch_and_failed_persistence_are_transactional() {
    let f = fixture(true);
    let (cookie, csrf) = login(&f.router).await;
    let patch = r#"{"features":{"save_text_file":true}}"#;
    assert_eq!(
        call(
            &f.router,
            "PATCH",
            "/api/admin/config",
            Some(&cookie),
            None,
            patch
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &f.router,
            "PATCH",
            "/api/admin/config",
            Some(&cookie),
            Some(&csrf),
            patch
        )
        .await
        .status(),
        StatusCode::OK
    );
    let before = json(
        call(
            &f.router,
            "GET",
            "/api/admin/config",
            Some(&cookie),
            None,
            "",
        )
        .await,
    )
    .await;
    assert_eq!(before["features"]["save_text_file"], true);
    assert_eq!(
        call(
            &f.router,
            "PATCH",
            "/api/admin/config?save=true",
            Some(&cookie),
            Some(&csrf),
            r#"{"features":{"save_text_file":false}}"#
        )
        .await
        .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    let after = json(
        call(
            &f.router,
            "GET",
            "/api/admin/config",
            Some(&cookie),
            None,
            "",
        )
        .await,
    )
    .await;
    assert_eq!(before, after);
}
#[tokio::test]
async fn http_ui_assets_and_api_fallback() {
    let f = fixture(false);
    for (path, mime, needle) in [
        ("/index.html", "text/html", "TealDrive"),
        ("/app.js", "application/javascript", "TdrvContract"),
        ("/style.css", "text/css", "--teal-500"),
        ("/favicon.svg", "image/svg+xml", "svg"),
        ("/files/folder", "text/html", "TealDrive"),
    ] {
        let response = call(&f.router, "GET", path, None, None, "").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers()["content-type"]
            .to_str()
            .unwrap()
            .contains(mime));
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains(needle));
    }
    assert_eq!(
        call(&f.router, "GET", "/api/unknown", None, None, "")
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&f.router, "GET", "/api/health", None, None, "")
            .await
            .status(),
        StatusCode::OK
    );
}
#[tokio::test]
async fn http_download_streams_large_files_ranges_and_safe_previews() {
    let f = fixture(false);
    let content = vec![0xA5; 1024 * 1024 + 17];
    fs::write(f.root.join("large.bin"), &content).unwrap();
    fs::write(
        f.root.join("sample.txt"),
        b"0123456789abcdefghijklmnopqrstuvwxyz",
    )
    .unwrap();
    fs::write(f.root.join("page.html"), b"<script>alert(1)</script>").unwrap();
    fs::create_dir(f.root.join("nested")).unwrap();
    fs::write(f.root.join("nested/.secret"), b"secret").unwrap();
    let (cookie, _) = login(&f.router).await;
    let response = call(
        &f.router,
        "GET",
        "/api/download?root_id=home&path=large.bin",
        Some(&cookie),
        None,
        "",
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(response.into_body(), 2 * 1024 * 1024)
            .await
            .unwrap()
            .as_ref(),
        &content
    );
    let request = Request::builder()
        .uri("/api/download?root_id=home&path=sample.txt")
        .header("cookie", &cookie)
        .header("range", "bytes=10-19")
        .body(Body::empty())
        .unwrap();
    let response = f.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()["content-range"], "bytes 10-19/36");
    assert_eq!(
        to_bytes(response.into_body(), 100).await.unwrap().as_ref(),
        b"abcdefghij"
    );
    assert_eq!(
        call(
            &f.router,
            "GET",
            "/api/download?root_id=home&path=sample.txt",
            None,
            None,
            ""
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &f.router,
            "GET",
            "/api/download?root_id=home&path=../../etc/passwd",
            Some(&cookie),
            None,
            ""
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert!(!call(
        &f.router,
        "GET",
        "/api/download?root_id=home&path=nested/.secret",
        Some(&cookie),
        None,
        ""
    )
    .await
    .status()
    .is_success());
    let response = call(
        &f.router,
        "GET",
        "/api/preview?root_id=home&path=page.html",
        Some(&cookie),
        None,
        "",
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers()["content-disposition"]
        .to_str()
        .unwrap()
        .starts_with("attachment"));
    assert!(response.headers()["content-security-policy"]
        .to_str()
        .unwrap()
        .contains("sandbox"));
}
