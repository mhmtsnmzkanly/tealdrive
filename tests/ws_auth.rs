use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tealdrive::session::{InMemorySessionStore, SessionStore};
use tealdrive::ws::server::{build_router, WsServerState};
use tealdrive::{
    auth::account::{MockAuthenticator, MockUser},
    config::AppConfig,
    protocol::{MessageType, RequestId, TdrvFrame, TdrvHeader},
};
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};
use tower::ServiceExt;

#[tokio::test]
async fn ws_first_login_gates_files_and_binds_cookie_once() {
    exercise(false).await;
    exercise(true).await;
}
async fn exercise(web_mode: bool) {
    let dir = std::env::temp_dir().join(format!("tdrv-ws-auth-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let mut config = AppConfig::default();
    config.server.behind_tls_proxy = false;
    config.session.secure_cookies = false;
    config.security.allowed_origins = vec!["http://test.local".into()];
    if !web_mode {
        config.account_policy.allowed_users = vec!["alice".into()];
    }
    config.web_accounts.enabled = web_mode;
    config.web_accounts.server_uid = Some(995);
    let authenticator = MockAuthenticator::new(config.account_policy.clone())
        .with_user(MockUser::regular("alice", "secret-test"));
    let store = InMemorySessionStore::new(config.session.clone());
    let helper =
        tealdrive::helper::client::HelperProcessClient::new("/bin/false", Duration::from_secs(1));
    let audit = tealdrive::audit::sink::JsonlAuditSink::open(dir.join("audit.jsonl")).unwrap();
    let state = WsServerState::with_authenticator(
        config,
        store,
        helper,
        audit,
        Arc::new(GatewayFixture(authenticator)),
        tealdrive::rate_limit::login::InMemoryLoginRateLimiter::new(5),
    );
    let app = build_router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let mut request = format!("ws://{address}/ws/auth")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("origin", "http://evil.local".parse().unwrap());
    assert!(tokio_tungstenite::connect_async(request).await.is_err());
    let mut request = format!("ws://{address}/ws/auth")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("origin", "http://test.local".parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    let frame = receive(&mut ws).await;
    assert_eq!(frame.header.message_type, MessageType::ServerHello);
    let hello: tealdrive::protocol::payload::ServerHelloPayload =
        tealdrive::protocol::codec::decode_msgpack(&frame.payload).unwrap();
    // Even valid-looking file requests cannot reach helper before auth.
    send(
        &mut ws,
        MessageType::ListDirectory,
        serde_json::json!({"root_id":"home","relative_path":"","limit":10}),
    )
    .await;
    let denial = receive(&mut ws).await;
    let denial: serde_json::Value =
        tealdrive::protocol::codec::decode_msgpack(&denial.payload).unwrap();
    assert_eq!(denial["code"], "AUTH_REQUIRED");
    send(&mut ws,MessageType::ClientHello,serde_json::json!({"protocol_version":1,"supported_encodings":["MessagePack","RawBinary"],"supported_compressions":["None"],"max_frame_size":hello.max_frame_size,"max_control_payload_size":hello.max_control_payload_size,"max_chunk_size":hello.max_chunk_size,"feature_flags":hello.feature_flags})).await;
    assert_eq!(
        receive(&mut ws).await.header.message_type,
        MessageType::ProtocolReady
    );
    send(
        &mut ws,
        MessageType::AuthLogin,
        serde_json::json!({"username":"alice","password":"wrong"}),
    )
    .await;
    let denied = receive(&mut ws).await;
    let denied: serde_json::Value =
        tealdrive::protocol::codec::decode_msgpack(&denied.payload).unwrap();
    assert_eq!(denied["authenticated"], false);
    if web_mode {
        send(
            &mut ws,
            MessageType::AuthLogin,
            serde_json::json!({"action":"enrollmentComplete","password":"secret-test"}),
        )
        .await;
        let missing: serde_json::Value =
            tealdrive::protocol::codec::decode_msgpack(&receive(&mut ws).await.payload).unwrap();
        assert_eq!(missing["code"], "ENROLLMENT_REQUIRED");
        send(&mut ws, MessageType::AuthLogin, serde_json::json!({"action":"enrollmentBegin","username":"alice","password":"linux-fixture"})).await;
        let pending: serde_json::Value =
            tealdrive::protocol::codec::decode_msgpack(&receive(&mut ws).await.payload).unwrap();
        assert_eq!(pending["enrollment_required"], true);
        assert!(pending.get("enrollment").is_none());
        assert!(pending.get("id").is_none());
    }
    send(
        &mut ws,
        MessageType::AuthLogin,
        serde_json::json!({"username":"alice","password":"secret-test","action":if web_mode {"enrollmentComplete"} else {"login"}}),
    )
    .await;
    let result = receive(&mut ws).await;
    let result: serde_json::Value =
        tealdrive::protocol::codec::decode_msgpack(&result.payload).unwrap();
    assert_eq!(result["authenticated"], true);
    assert!(result.get("session_id").is_none());
    let token = result["bind_token"].as_str().unwrap();
    let bind = || {
        let mut req = Request::builder()
            .method("POST")
            .uri("/api/session/bind")
            .header("origin", "http://test.local")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::json!({"token":token}).to_string()))
            .unwrap();
        req.extensions_mut()
            .insert(axum::extract::ConnectInfo(address));
        req
    };
    let router = build_router(state.clone());
    let response = router.clone().oneshot(bind()).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .contains("HttpOnly"));
    assert_eq!(
        router.oneshot(bind()).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    state
        .session_store
        .lock()
        .await
        .revoke_all_for_user("alice");
    let _ = ws.close(None).await;
    task.abort();
    let log = std::fs::read_to_string(dir.join("audit.jsonl")).unwrap();
    assert!(!log.contains("secret-test"));
    assert!(!log.contains(token));
    assert!(!log.contains("fixture-proof-secret"));
    assert!(!log.contains("fixture-grant-secret"));
    std::fs::remove_dir_all(dir).unwrap();
}

// Isolate actual TCP protocol/cookie tests from the separately tested root SQLite/PAM gateway.
struct GatewayFixture(MockAuthenticator);
impl tealdrive::auth::Authenticator for GatewayFixture {
    fn authenticate(
        &self,
        request: &tealdrive::auth::AuthRequest,
    ) -> Result<tealdrive::auth::AuthSuccess, tealdrive::auth::AuthError> {
        tealdrive::auth::Authenticator::authenticate(&self.0, request)
    }
    fn account_command(
        &self,
        command: tealdrive::helper::account_gateway::AccountCommand,
    ) -> Result<
        tealdrive::helper::account_gateway::AccountReply,
        tealdrive::helper::account_gateway::GatewayFailure,
    > {
        use tealdrive::accounts::{
            grants::{CallerBinding, EnrollmentProof, SessionGrant},
            store::{Account, AccountKind, AccountStatus, Role},
        };
        use tealdrive::helper::account_gateway::{AccountCommand, AccountReply, GatewayFailure};
        let now = InMemorySessionStore::now_epoch_seconds();
        let account = Account {
            id: "account-alice".into(),
            username: "alice".into(),
            uid: 1000,
            gid: 1000,
            home: "/home/alice".into(),
            kind: AccountKind::Linux,
            role: Role::User,
            owner_admin_id: None,
            status: AccountStatus::Active,
            login_enabled: true,
            generation: 1,
            password_enrolled: true,
            quota_bytes: None,
            undo_hours: None,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            deny_enrollment: false,
        };
        let grant = SessionGrant {
            nonce: "fixture-grant-secret".into(),
            account_id: account.id.clone(),
            generation: 1,
            caller: CallerBinding {
                uid: 995,
                pid: 1,
                start_time: 1,
                boot_id: "00000000-0000-4000-8000-000000000001".into(),
            },
            expires_at: now + 3600,
        };
        match command {
            AccountCommand::BeginEnrollment(request) if request.password == "linux-fixture" => {
                Ok(AccountReply::Enrollment(EnrollmentProof {
                    id: "fixture-proof-secret".into(),
                    expires_at: now + 300,
                }))
            }
            AccountCommand::CompleteEnrollment { proof, password }
                if proof.id == "fixture-proof-secret" && &*password == "secret-test" =>
            {
                Ok(AccountReply::Authenticated { account, grant })
            }
            AccountCommand::Login(request) if request.password == "secret-test" => {
                Ok(AccountReply::Authenticated { account, grant })
            }
            AccountCommand::Validate(_) => Ok(AccountReply::Validated(account)),
            AccountCommand::Logout(_) => Ok(AccountReply::LoggedOut),
            _ => Err(GatewayFailure::InvalidCredentials),
        }
    }
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
async fn receive(ws: &mut Socket) -> TdrvFrame {
    let message = tokio::time::timeout(Duration::from_secs(3), ws.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let Message::Binary(bytes) = message else {
        panic!("Expected binary response")
    };
    TdrvFrame::decode(&bytes).unwrap()
}
async fn send(ws: &mut Socket, kind: MessageType, payload: serde_json::Value) {
    let frame = TdrvFrame::new(
        TdrvHeader::new(kind, RequestId::new(), 0),
        tealdrive::protocol::codec::encode_msgpack(&payload).unwrap(),
    )
    .unwrap();
    ws.send(Message::Binary(frame.encode().unwrap()))
        .await
        .unwrap();
}
