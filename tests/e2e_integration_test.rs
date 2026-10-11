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
        username: "tester".to_owned(),
        uid: 1000,
        gid: 1000,
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
fn test_real_helper_binary_subprocess_lifecycle() {
    let helper_binary = PathBuf::from(env!("CARGO_BIN_EXE_tealdrive-helper"));
    assert!(
        helper_binary.is_file(),
        "Compiled helper binary must exist: {:?}",
        helper_binary
    );

    let base = temp_dir();
    let client = HelperProcessClient::new(helper_binary, Duration::from_secs(5))
        .with_env("TEALDRIVE_HELPER_ROOTS", format!("home={}", base.display()));

    // 1. Create directory
    let create_req = sample_request(HelperCommand::CreateFolder, "", Some("workspace"), None, None);
    let create_resp = client.execute(&create_req).expect("helper execute create");
    assert!(create_resp.success, "create failed: {:?}", create_resp.safe_message);
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
    let write_resp = client.execute(&write_req).expect("helper execute write");
    assert!(write_resp.success, "write failed: {:?}", write_resp.safe_message);
    assert_eq!(
        fs::read(base.join("workspace/hello.txt")).expect("read written file"),
        content
    );

    // 3. List directory
    let list_req = sample_request(HelperCommand::ListDirectory, "workspace", None, None, None);
    let list_resp = client.execute(&list_req).expect("helper execute list");
    assert!(list_resp.success, "list failed: {:?}", list_resp.safe_message);
    let tealdrive::helper::types::HelperResponsePayload::DirectoryList(list_data) = list_resp.payload else {
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
    let zip_resp = client.execute(&zip_req).expect("helper execute zip");
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
    let ext_resp = client.execute(&ext_req).expect("helper execute extract");
    assert!(ext_resp.success, "extract failed: {:?}", ext_resp.safe_message);
    assert!(base.join("unpacked_zip/hello.txt").is_file());
    assert_eq!(
        fs::read(base.join("unpacked_zip/hello.txt")).expect("read extracted"),
        content
    );

    let _ = fs::remove_dir_all(base);
}

#[test]
fn test_real_helper_binary_security_enforcement() {
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

    // Refuses directory traversal
    let traversal_req = sample_request(
        HelperCommand::ListDirectory,
        "../../etc",
        None,
        None,
        None,
    );
    let traversal_resp = client.execute(&traversal_req).expect("execute");
    assert!(!traversal_resp.success);

    // Refuses unknown root id
    let mut unknown_root_req = sample_request(HelperCommand::ListDirectory, "", None, None, None);
    unknown_root_req.root_id = RootId::new("non_existent_root");
    let unknown_resp = client.execute(&unknown_root_req).expect("execute");
    assert!(!unknown_resp.success);

    let _ = fs::remove_dir_all(base);
}

#[tokio::test]
async fn test_e2e_server_http_login_session_lifecycle() {
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    use tealdrive::audit::sink::JsonlAuditSink;
    use tealdrive::auth::account::{MockAuthenticator, MockUser};
    use tealdrive::config::AccountPolicyConfig;
    use tealdrive::rate_limit::login::InMemoryLoginRateLimiter;
    use tealdrive::session::store::InMemorySessionStore;
    use tealdrive::ws::server::{build_router, WsServerState};

    let helper_binary = PathBuf::from(env!("CARGO_BIN_EXE_tealdrive-helper"));
    let client = HelperProcessClient::new(helper_binary, Duration::from_secs(5));

    let auth = MockAuthenticator::new(AccountPolicyConfig::default())
        .with_user(MockUser::regular("alice", "secret123"));

    let config = AppConfig {
        roots: vec![AllowedRoot {
            root_id: RootId::new("home"),
            base_path: temp_dir(),
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
        }],
        ..AppConfig::default()
    };

    let session_store = InMemorySessionStore::new(config.session.clone());
    let audit_sink = JsonlAuditSink::open_default().expect("sink");
    let state = WsServerState::with_authenticator(
        config,
        session_store,
        client,
        audit_sink,
        Arc::new(auth),
        InMemoryLoginRateLimiter::new(5),
    );

    let router = build_router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind port");
    let addr = listener.local_addr().expect("local addr");

    tokio::spawn(async move {
        axum::serve(listener, router.into_make_service())
            .await
            .expect("server run");
    });

    // 1. Health check GET /
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .expect("write");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read");
    let resp_str = String::from_utf8_lossy(&buf);
    assert!(resp_str.contains("HTTP/1.1 200 OK"));
    assert!(resp_str.contains("ok"));

    // 2. Login POST /api/login
    let body = r#"{"username":"alice","password":"secret123"}"#;
    let login_req = format!(
        "POST /api/login HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    stream.write_all(login_req.as_bytes()).await.expect("write");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read");
    let resp_str = String::from_utf8_lossy(&buf);
    assert!(resp_str.contains("HTTP/1.1 200 OK"));
    assert!(resp_str.contains("set-cookie: tealdrive_session="));
    assert!(resp_str.contains(r#""username":"alice""#));
}

#[tokio::test]
async fn test_e2e_web_ui_and_static_assets_serving() {
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    use tealdrive::audit::sink::JsonlAuditSink;
    use tealdrive::auth::account::{MockAuthenticator, MockUser};
    use tealdrive::config::AccountPolicyConfig;
    use tealdrive::rate_limit::login::InMemoryLoginRateLimiter;
    use tealdrive::session::store::InMemorySessionStore;
    use tealdrive::ws::server::{build_router, WsServerState};

    let helper_binary = PathBuf::from(env!("CARGO_BIN_EXE_tealdrive-helper"));
    let client = HelperProcessClient::new(helper_binary, Duration::from_secs(5));

    let auth = MockAuthenticator::new(AccountPolicyConfig::default())
        .with_user(MockUser::regular("alice", "secret123"));

    let config = AppConfig {
        roots: vec![AllowedRoot {
            root_id: RootId::new("home"),
            base_path: temp_dir(),
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
        }],
        ..AppConfig::default()
    };

    let session_store = InMemorySessionStore::new(config.session.clone());
    let audit_sink = JsonlAuditSink::open_default().expect("sink");
    let state = WsServerState::with_authenticator(
        config,
        session_store,
        client,
        audit_sink,
        Arc::new(auth),
        InMemoryLoginRateLimiter::new(5),
    );

    let router = build_router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind port");
    let addr = listener.local_addr().expect("local addr");

    tokio::spawn(async move {
        axum::serve(listener, router.into_make_service())
            .await
            .expect("server run");
    });

    async fn get_req(addr: std::net::SocketAddr, path: &str) -> String {
        let mut stream = TcpStream::connect(addr).await.expect("connect");
        let req = format!("GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n", path);
        stream.write_all(req.as_bytes()).await.expect("write");
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.expect("read");
        String::from_utf8_lossy(&buf).to_string()
    }

    // 1. GET /index.html (HTML & Security Headers)
    let index_resp = get_req(addr, "/index.html").await;
    assert!(index_resp.contains("HTTP/1.1 200 OK"));
    assert!(index_resp.contains("text/html; charset=utf-8"));
    assert!(index_resp.contains("x-content-type-options: nosniff"));
    assert!(index_resp.contains("x-frame-options: SAMEORIGIN"));
    assert!(index_resp.contains("content-security-policy:"));
    assert!(index_resp.contains("TealDrive"));
    assert!(index_resp.contains(r#"meta name="status" content="ok""#));

    // 2. GET /style.css
    let css_resp = get_req(addr, "/style.css").await;
    assert!(css_resp.contains("HTTP/1.1 200 OK"));
    assert!(css_resp.contains("text/css; charset=utf-8"));
    assert!(css_resp.contains("--teal-500"));

    // 3. GET /app.js
    let js_resp = get_req(addr, "/app.js").await;
    assert!(js_resp.contains("HTTP/1.1 200 OK"));
    assert!(js_resp.contains("application/javascript; charset=utf-8"));
    assert!(js_resp.contains("TDRV"));

    // 4. GET /favicon.svg
    let svg_resp = get_req(addr, "/favicon.svg").await;
    assert!(svg_resp.contains("HTTP/1.1 200 OK"));
    assert!(svg_resp.contains("image/svg+xml"));

    // 5. GET /api/health
    let health_resp = get_req(addr, "/api/health").await;
    assert!(health_resp.contains("HTTP/1.1 200 OK"));
    assert!(health_resp.contains("ok"));

    // 6. SPA fallback for unknown path: GET /files/my-folder
    let spa_resp = get_req(addr, "/files/my-folder").await;
    assert!(spa_resp.contains("HTTP/1.1 200 OK"));
    assert!(spa_resp.contains("text/html; charset=utf-8"));
    assert!(spa_resp.contains("TealDrive"));

    // 7. Unknown API endpoint returns 404 JSON: GET /api/unknown
    let api_404_resp = get_req(addr, "/api/unknown").await;
    assert!(api_404_resp.contains("HTTP/1.1 404 Not Found"));
    assert!(api_404_resp.contains(r#"{"error":"not_found"}"#));
}

#[tokio::test]
async fn test_e2e_http_streaming_download_and_range_requests() {
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    use tealdrive::audit::sink::JsonlAuditSink;
    use tealdrive::auth::account::{MockAuthenticator, MockUser};
    use tealdrive::config::AccountPolicyConfig;
    use tealdrive::rate_limit::login::InMemoryLoginRateLimiter;
    use tealdrive::session::store::InMemorySessionStore;
    use tealdrive::ws::server::{build_router, WsServerState};

    let helper_binary = PathBuf::from(env!("CARGO_BIN_EXE_tealdrive-helper"));
    let client = HelperProcessClient::new(helper_binary, Duration::from_secs(5));

    let auth = MockAuthenticator::new(AccountPolicyConfig::default())
        .with_user(MockUser::regular("alice", "secret123"));

    let base_dir = temp_dir();
    let file_content = b"0123456789abcdefghijklmnopqrstuvwxyz";
    fs::write(base_dir.join("sample.txt"), file_content).expect("write sample");

    let config = AppConfig {
        roots: vec![AllowedRoot {
            root_id: RootId::new("home"),
            base_path: base_dir.clone(),
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
        }],
        ..AppConfig::default()
    };

    let session_store = InMemorySessionStore::new(config.session.clone());
    let audit_sink = JsonlAuditSink::open_default().expect("sink");
    let state = WsServerState::with_authenticator(
        config,
        session_store,
        client,
        audit_sink,
        Arc::new(auth),
        InMemoryLoginRateLimiter::new(5),
    );

    let router = build_router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind port");
    let addr = listener.local_addr().expect("local addr");

    tokio::spawn(async move {
        axum::serve(listener, router.into_make_service())
            .await
            .expect("server run");
    });

    // 1. Login to get cookie
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let body = r#"{"username":"alice","password":"secret123"}"#;
    let login_req = format!(
        "POST /api/login HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(login_req.as_bytes()).await.expect("write");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read");
    let login_resp = String::from_utf8_lossy(&buf);
    let cookie = login_resp
        .lines()
        .find(|l| l.to_lowercase().starts_with("set-cookie:"))
        .expect("cookie line")
        .split(';')
        .next()
        .unwrap()
        .split_once(':')
        .unwrap()
        .1
        .trim();

    // 2. Full Download: GET /api/download?root_id=home&path=sample.txt
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let dl_req = format!(
        "GET /api/download?root_id=home&path=sample.txt HTTP/1.1\r\nHost: localhost\r\nCookie: {}\r\nConnection: close\r\n\r\n",
        cookie
    );
    stream.write_all(dl_req.as_bytes()).await.expect("write");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read");
    let dl_resp = String::from_utf8_lossy(&buf);
    assert!(dl_resp.contains("HTTP/1.1 200 OK"));
    assert!(dl_resp.contains("content-disposition: attachment; filename=\"sample.txt\""));
    assert!(dl_resp.contains("accept-ranges: bytes"));
    assert!(dl_resp.contains("0123456789abcdefghijklmnopqrstuvwxyz"));

    // 3. Range Download: GET with Range: bytes=10-19 (10 bytes: "abcdefghij")
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let range_req = format!(
        "GET /api/download?root_id=home&path=sample.txt HTTP/1.1\r\nHost: localhost\r\nCookie: {}\r\nRange: bytes=10-19\r\nConnection: close\r\n\r\n",
        cookie
    );
    stream.write_all(range_req.as_bytes()).await.expect("write");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read");
    let range_resp = String::from_utf8_lossy(&buf);
    assert!(range_resp.contains("HTTP/1.1 206 Partial Content"));
    assert!(range_resp.contains("content-range: bytes 10-19/36"));
    assert!(range_resp.contains("content-length: 10"));
    assert!(range_resp.contains("abcdefghij"));

    // 4. Unauthenticated download rejected
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let no_auth_req = "GET /api/download?root_id=home&path=sample.txt HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    stream.write_all(no_auth_req.as_bytes()).await.expect("write");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read");
    let no_auth_resp = String::from_utf8_lossy(&buf);
    assert!(no_auth_resp.contains("HTTP/1.1 401 Unauthorized"));

    // 5. Path traversal download rejected
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let traversal_req = format!(
        "GET /api/download?root_id=home&path=../../etc/passwd HTTP/1.1\r\nHost: localhost\r\nCookie: {}\r\nConnection: close\r\n\r\n",
        cookie
    );
    stream.write_all(traversal_req.as_bytes()).await.expect("write");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read");
    let traversal_resp = String::from_utf8_lossy(&buf);
    assert!(traversal_resp.contains("HTTP/1.1 400 Bad Request"));

    let _ = fs::remove_dir_all(base_dir);
}
