//! Opt-in real listener acceptance; no production paths or services are modified.
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command, time::Duration};
use tealdrive::{
    config::AppConfig,
    helper::client::HelperProcessClient,
    session::InMemorySessionStore,
    ws::server::{run_server_with_shutdown, WsServerState},
};

fn generate(directory: &Path, label: &str) {
    assert!(Command::new("/usr/bin/openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "1",
            "-subj",
            "/CN=localhost",
            "-addext",
            "subjectAltName=DNS:localhost,IP:127.0.0.1",
            "-keyout"
        ])
        .arg(directory.join(format!("{label}.key")))
        .arg("-out")
        .arg(directory.join(format!("{label}.crt")))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success());
    fs::set_permissions(
        directory.join(format!("{label}.key")),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
}
async fn peer(port: u16, trust: &Path) -> bool {
    let trust = trust.to_owned();
    tokio::task::spawn_blocking(move || {
        Command::new("python3").arg("-c").arg("import ssl,socket,sys; c=ssl.create_default_context(cafile=sys.argv[2]); s=c.wrap_socket(socket.create_connection(('127.0.0.1',int(sys.argv[1])),timeout=3),server_hostname='localhost'); s.close()")
            .arg(port.to_string()).arg(trust).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().unwrap().success()
    }).await.unwrap()
}
#[tokio::test]
#[ignore = "requires explicit Root session, openssl/python, and two certificate refresh intervals"]
async fn real_tls_refresh_preserves_old_pair_on_failure_and_rotates_without_restart() {
    assert_eq!(unsafe { libc::getuid() }, 0);
    let directory =
        Path::new("/root").join(format!(".tealdrive-tls-refresh-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&directory).unwrap();
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(directory.clone());
    generate(&directory, "old");
    generate(&directory, "new");
    let cert = directory.join("active.crt");
    let key = directory.join("active.key");
    fs::copy(directory.join("old.crt"), &cert).unwrap();
    fs::copy(directory.join("old.key"), &key).unwrap();
    let mut config = AppConfig::default();
    config.server.require_tls = Some(true);
    config.server.behind_tls_proxy = false;
    config.server.tls_cert_path = Some(cert.clone());
    config.server.tls_key_path = Some(key.clone());
    let state = WsServerState::new(
        config.clone(),
        InMemorySessionStore::new(config.session),
        HelperProcessClient::new("/bin/false", Duration::from_secs(1)),
        tealdrive::audit::sink::JsonlAuditSink::open(directory.join("audit.jsonl")).unwrap(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (send, receive) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(run_server_with_shutdown(
        listener,
        state,
        async move {
            let _ = receive.await;
        },
        Duration::from_secs(1),
    ));
    // Initial TLS listener setup is asynchronous.
    for _ in 0..30 {
        if peer(port, &directory.join("old.crt")).await {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(peer(port, &directory.join("old.crt")).await);
    fs::write(&key, b"invalid private key").unwrap();
    tokio::time::sleep(Duration::from_secs(62)).await;
    assert!(
        peer(port, &directory.join("old.crt")).await,
        "invalid renewal must retain the last validated pair"
    );
    fs::copy(directory.join("new.crt"), &cert).unwrap();
    fs::copy(directory.join("new.key"), &key).unwrap();
    tokio::time::sleep(Duration::from_secs(62)).await;
    assert!(
        peer(port, &directory.join("new.crt")).await,
        "new pair must be served without restart"
    );
    assert!(
        !peer(port, &directory.join("old.crt")).await,
        "old trust must not accept the new self-signed identity"
    );
    send.send(()).unwrap();
    task.await.unwrap().unwrap();
}
