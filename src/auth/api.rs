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
use crate::session::store::{
    InMemorySessionStore, SessionCreateInput, SessionLookupResult, SessionStore,
};
use crate::ws::server::{extract_client_ip, extract_session_id, WsHelperBackend, WsServerState};

#[derive(Clone, Deserialize, zeroize::ZeroizeOnDrop)]
pub struct LoginPayload {
    #[zeroize(skip)]
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootSummary {
    pub root_id: String,
    pub read_only: bool,
    pub is_web_root: bool,
    pub uploads_allowed: bool,
    #[serde(default)]
    pub hidden_files_allowed: bool,
    #[serde(default)]
    pub quota_bytes: Option<u64>,
    #[serde(default)]
    pub used_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginResponse {
    #[serde(default)]
    pub password_change_required: bool,
    pub username: String,
    pub uid: u32,
    pub gid: u32,
    pub csrf_token: String,
    pub roots: Vec<RootSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionResponse {
    #[serde(default)]
    pub password_change_required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_generation: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_kind: Option<crate::accounts::store::AccountKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<crate::accounts::store::Role>,
    pub authenticated: bool,
    pub is_admin: bool,
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

/// Root-backed sessions are revalidated before cookie binding, resume or management.
pub async fn validated_session_account<H>(
    state: &WsServerState<H>,
    session: &crate::session::store::Session,
) -> Result<Option<crate::accounts::store::Account>, crate::helper::account_gateway::GatewayFailure>
{
    use crate::helper::account_gateway::{AccountCommand, AccountReply, GatewayFailure};
    if !session.allowed_by(&state.config()) {
        return Err(GatewayFailure::Forbidden);
    }
    let Some(grant) = session.grant.as_deref().cloned() else {
        return Ok(None);
    };
    let permit = crate::session::lease::validation_slots()
        .acquire_owned()
        .await
        .map_err(|_| GatewayFailure::Unavailable)?;
    let activity = state.shutdown.enter().ok_or(GatewayFailure::Unavailable)?;
    let authenticator = state.authenticator.clone();
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let _activity = activity;
        authenticator.account_command(AccountCommand::Validate(grant))
    })
    .await
    .map_err(|_| GatewayFailure::Unavailable)??
    {
        AccountReply::Validated(account)
            if account.username == session.username
                && account.uid == session.uid
                && account.gid == session.gid =>
        {
            Ok(Some(account))
        }
        _ => Err(GatewayFailure::Forbidden),
    }
}

pub async fn login_handler<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
    Json(payload): Json<LoginPayload>,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    login_command_handler(State(state), headers, Json(payload), None).await
}

pub async fn login_command_handler<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
    Json(mut payload): Json<LoginPayload>,
    command: Option<crate::helper::account_gateway::AccountCommand>,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    let client_ip = extract_client_ip(&headers);
    let user_agent = headers
        .get(USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(ToOwned::to_owned);

    let cfg = state.config.load();
    if command.is_some() && !cfg.web_accounts.enabled {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(AuthErrorResponse {
                error: "Web enrollment is disabled.".into(),
                code: "FEATURE_DISABLED".into(),
            }),
        )
            .into_response();
    }
    let max_failures = cfg.rate_limit.max_login_failures;
    let lockout_secs = cfg.rate_limit.lockout_duration_seconds;
    let now = InMemorySessionStore::now_epoch_seconds();

    let rate_key = LoginRateLimitKey {
        username: payload.username.clone(),
        ip: client_ip.clone(),
    };

    {
        let limiter = state.rate_limiter.lock().await;
        if limiter.check_at(&rate_key, max_failures, lockout_secs, now)
            == LoginRateLimitDecision::Limited
        {
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

    if (!cfg.web_accounts.enabled && !cfg.account_policy.allows_login(&payload.username))
        || cfg.web_accounts.denied_users.contains(&payload.username)
    {
        state
            .rate_limiter
            .lock()
            .await
            .record_failure_at(rate_key, lockout_secs, now);
        let _ = state.audit_sink.emit(&AuditEvent::login_failure(
            RequestId::new(),
            &payload.username,
            AuditReason::AccountNotAllowed,
        ));
        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthErrorResponse {
                error: "Invalid username or password.".into(),
                code: "INVALID_CREDENTIALS".into(),
            }),
        )
            .into_response();
    }
    let auth_req = AuthRequest {
        username: payload.username.clone(),
        password: std::mem::take(&mut payload.password),
        ip: client_ip.clone(),
    };

    let permit = match state.operation_slots.clone().acquire_owned().await {
        Ok(p) => p,
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let Some(activity) = state.shutdown.enter() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let cancellation = state.shutdown.cancellation();
    let authenticator = state.authenticator.clone();
    let web_mode = cfg.web_accounts.enabled;
    let auth_result = tokio::task::spawn_blocking(move || {
        let _activity = activity;
        let _permit = permit;
        crate::session::lease::with_cancellation(cancellation, || {
            if web_mode {
                use crate::helper::account_gateway::{
                    AccountCommand, AccountReply, GatewayFailure,
                };
                let mut command =
                    command.unwrap_or_else(|| AccountCommand::Login(auth_req.clone()));
                if let AccountCommand::BeginEnrollment(request) = &mut command {
                    request.ip = auth_req.ip.clone();
                }
                match authenticator.account_command(command) {
                    Ok(AccountReply::Authenticated { account, grant }) => Ok(Ok((
                        crate::auth::AuthSuccess {
                            username: account.username,
                            uid: account.uid,
                            primary_gid: account.gid,
                            home_dir: Some(account.home),
                            shell: None,
                            groups: vec![],
                        },
                        Some(grant),
                        account.password_change_required,
                    ))),
                    Ok(AccountReply::Enrollment(proof)) => Ok(Err(proof)),
                    Err(GatewayFailure::RateLimited) => Err(AuthError::RateLimited),
                    Err(GatewayFailure::Unavailable | GatewayFailure::Disabled) => {
                        Err(AuthError::AuthBackendUnavailable)
                    }
                    _ => Err(AuthError::InvalidCredentials),
                }
            } else {
                authenticator
                    .authenticate(&auth_req)
                    .map(|success| Ok((success, None, false)))
            }
        })
    })
    .await;
    let (auth_success, auth_grant, password_change_required) =
        match auth_result.unwrap_or(Err(AuthError::AuthBackendUnavailable)) {
            Ok(Ok(success)) => success,
            Ok(Err(proof)) => {
                state.rate_limiter.lock().await.record_success(&rate_key);
                return (
                    StatusCode::ACCEPTED,
                    Json(serde_json::json!({ "enrollment": proof })),
                )
                    .into_response();
            }
            Err(err) => {
                if err == AuthError::RateLimited {
                    let _ = state.audit_sink.emit(&AuditEvent::login_failure(
                        RequestId::new(),
                        &payload.username,
                        AuditReason::RateLimited,
                    ));
                    return (
                        StatusCode::TOO_MANY_REQUESTS,
                        Json(AuthErrorResponse {
                            error: "Too many failed login attempts. Please try again later.".into(),
                            code: "RATE_LIMITED".into(),
                        }),
                    )
                        .into_response();
                }
                if err == AuthError::AuthBackendUnavailable {
                    return (
                        StatusCode::SERVICE_UNAVAILABLE,
                        Json(AuthErrorResponse {
                            error: "Authentication service unavailable.".into(),
                            code: "SERVICE_UNAVAILABLE".into(),
                        }),
                    )
                        .into_response();
                }
                let mut limiter = state.rate_limiter.lock().await;
                let _ = limiter.record_failure_at(rate_key, lockout_secs, now);

                let reason = match err {
                    AuthError::InvalidCredentials => AuditReason::InvalidCredentials,
                    AuthError::AccountLocked => AuditReason::AccountLocked,
                    AuthError::AccountExpired => AuditReason::AccountExpired,
                    AuthError::DisabledShell => AuditReason::DisabledShell,
                    AuthError::RootLoginDisabled => AuditReason::RootLoginDisabled,
                    AuthError::SystemAccountDisabled => AuditReason::SystemAccountDisabled,
                    AuthError::AccountNotAllowed => AuditReason::AccountNotAllowed,
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

    let candidate = crate::auth::account::MockUser {
        username: auth_success.username.clone(),
        password: String::new(),
        uid: auth_success.uid,
        primary_gid: auth_success.primary_gid,
        home_dir: auth_success.home_dir.clone(),
        shell: auth_success.shell.clone(),
        groups: auth_success.groups.clone(),
        status: crate::auth::account::AccountStatus::Active,
    };
    let current = state.config();
    if current.web_accounts.enabled != web_mode
        || current
            .web_accounts
            .denied_users
            .contains(&candidate.username)
        || (!web_mode
            && !matches!(
                crate::auth::account::evaluate_account_policy(&candidate, &current.account_policy),
                crate::auth::account::AccountPolicyResult::Allowed
            ))
    {
        state
            .rate_limiter
            .lock()
            .await
            .record_failure_at(rate_key, lockout_secs, now);
        let _ = state.audit_sink.emit(&AuditEvent::login_failure(
            RequestId::new(),
            &payload.username,
            AuditReason::AccountNotAllowed,
        ));
        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthErrorResponse {
                error: "Invalid username or password.".into(),
                code: "INVALID_CREDENTIALS".into(),
            }),
        )
            .into_response();
    }
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

    if let Some(grant) = auth_grant {
        if !state
            .session_store
            .lock()
            .await
            .attach_grant(&session.session_id, grant)
        {
            state
                .session_store
                .lock()
                .await
                .revoke_session(&session.session_id);
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
    }
    let request_id = RequestId::new();
    let mut event = AuditEvent::login_success(
        request_id,
        &auth_success.username,
        auth_success.uid,
        auth_success.primary_gid,
    );
    event.session_id_hash = crate::audit::context::session_hash(session.session_id);
    let _ = state.audit_sink.emit(&event);

    let cfg = state.config.load();
    let cookie_config = SessionCookieConfig::from_session_config(&cfg.session);
    let cookie_val = cookie_config.build_set_cookie_value(&session.session_id);
    let csrf_token = session.csrf_token.clone();

    let mut login_config = cfg.as_ref().clone();
    if let Some(template) = &cfg.home_template {
        let Some(home) = auth_success.home_dir.as_ref() else {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        };
        let mut root = template.clone();
        root.base_path = home.clone();
        login_config.roots = vec![root];
    }
    let roots = login_config
        .allowed_roots()
        .iter()
        .filter(|root| {
            !password_change_required
                && (!web_mode
                    || auth_success
                        .home_dir
                        .as_ref()
                        .is_some_and(|home| &root.base_path == home))
        })
        .map(|r| RootSummary {
            root_id: r.root_id.0.clone(),
            read_only: r.read_only,
            is_web_root: r.is_web_root,
            uploads_allowed: r.uploads_allowed,
            hidden_files_allowed: r.hidden_files_allowed,
            quota_bytes: r.quota_bytes,
            used_bytes: None,
        })
        .collect();

    let response_body = LoginResponse {
        password_change_required,
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
    let cfg = state.config.load();
    if let Ok(session_id) = extract_session_id(&headers, &cfg.session.cookie_name) {
        let mut store = state.session_store.lock().await;
        let now = InMemorySessionStore::now_epoch_seconds();
        if let SessionLookupResult::Found(session) = store.get_session(&session_id, now) {
            if !crate::session::csrf::validate_header(&session, &headers) {
                return StatusCode::FORBIDDEN.into_response();
            }
            if let Some(grant) = session.grant.as_deref().cloned() {
                drop(store);
                let authenticator = state.authenticator.clone();
                let result = tokio::task::spawn_blocking(move || {
                    authenticator.account_command(
                        crate::helper::account_gateway::AccountCommand::Logout(grant),
                    )
                })
                .await;
                if matches!(
                    result,
                    Err(_)
                        | Ok(Err(
                            crate::helper::account_gateway::GatewayFailure::Unavailable
                                | crate::helper::account_gateway::GatewayFailure::Disabled
                        ))
                ) {
                    return StatusCode::SERVICE_UNAVAILABLE.into_response();
                }
                store = state.session_store.lock().await;
            }
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

    let cookie_config = SessionCookieConfig::from_session_config(&cfg.session);
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
    let cfg = state.config.load();
    let session_id = match extract_session_id(&headers, &cfg.session.cookie_name) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(SessionResponse {
                    account_id: None,
                    account_generation: None,
                    account_kind: None,
                    role: None,
                    password_change_required: false,
                    authenticated: false,
                    is_admin: false,
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
            let account = match validated_session_account(&state, &session).await {
                Ok(account) => account,
                Err(crate::helper::account_gateway::GatewayFailure::Unavailable) => {
                    return StatusCode::SERVICE_UNAVAILABLE.into_response()
                }
                Err(_) => {
                    state.session_store.lock().await.revoke_session(&session_id);
                    return StatusCode::UNAUTHORIZED.into_response();
                }
            };
            let cfg = match account.as_ref() {
                Some(account) => {
                    match crate::policy::home::resolve(&cfg, account, cfg.home_template.as_ref()) {
                        Ok(config) => config,
                        Err(_) => return StatusCode::FORBIDDEN.into_response(),
                    }
                }
                None if cfg.home_template.is_some() => {
                    return StatusCode::FORBIDDEN.into_response();
                }
                None => cfg.as_ref().clone(),
            };
            let is_admin = cfg.account_policy.is_admin(&session.username)
                && account.as_ref().is_none_or(|account| {
                    account.role == crate::accounts::store::Role::Admin
                        && account.global_admin
                        && !account.password_change_required
                });
            let csrf_token = session.csrf_token.clone();
            let roots = cfg
                .allowed_roots()
                .iter()
                .filter(|root| {
                    account.as_ref().is_none_or(|account| {
                        !account.password_change_required && root.base_path == account.home
                    })
                })
                .map(|r| RootSummary {
                    root_id: r.root_id.0.clone(),
                    read_only: r.read_only,
                    is_web_root: r.is_web_root,
                    uploads_allowed: r.uploads_allowed,
                    hidden_files_allowed: r.hidden_files_allowed,
                    quota_bytes: if account.as_ref().is_some_and(|account| {
                        account.kind == crate::accounts::store::AccountKind::Virtual
                    }) {
                        let quota = account
                            .as_ref()
                            .and_then(|account| account.quota_bytes)
                            .unwrap_or(0);
                        Some(r.quota_bytes.map_or(quota, |ceiling| ceiling.min(quota)))
                    } else {
                        r.quota_bytes
                    },
                    used_bytes: None,
                })
                .collect();

            (
                StatusCode::OK,
                Json(SessionResponse {
                    account_id: account.as_ref().map(|account| account.id.clone()),
                    account_generation: account.as_ref().map(|account| account.generation),
                    account_kind: account.as_ref().map(|account| account.kind),
                    role: account.as_ref().map(|account| account.role),
                    password_change_required: account
                        .as_ref()
                        .is_some_and(|a| a.password_change_required),
                    authenticated: true,
                    is_admin,
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
            let cookie_config = SessionCookieConfig::from_session_config(&cfg.session);
            let clear_val = cookie_config.build_clear_cookie_value();

            let mut response = (
                StatusCode::UNAUTHORIZED,
                Json(SessionResponse {
                    account_id: None,
                    account_generation: None,
                    account_kind: None,
                    role: None,
                    password_change_required: false,
                    authenticated: false,
                    is_admin: false,
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
    use crate::fs::search::SearchFilesHelper;
    use crate::fs::text::{SaveTextFileHelper, TextPreviewHelper};
    use crate::fs::trash::{
        DeletePermanentlyHelper, ListTrashHelper, MoveToTrashHelper, RestoreFromTrashHelper,
    };
    use crate::helper::types::{HelperRequest, HelperResponse};
    use crate::rate_limit::login::InMemoryLoginRateLimiter;
    use crate::session::store::InMemorySessionStore;
    use crate::upload::transfer::UploadHelper;
    use axum::body::to_bytes;
    use axum::http::header::COOKIE;
    use std::sync::Arc;

    #[derive(Clone, Debug)]
    struct DummyHelper;

    macro_rules! impl_dummy_helper {
        ($trait_name:ident) => {
            impl $trait_name for DummyHelper {
                fn execute_helper(
                    &self,
                    _request: &HelperRequest,
                ) -> Result<HelperResponse, crate::errors::TealDriveError> {
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
    impl_dummy_helper!(ListTrashHelper);
    impl_dummy_helper!(ChmodFileHelper);
    impl_dummy_helper!(UploadHelper);
    impl_dummy_helper!(CompressZipHelper);
    impl_dummy_helper!(CompressTarGzHelper);
    impl_dummy_helper!(ExtractArchiveHelper);
    impl_dummy_helper!(SearchFilesHelper);

    fn test_state(max_failures: u32) -> WsServerState<DummyHelper> {
        let auth = MockAuthenticator::new(AccountPolicyConfig {
            allowed_users: vec!["alice".into()],
            ..Default::default()
        })
        .with_user(MockUser::regular("alice", "secret123"));
        let mut config = AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: std::path::PathBuf::from("/home"),
                read_only: false,
                uploads_allowed: true,
                hidden_files_allowed: false,
                is_web_root: false,
                quota_bytes: None,
            }],
            ..AppConfig::default()
        };
        config.account_policy.allowed_users = vec!["alice".into()];
        config.rate_limit.max_login_failures = max_failures;
        let session_store = InMemorySessionStore::new(config.session.clone());
        let audit_sink = JsonlAuditSink::open(
            std::env::temp_dir().join(format!("tealdrive-audit-{}.jsonl", uuid::Uuid::new_v4())),
        )
        .expect("sink");
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
        let resp1 = login_handler(
            State(state.clone()),
            HeaderMap::new(),
            Json(payload.clone()),
        )
        .await;
        assert_eq!(resp1.status(), StatusCode::UNAUTHORIZED);

        // Attempt 2 -> 401
        let resp2 = login_handler(
            State(state.clone()),
            HeaderMap::new(),
            Json(payload.clone()),
        )
        .await;
        assert_eq!(resp2.status(), StatusCode::UNAUTHORIZED);

        // Attempt 3 -> 429
        let resp3 = login_handler(
            State(state.clone()),
            HeaderMap::new(),
            Json(payload.clone()),
        )
        .await;
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
        let cookie_raw = login_res
            .headers()
            .get(SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();

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

        auth_headers.insert(
            "x-csrf-token",
            session_data.csrf_token.unwrap().parse().unwrap(),
        );
        // 3. Logout
        let logout_res = logout_handler(State(state.clone()), auth_headers.clone()).await;
        assert_eq!(logout_res.status(), StatusCode::OK);
        let logout_cookie = logout_res
            .headers()
            .get(SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap();
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

#[derive(Deserialize, zeroize::ZeroizeOnDrop)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangePasswordPayload {
    current_password: String,
    new_password: String,
}

pub async fn revoke_self_handler<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    mutate_credentials(state, headers, None).await
}

pub async fn change_password_handler<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
    Json(payload): Json<ChangePasswordPayload>,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    mutate_credentials(state, headers, Some(payload)).await
}

async fn mutate_credentials<H>(
    state: WsServerState<H>,
    headers: HeaderMap,
    payload: Option<ChangePasswordPayload>,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    use crate::helper::account_gateway::{AccountCommand, AccountReply, GatewayFailure};
    let cfg = state.config.load();
    if !cfg.web_accounts.enabled {
        return StatusCode::NOT_IMPLEMENTED.into_response();
    }
    let Ok(id) = extract_session_id(&headers, &cfg.session.cookie_name) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let session = {
        let store = state.session_store.lock().await;
        match store.get_session(&id, InMemorySessionStore::now_epoch_seconds()) {
            SessionLookupResult::Found(session) => session,
            _ => return StatusCode::UNAUTHORIZED.into_response(),
        }
    };
    if !crate::session::csrf::validate_header(&session, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if !session.allowed_by(&cfg) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(grant) = session.grant.as_deref().cloned() else {
        return StatusCode::FORBIDDEN.into_response();
    };
    let password_change = payload.is_some();
    let now = InMemorySessionStore::now_epoch_seconds();
    let rate_key = LoginRateLimitKey {
        username: session.username.clone(),
        ip: extract_client_ip(&headers),
    };
    if password_change
        && state.rate_limiter.lock().await.check_at(
            &rate_key,
            cfg.rate_limit.max_login_failures,
            cfg.rate_limit.lockout_duration_seconds,
            now,
        ) == LoginRateLimitDecision::Limited
    {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    let command = match payload {
        Some(payload) => {
            if payload.current_password.len() > 1024
                || payload.new_password.len() > 1024
                || payload.new_password.chars().count() < 12
                || payload.new_password.contains('\0')
            {
                return StatusCode::BAD_REQUEST.into_response();
            }
            AccountCommand::ChangePassword {
                grant,
                current_password: zeroize::Zeroizing::new(payload.current_password.clone()),
                new_password: zeroize::Zeroizing::new(payload.new_password.clone()),
            }
        }
        None => AccountCommand::RevokeSelf(grant),
    };
    let Ok(permit) = crate::session::lease::validation_slots()
        .acquire_owned()
        .await
    else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Some(activity) = state.shutdown.enter() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let authenticator = state.authenticator.clone();
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let _activity = activity;
        authenticator.account_command(command)
    })
    .await;
    match result {
        Ok(Ok(AccountReply::AccessRevoked | AccountReply::PasswordChanged)) => {
            state.session_store.lock().await.revoke_session(&id);
            let mut event = AuditEvent::new(
                RequestId::new(),
                &session.username,
                session.uid,
                session.gid,
                if password_change {
                    crate::audit::event::AuditAction::PasswordChanged
                } else {
                    crate::audit::event::AuditAction::SessionRevoked
                },
                crate::audit::event::AuditStatus::Success,
            );
            event.session_id_hash = crate::audit::context::session_hash(session.session_id);
            event.operation = Some(
                if password_change {
                    "change_web_password"
                } else {
                    "revoke_self"
                }
                .into(),
            );
            let _ = state.audit_sink.emit(&event);
            let mut response = StatusCode::NO_CONTENT.into_response();
            let cookie =
                SessionCookieConfig::from_session_config(&cfg.session).build_clear_cookie_value();
            if let Ok(value) = HeaderValue::from_str(&cookie) {
                response.headers_mut().insert(SET_COOKIE, value);
            }
            response
        }
        Ok(Err(GatewayFailure::InvalidCredentials)) => {
            if password_change {
                state.rate_limiter.lock().await.record_failure_at(
                    rate_key,
                    cfg.rate_limit.lockout_duration_seconds,
                    now,
                );
            }
            StatusCode::UNAUTHORIZED.into_response()
        }
        Ok(Err(GatewayFailure::RateLimited)) => (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({"code":"RATE_LIMITED"})),
        )
            .into_response(),
        Ok(Err(GatewayFailure::Forbidden)) => StatusCode::FORBIDDEN.into_response(),
        Ok(Err(GatewayFailure::Conflict)) => StatusCode::CONFLICT.into_response(),
        _ => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
