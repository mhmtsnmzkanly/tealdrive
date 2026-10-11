use axum::extract::State;
use axum::http::header::{HeaderMap, HeaderValue, SET_COOKIE, USER_AGENT};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::audit::event::{AuditEvent, AuditReason};
use crate::auth::account::{AuthError, AuthRequest};
use crate::protocol::frame::RequestId;
use crate::rate_limit::login::{LoginRateLimitDecision, LoginRateLimitKey};
use crate::session::cookie::SessionCookieConfig;
use crate::session::csrf::CsrfToken;
use crate::session::store::{
    InMemorySessionStore, SessionCreateInput, SessionLookupResult, SessionStore,
};
use crate::ws::server::{extract_client_ip, extract_session_id, WsHelperBackend, WsServerState};

#[derive(Debug, Clone, Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootSummary {
    pub root_id: String,
    pub read_only: bool,
    pub is_web_root: bool,
    pub uploads_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginResponse {
    pub username: String,
    pub uid: u32,
    pub gid: u32,
    pub csrf_token: String,
    pub roots: Vec<RootSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionResponse {
    pub authenticated: bool,
    pub username: Option<String>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub csrf_token: Option<String>,
    pub roots: Vec<RootSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogoutResponse {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthErrorResponse {
    pub error: String,
    pub code: String,
}

pub async fn login_handler<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
    Json(payload): Json<LoginPayload>,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    let client_ip = extract_client_ip(&headers);
    let user_agent = headers
        .get(USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(ToOwned::to_owned);

    let rate_key = LoginRateLimitKey {
        username: payload.username.clone(),
        ip: client_ip.clone(),
    };

    {
        let limiter = state.rate_limiter.lock().await;
        if limiter.check(&rate_key) == LoginRateLimitDecision::Limited {
            let request_id = RequestId::new();
            let _ = state.audit_sink.emit(&AuditEvent::login_failure(
                request_id,
                &payload.username,
                AuditReason::RateLimited,
            ));
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(AuthErrorResponse {
                    error: "Too many failed login attempts. Please try again later.".to_owned(),
                    code: "RATE_LIMITED".to_owned(),
                }),
            )
                .into_response();
        }
    }

    let auth_req = AuthRequest {
        username: payload.username.clone(),
        password: payload.password,
        ip: client_ip.clone(),
    };

    let auth_success = match state.authenticator.authenticate(&auth_req) {
        Ok(success) => success,
        Err(err) => {
            let mut limiter = state.rate_limiter.lock().await;
            let _ = limiter.record_failure(rate_key);

            let reason = match err {
                AuthError::InvalidCredentials => AuditReason::InvalidCredentials,
                AuthError::AccountLocked => AuditReason::AccountLocked,
                AuthError::AccountExpired => AuditReason::AccountExpired,
                AuthError::DisabledShell => AuditReason::DisabledShell,
                AuthError::RootLoginDisabled => AuditReason::RootLoginDisabled,
                AuthError::SystemAccountDisabled => AuditReason::SystemAccountDisabled,
                _ => AuditReason::InvalidCredentials,
            };
            let request_id = RequestId::new();
            let _ = state.audit_sink.emit(&AuditEvent::login_failure(
                request_id,
                &payload.username,
                reason,
            ));
            return (
                StatusCode::UNAUTHORIZED,
                Json(AuthErrorResponse {
                    error: "Invalid username or password.".to_owned(),
                    code: "INVALID_CREDENTIALS".to_owned(),
                }),
            )
                .into_response();
        }
    };

    {
        let mut limiter = state.rate_limiter.lock().await;
        limiter.record_success(&rate_key);
    }

    let now = InMemorySessionStore::now_epoch_seconds();
    let session_input = SessionCreateInput {
        username: auth_success.username.clone(),
        uid: auth_success.uid,
        gid: auth_success.primary_gid,
        user_agent,
        ip: client_ip,
    };

    let session = {
        let mut store = state.session_store.lock().await;
        store.create_session(session_input, now)
    };

    let request_id = RequestId::new();
    let _ = state.audit_sink.emit(&AuditEvent::login_success(
        request_id,
        &auth_success.username,
        auth_success.uid,
        auth_success.primary_gid,
    ));

    let cookie_config = SessionCookieConfig::from_session_config(&state.config.session);
    let cookie_val = cookie_config.build_set_cookie_value(&session.session_id);
    let csrf_token = CsrfToken::generate();

    let roots = state
        .config
        .allowed_roots()
        .iter()
        .map(|r| RootSummary {
            root_id: r.root_id.0.clone(),
            read_only: r.read_only,
            is_web_root: r.is_web_root,
            uploads_allowed: r.uploads_allowed,
        })
        .collect();

    let response_body = LoginResponse {
        username: auth_success.username,
        uid: auth_success.uid,
        gid: auth_success.primary_gid,
        csrf_token: csrf_token.0,
        roots,
    };

    let mut response = (StatusCode::OK, Json(response_body)).into_response();
    if let Ok(val) = HeaderValue::from_str(&cookie_val) {
        response.headers_mut().insert(SET_COOKIE, val);
    }
    response
}

pub async fn logout_handler<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    if let Ok(session_id) = extract_session_id(&headers, &state.config.session.cookie_name) {
        let mut store = state.session_store.lock().await;
        let now = InMemorySessionStore::now_epoch_seconds();
        if let SessionLookupResult::Found(session) = store.get_session(&session_id, now) {
            store.revoke_session(&session_id);
            let request_id = RequestId::new();
            let _ = state.audit_sink.emit(&AuditEvent::logout(
                request_id,
                &session.username,
                session.uid,
                session.gid,
            ));
        }
    }

    let cookie_config = SessionCookieConfig::from_session_config(&state.config.session);
    let clear_val = cookie_config.build_clear_cookie_value();

    let mut response = (
        StatusCode::OK,
        Json(LogoutResponse {
            message: "Logged out successfully.".to_owned(),
        }),
    )
        .into_response();

    if let Ok(val) = HeaderValue::from_str(&clear_val) {
        response.headers_mut().insert(SET_COOKIE, val);
    }
    response
}

pub async fn session_handler<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    let session_id = match extract_session_id(&headers, &state.config.session.cookie_name) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(SessionResponse {
                    authenticated: false,
                    username: None,
                    uid: None,
                    gid: None,
                    csrf_token: None,
                    roots: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    let now = InMemorySessionStore::now_epoch_seconds();
    let lookup = {
        let mut store = state.session_store.lock().await;
        store.touch_session(&session_id, now)
    };

    match lookup {
        SessionLookupResult::Found(session) => {
            let csrf_token = CsrfToken::generate();
            let roots = state
                .config
                .allowed_roots()
                .iter()
                .map(|r| RootSummary {
                    root_id: r.root_id.0.clone(),
                    read_only: r.read_only,
                    is_web_root: r.is_web_root,
                    uploads_allowed: r.uploads_allowed,
                })
                .collect();

            (
                StatusCode::OK,
                Json(SessionResponse {
                    authenticated: true,
                    username: Some(session.username),
                    uid: Some(session.uid),
                    gid: Some(session.gid),
                    csrf_token: Some(csrf_token.0),
                    roots,
                }),
            )
                .into_response()
        }
        _ => {
            let cookie_config = SessionCookieConfig::from_session_config(&state.config.session);
            let clear_val = cookie_config.build_clear_cookie_value();

            let mut response = (
                StatusCode::UNAUTHORIZED,
                Json(SessionResponse {
                    authenticated: false,
                    username: None,
                    uid: None,
                    gid: None,
                    csrf_token: None,
                    roots: Vec::new(),
                }),
            )
                .into_response();

            if let Ok(val) = HeaderValue::from_str(&clear_val) {
                response.headers_mut().insert(SET_COOKIE, val);
            }
            response
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use axum::body::to_bytes;
    use axum::http::header::COOKIE;
    use crate::audit::sink::JsonlAuditSink;
    use crate::auth::account::{MockAuthenticator, MockUser};
    use crate::config::{AccountPolicyConfig, AllowedRoot, AppConfig, RootId};
    use crate::download::transfer::DownloadHelper;
    use crate::fs::archive::{CompressTarGzHelper, CompressZipHelper, ExtractArchiveHelper};
    use crate::fs::chmod::ChmodFileHelper;
    use crate::fs::create_folder::CreateFolderHelper;
    use crate::fs::listing::DirectoryListingHelper;
    use crate::fs::metadata::MetadataHelper;
    use crate::fs::move_copy::{CopyHelper, MoveHelper};
    use crate::fs::rename::RenameHelper;
    use crate::fs::text::{SaveTextFileHelper, TextPreviewHelper};
    use crate::fs::trash::{DeletePermanentlyHelper, MoveToTrashHelper, RestoreFromTrashHelper};
    use crate::helper::types::{HelperRequest, HelperResponse};
    use crate::rate_limit::login::InMemoryLoginRateLimiter;
    use crate::session::store::InMemorySessionStore;
    use crate::upload::transfer::UploadHelper;

    #[derive(Clone, Debug)]
    struct DummyHelper;

    macro_rules! impl_dummy_helper {
        ($trait_name:ident) => {
            impl $trait_name for DummyHelper {
                fn execute_helper(&self, _request: &HelperRequest) -> Result<HelperResponse, crate::errors::TealDriveError> {
                    Err(crate::errors::TealDriveError::HelperCommandNotImplemented)
                }
            }
        };
    }

    impl_dummy_helper!(DirectoryListingHelper);
    impl_dummy_helper!(MetadataHelper);
    impl_dummy_helper!(DownloadHelper);
    impl_dummy_helper!(TextPreviewHelper);
    impl_dummy_helper!(SaveTextFileHelper);
    impl_dummy_helper!(CreateFolderHelper);
    impl_dummy_helper!(RenameHelper);
    impl_dummy_helper!(MoveHelper);
    impl_dummy_helper!(CopyHelper);
    impl_dummy_helper!(MoveToTrashHelper);
    impl_dummy_helper!(RestoreFromTrashHelper);
    impl_dummy_helper!(DeletePermanentlyHelper);
    impl_dummy_helper!(ChmodFileHelper);
    impl_dummy_helper!(UploadHelper);
    impl_dummy_helper!(CompressZipHelper);
    impl_dummy_helper!(CompressTarGzHelper);
    impl_dummy_helper!(ExtractArchiveHelper);

    fn test_state(max_failures: u32) -> WsServerState<DummyHelper> {
        let auth = MockAuthenticator::new(AccountPolicyConfig::default())
            .with_user(MockUser::regular("alice", "secret123"));
        let config = AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: std::path::PathBuf::from("/home"),
                read_only: false,
                uploads_allowed: true,
                hidden_files_allowed: false,
                is_web_root: false,
            }],
            ..AppConfig::default()
        };
        let session_store = InMemorySessionStore::new(config.session.clone());
        let audit_sink = JsonlAuditSink::open_default().expect("sink");
        WsServerState::with_authenticator(
            config,
            session_store,
            DummyHelper,
            audit_sink,
            Arc::new(auth),
            InMemoryLoginRateLimiter::new(max_failures),
        )
    }

    #[tokio::test]
    async fn test_login_success_sets_cookie_and_returns_session_info() {
        let state = test_state(5);
        let payload = LoginPayload {
            username: "alice".to_owned(),
            password: "secret123".to_owned(),
        };

        let response = login_handler(State(state), HeaderMap::new(), Json(payload)).await;
        assert_eq!(response.status(), StatusCode::OK);

        let set_cookie = response
            .headers()
            .get(SET_COOKIE)
            .expect("set-cookie")
            .to_str()
            .expect("utf8");
        assert!(set_cookie.starts_with("tealdrive_session="));
        assert!(set_cookie.contains("HttpOnly"));

        let body_bytes = to_bytes(response.into_body(), 4096).await.expect("bytes");
        let login_resp: LoginResponse = serde_json::from_slice(&body_bytes).expect("json");
        assert_eq!(login_resp.username, "alice");
        assert_eq!(login_resp.uid, 1000);
        assert_eq!(login_resp.gid, 1000);
        assert!(!login_resp.csrf_token.is_empty());
        assert_eq!(login_resp.roots.len(), 1);
        assert_eq!(login_resp.roots[0].root_id, "home");
    }

    #[tokio::test]
    async fn test_login_failure_invalid_credentials() {
        let state = test_state(5);
        let payload = LoginPayload {
            username: "alice".to_owned(),
            password: "wrongpassword".to_owned(),
        };

        let response = login_handler(State(state), HeaderMap::new(), Json(payload)).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body_bytes = to_bytes(response.into_body(), 4096).await.expect("bytes");
        let err_resp: AuthErrorResponse = serde_json::from_slice(&body_bytes).expect("json");
        assert_eq!(err_resp.code, "INVALID_CREDENTIALS");
    }

    #[tokio::test]
    async fn test_login_rate_limiting_after_repeated_failures() {
        let state = test_state(2);
        let payload = LoginPayload {
            username: "alice".to_owned(),
            password: "bad".to_owned(),
        };

        // Attempt 1 -> 401
        let resp1 = login_handler(State(state.clone()), HeaderMap::new(), Json(payload.clone())).await;
        assert_eq!(resp1.status(), StatusCode::UNAUTHORIZED);

        // Attempt 2 -> 401
        let resp2 = login_handler(State(state.clone()), HeaderMap::new(), Json(payload.clone())).await;
        assert_eq!(resp2.status(), StatusCode::UNAUTHORIZED);

        // Attempt 3 -> 429
        let resp3 = login_handler(State(state.clone()), HeaderMap::new(), Json(payload.clone())).await;
        assert_eq!(resp3.status(), StatusCode::TOO_MANY_REQUESTS);

        let body_bytes = to_bytes(resp3.into_body(), 4096).await.expect("bytes");
        let err_resp: AuthErrorResponse = serde_json::from_slice(&body_bytes).expect("json");
        assert_eq!(err_resp.code, "RATE_LIMITED");
    }

    #[tokio::test]
    async fn test_session_and_logout_lifecycle() {
        let state = test_state(5);
        let payload = LoginPayload {
            username: "alice".to_owned(),
            password: "secret123".to_owned(),
        };

        // 1. Login
        let login_res = login_handler(State(state.clone()), HeaderMap::new(), Json(payload)).await;
        assert_eq!(login_res.status(), StatusCode::OK);
        let cookie_raw = login_res.headers().get(SET_COOKIE).unwrap().to_str().unwrap().to_owned();

        // Extract session cookie key-value
        let cookie_val = cookie_raw.split(';').next().unwrap().to_owned();
        let mut auth_headers = HeaderMap::new();
        auth_headers.insert(COOKIE, HeaderValue::from_str(&cookie_val).unwrap());

        // 2. Check session with cookie
        let session_res = session_handler(State(state.clone()), auth_headers.clone()).await;
        assert_eq!(session_res.status(), StatusCode::OK);
        let session_bytes = to_bytes(session_res.into_body(), 4096).await.unwrap();
        let session_data: SessionResponse = serde_json::from_slice(&session_bytes).unwrap();
        assert!(session_data.authenticated);
        assert_eq!(session_data.username.as_deref(), Some("alice"));

        // 3. Logout
        let logout_res = logout_handler(State(state.clone()), auth_headers.clone()).await;
        assert_eq!(logout_res.status(), StatusCode::OK);
        let logout_cookie = logout_res.headers().get(SET_COOKIE).unwrap().to_str().unwrap();
        assert!(logout_cookie.contains("Max-Age=0"));

        // 4. Check session again after logout
        let after_res = session_handler(State(state.clone()), auth_headers).await;
        assert_eq!(after_res.status(), StatusCode::UNAUTHORIZED);
        let after_bytes = to_bytes(after_res.into_body(), 4096).await.unwrap();
        let after_data: SessionResponse = serde_json::from_slice(&after_bytes).unwrap();
        assert!(!after_data.authenticated);
    }

    #[tokio::test]
    async fn test_session_unauthenticated_without_cookie() {
        let state = test_state(5);
        let response = session_handler(State(state), HeaderMap::new()).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body_bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        let data: SessionResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert!(!data.authenticated);
    }
}
