use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::audit::event::{AuditEvent, AuditReason, AuditStatus};
use crate::audit::sink::JsonlAuditSink;
use crate::auth::account::Authenticator;
use crate::auth::api::{login_handler, logout_handler, session_handler};
use crate::config::{AppConfig, DynamicConfig};
use crate::download::transfer::DownloadHelper;
use crate::errors::{ErrorKind, TealDriveError};
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
use crate::protocol::frame::{RequestId, TdrvFrame};
use crate::protocol::message_type::MessageType;
use crate::rate_limit::login::InMemoryLoginRateLimiter;
use crate::session::store::{InMemorySessionStore, SessionId, SessionStore};
use crate::upload::transfer::UploadHelper;
use crate::ws::connection::WsConnectionContext;
use crate::ws::dispatch::{
    error_frame, handle_decoded_frame_with_service, operation_failed_frame, DispatchDecision,
};
use crate::ws::handshake::create_server_hello_frame;
use crate::ws::upgrade::{accept_session_bound_upgrade, WsUpgradeRequest};

pub trait WsHelperBackend:
    DirectoryListingHelper
    + MetadataHelper
    + DownloadHelper
    + TextPreviewHelper
    + SaveTextFileHelper
    + CreateFolderHelper
    + RenameHelper
    + MoveHelper
    + CopyHelper
    + MoveToTrashHelper
    + RestoreFromTrashHelper
    + DeletePermanentlyHelper
    + ListTrashHelper
    + ChmodFileHelper
    + UploadHelper
    + CompressZipHelper
    + CompressTarGzHelper
    + ExtractArchiveHelper
    + SearchFilesHelper
    + Send
    + Sync
{
}

impl<T> WsHelperBackend for T where
    T: DirectoryListingHelper
        + MetadataHelper
        + DownloadHelper
        + TextPreviewHelper
        + SaveTextFileHelper
        + CreateFolderHelper
        + RenameHelper
        + MoveHelper
        + CopyHelper
        + MoveToTrashHelper
        + RestoreFromTrashHelper
        + DeletePermanentlyHelper
        + ListTrashHelper
        + ChmodFileHelper
        + UploadHelper
        + CompressZipHelper
        + CompressTarGzHelper
        + ExtractArchiveHelper
        + SearchFilesHelper
        + Send
        + Sync
{
}

pub struct WsServerState<H> {
    pub config: DynamicConfig,
    pub shutdown: crate::shutdown::Shutdown,
    pub operation_slots: Arc<tokio::sync::Semaphore>,
    pub pre_auth_slots: Arc<tokio::sync::Semaphore>,
    pub auth_bridges: Arc<Mutex<std::collections::HashMap<String, crate::ws::auth::CookieBridge>>>,
    pub session_store: Arc<Mutex<InMemorySessionStore>>,
    pub helper_client: Arc<H>,
    pub audit_sink: Arc<JsonlAuditSink>,
    pub authenticator: Arc<dyn Authenticator>,
    pub rate_limiter: Arc<Mutex<InMemoryLoginRateLimiter>>,
    pub helper_timeout_updater: Option<Arc<dyn Fn(Duration) + Send + Sync>>,
}

impl<H: std::fmt::Debug> std::fmt::Debug for WsServerState<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WsServerState")
            .field("config", &self.config)
            .field("session_store", &"<session_store>")
            .field("helper_client", &self.helper_client)
            .field("audit_sink", &"<audit_sink>")
            .field("authenticator", &"<authenticator>")
            .field("rate_limiter", &"<rate_limiter>")
            .finish()
    }
}

impl<H> WsServerState<H> {
    pub async fn sync_runtime_config(&self, config: &AppConfig) {
        self.session_store
            .lock()
            .await
            .update_config(config.session.clone());
        self.set_helper_timeout(Duration::from_secs(config.helper.timeout_seconds));
        self.rate_limiter.lock().await.set_limits(
            config.rate_limit.max_login_failures,
            config.rate_limit.lockout_duration_seconds,
        );
    }
    pub fn config(&self) -> Arc<AppConfig> {
        self.config.load_full()
    }

    pub fn set_helper_timeout(&self, timeout: Duration) {
        if let Some(ref updater) = self.helper_timeout_updater {
            updater(timeout);
        }
    }

    pub fn with_timeout_updater<F>(mut self, updater: F) -> Self
    where
        F: Fn(Duration) + Send + Sync + 'static,
    {
        self.helper_timeout_updater = Some(Arc::new(updater));
        self
    }

    pub fn new(
        config: AppConfig,
        session_store: InMemorySessionStore,
        helper_client: H,
        audit_sink: JsonlAuditSink,
    ) -> Self {
        let max_failures = config.rate_limit.max_login_failures;
        let lockout = config.rate_limit.lockout_duration_seconds;
        Self::with_authenticator(
            config,
            session_store,
            helper_client,
            audit_sink,
            Arc::new(crate::auth::pam::PamAuthenticator::new()),
            InMemoryLoginRateLimiter::with_lockout(max_failures, lockout),
        )
    }

    pub fn with_authenticator(
        config: AppConfig,
        session_store: InMemorySessionStore,
        helper_client: H,
        audit_sink: JsonlAuditSink,
        authenticator: Arc<dyn Authenticator>,
        rate_limiter: InMemoryLoginRateLimiter,
    ) -> Self {
        Self::with_authenticator_and_path(
            config,
            None,
            session_store,
            helper_client,
            audit_sink,
            authenticator,
            rate_limiter,
        )
    }

    pub fn with_authenticator_and_path(
        config: AppConfig,
        config_path: Option<PathBuf>,
        session_store: InMemorySessionStore,
        helper_client: H,
        audit_sink: JsonlAuditSink,
        authenticator: Arc<dyn Authenticator>,
        rate_limiter: InMemoryLoginRateLimiter,
    ) -> Self {
        let audit_sink = Arc::new(audit_sink);
        let mut config = config;
        if config.audit.path.is_none() {
            config.audit.path = Some(audit_sink.path());
        }
        Self {
            config: DynamicConfig::new(config, config_path).with_audit_sink(audit_sink.clone()),
            shutdown: crate::shutdown::Shutdown::default(),
            operation_slots: Arc::new(tokio::sync::Semaphore::new(8)),
            pre_auth_slots: Arc::new(tokio::sync::Semaphore::new(64)),
            auth_bridges: Arc::new(Mutex::new(std::collections::HashMap::new())),
            session_store: Arc::new(Mutex::new(session_store)),
            helper_client: Arc::new(helper_client),
            audit_sink,
            authenticator,
            rate_limiter: Arc::new(Mutex::new(rate_limiter)),
            helper_timeout_updater: None,
        }
    }
}

impl<H> Clone for WsServerState<H> {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            shutdown: self.shutdown.clone(),
            operation_slots: self.operation_slots.clone(),
            pre_auth_slots: self.pre_auth_slots.clone(),
            auth_bridges: self.auth_bridges.clone(),
            session_store: self.session_store.clone(),
            helper_client: self.helper_client.clone(),
            audit_sink: self.audit_sink.clone(),
            authenticator: self.authenticator.clone(),
            rate_limiter: self.rate_limiter.clone(),
            helper_timeout_updater: self.helper_timeout_updater.clone(),
        }
    }
}

pub fn build_router<H>(state: WsServerState<H>) -> Router
where
    H: WsHelperBackend + Clone + 'static,
{
    Router::new()
        .route("/", get(crate::ui::serve_index::<H>))
        .route("/index.html", get(crate::ui::serve_index::<H>))
        .route("/style.css", get(crate::ui::serve_style::<H>))
        .route("/app.js", get(crate::ui::serve_app::<H>))
        .route("/favicon.svg", get(crate::ui::serve_favicon::<H>))
        .route("/favicon.ico", get(crate::ui::serve_favicon::<H>))
        .route("/static/*path", get(crate::ui::serve_static_asset::<H>))
        .route("/api/quota", get(crate::fs::usage::usage_handler::<H>))
        .route("/sdk/v1/tealdrive.js", get(crate::ui::serve_sdk))
        .route("/sdk/v1/tealdrive.d.ts", get(crate::ui::serve_sdk_types))
        .route("/api/health", get(health_check::<H>))
        .route("/api/login", post(login_handler::<H>))
        .route("/api/logout", post(logout_handler::<H>))
        .route(
            "/api/auth/revoke",
            post(crate::auth::api::revoke_self_handler::<H>),
        )
        .route(
            "/api/auth/password",
            post(crate::auth::api::change_password_handler::<H>),
        )
        .route("/api/session", get(session_handler::<H>))
        .route(
            "/api/admin/accounts/system",
            get(crate::admin::accounts::list_system::<H>),
        )
        .route(
            "/api/admin/virtual/list",
            get(crate::admin::accounts::list_virtual::<H>),
        )
        .route(
            "/api/admin/system/approve",
            post(crate::admin::accounts::approve::<H>),
        )
        .route(
            "/api/admin/system/allow",
            post(crate::admin::accounts::enable::<H>),
        )
        .route(
            "/api/admin/system/deny",
            post(crate::admin::accounts::disable::<H>),
        )
        .route(
            "/api/admin/virtual/enable",
            post(crate::admin::accounts::enable_virtual::<H>),
        )
        .route(
            "/api/admin/virtual/disable",
            post(crate::admin::accounts::disable_virtual::<H>),
        )
        .route(
            "/api/download",
            get(crate::download::api::download_handler::<H>),
        )
        .route(
            "/api/preview",
            get(crate::download::api::preview_handler::<H>),
        )
        .route(
            "/api/admin/config",
            get(crate::admin::api::get_config_handler::<H>),
        )
        .route(
            "/api/admin/config",
            patch(crate::admin::api::patch_config_handler::<H>),
        )
        .route(
            "/api/admin/config/reload",
            post(crate::admin::api::reload_config_handler::<H>),
        )
        .route(
            "/api/admin/users",
            get(crate::admin::api::get_users_handler::<H>),
        )
        .route("/ws", get(websocket_upgrade::<H>))
        .route("/ws/auth", get(crate::ws::auth::upgrade::<H>))
        .route("/api/session/bind", post(crate::ws::auth::bind_cookie::<H>))
        .fallback(crate::ui::serve_fallback::<H>)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            shutdown_admission::<H>,
        ))
        .with_state(state)
}

pub async fn run_server<H>(
    bind_addr: SocketAddr,
    state: WsServerState<H>,
) -> Result<(), TealDriveError>
where
    H: WsHelperBackend + Clone + 'static,
{
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .map_err(|_| TealDriveError::Internal)?;
    run_server_with_shutdown(
        listener,
        state,
        crate::shutdown::signal(),
        crate::shutdown::DRAIN_TIMEOUT,
    )
    .await
}

/// Injectable signal and listener for real connection shutdown tests.
pub async fn run_server_with_shutdown<H, F>(
    listener: tokio::net::TcpListener,
    state: WsServerState<H>,
    signal: F,
    drain_timeout: Duration,
) -> Result<(), TealDriveError>
where
    H: WsHelperBackend + Clone + 'static,
    F: std::future::Future<Output = ()>,
{
    let shutdown = state.shutdown.clone();
    let audit = state.audit_sink.clone();
    let expiry_config = state.config.clone();
    let expiry_store = state.session_store.clone();
    let expiry_audit = audit.clone();
    let expiry_shutdown = shutdown.clone();
    struct AbortTask(tokio::task::JoinHandle<()>);
    impl Drop for AbortTask {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let _expiry_task = AbortTask(tokio::spawn(async move {
        let mut timer = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                _ = expiry_shutdown.draining() => break,
                _ = timer.tick() => {
                    let disallowed = expiry_store.lock().await.take_disallowed(&expiry_config.load().account_policy);
                    for session in disallowed {
                        let mut event = AuditEvent::new(RequestId::new(), session.username, session.uid, session.gid, crate::audit::event::AuditAction::SessionRevoked, AuditStatus::Denied);
                        event.reason = Some(AuditReason::AccountNotAllowed);
                        event.session_id_hash = crate::audit::context::session_hash(session.session_id);
                        event.ip = session.ip;
                        let _ = expiry_audit.emit(&event);
                    }
                    let expired = expiry_store.lock().await.take_expired(InMemorySessionStore::now_epoch_seconds());
                    for session in expired {
                        let mut event = AuditEvent::session_expired(RequestId::new(), session.username, session.uid, session.gid);
                        event.session_id_hash = crate::audit::context::session_hash(session.session_id);
                        event.ip = session.ip;
                        event.user_agent = session.user_agent.map(|s| s.chars().take(512).collect());
                        let _ = expiry_audit.emit(&event);
                    }
                }
            }
        }
    }));
    let stop_accepting = shutdown.clone();
    let server = axum::serve(
        listener,
        build_router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        stop_accepting.draining().await;
    });
    let server = std::future::IntoFuture::into_future(server);
    tokio::pin!(server);
    tokio::select! {
        result = &mut server => return result.map_err(|_| TealDriveError::Internal),
        _ = signal => {},
    }
    eprintln!("Shutdown requested; rejecting new work and draining active operations.");
    let clean = shutdown
        .finish(drain_timeout, crate::shutdown::CANCEL_TIMEOUT)
        .await;
    // Hyper may still have idle/slow peers; it must not hold shutdown indefinitely.
    let _ = tokio::time::timeout(Duration::from_secs(2), &mut server).await;
    let event = AuditEvent::new(
        RequestId::new(),
        "server",
        unsafe { libc::geteuid() },
        unsafe { libc::getegid() },
        crate::audit::event::AuditAction::ServerShutdown,
        if clean {
            AuditStatus::Success
        } else {
            AuditStatus::Failed
        },
    );
    if let Err(error) = audit.emit(&event).and_then(|()| audit.sync()) {
        eprintln!("Failed to persist shutdown audit record: {error}");
        return Err(TealDriveError::Internal);
    }
    if clean {
        eprintln!("Shutdown complete; active connections and helper jobs released.");
        Ok(())
    } else {
        eprintln!("Shutdown incomplete: work did not stop within the cancellation deadline.");
        Err(TealDriveError::Internal)
    }
}

async fn shutdown_admission<H>(
    State(state): State<WsServerState<H>>,
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    use crate::audit::context::{AuditContext, REQUEST};
    let cfg = state.config.load_full();
    let peer = request
        .extensions()
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip());
    let session_id = extract_session_id(request.headers(), &cfg.session.cookie_name).ok();
    let session = if let Some(id) = session_id {
        match state
            .session_store
            .lock()
            .await
            .get_session(&id, InMemorySessionStore::now_epoch_seconds())
        {
            crate::session::store::SessionLookupResult::Found(session) => Some(session),
            _ => None,
        }
    } else {
        None
    };
    let context = AuditContext {
        request_id: Some(RequestId::new()),
        user: session.as_ref().map(|s| crate::config::UserContext {
            username: s.username.clone(),
            uid: s.uid,
            gid: s.gid,
        }),
        session_id,
        ip: cfg
            .proxy
            .resolve(peer, request.headers())
            .map(|ip| ip.to_string()),
        peer_ip: peer.map(|ip| ip.to_string()),
        user_agent: request
            .headers()
            .get(axum::http::header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.chars().take(512).collect()),
    };
    // Raw proxy headers never reach identity/rate-limit code. It uses the scoped verified address.
    request.headers_mut().remove("x-tealdrive-client-ip");
    let mut audit = crate::audit::http::HttpAudit::new(
        request.method(),
        request.uri(),
        context.clone(),
        state.audit_sink.clone(),
    );
    if session.as_ref().is_some_and(|s| !s.allowed_by(&cfg))
        && (request.uri().path().starts_with("/api/") || request.uri().path() == "/ws")
        && request.uri().path() != "/api/login"
    {
        audit.rejected(AuditReason::AccountNotAllowed);
        audit.response(StatusCode::UNAUTHORIZED, &HeaderMap::new());
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let csrf_invalid = request.uri().path() != "/api/session/bind"
        && session
            .as_ref()
            .is_some_and(|s| !crate::session::csrf::validate_header(s, request.headers()))
        && matches!(
            *request.method(),
            axum::http::Method::POST | axum::http::Method::PATCH | axum::http::Method::DELETE
        );
    let admin_required = request.uri().path().starts_with("/api/admin/")
        && session
            .as_ref()
            .is_some_and(|s| !cfg.account_policy.is_admin(&s.username));
    let path = request.uri().path().to_owned();
    let Some(activity) = state.shutdown.enter() else {
        audit.fail(AuditReason::Shutdown);
        return (StatusCode::SERVICE_UNAVAILABLE, "server shutting down").into_response();
    };
    let cancellation = state.shutdown.cancellation();
    let mut response = tokio::select! {
        response = REQUEST.scope(context.clone(), next.run(request)) => response,
        _ = cancellation.cancelled() => { audit.fail(AuditReason::Shutdown);return StatusCode::SERVICE_UNAVAILABLE.into_response(); },
    };
    audit.response(response.status(), response.headers());
    if let Some(failure) = response
        .extensions()
        .get::<crate::audit::http::AuditFailure>()
    {
        audit.rejected(failure.0);
    }
    if response.status() == StatusCode::FORBIDDEN {
        if admin_required {
            audit.rejected(AuditReason::AdminRequired);
        } else if csrf_invalid {
            audit.rejected(AuditReason::CsrfRejected);
        }
    }
    if path == "/api/login" && response.status().is_success() {
        if let Some(cookie) = response
            .headers()
            .get(axum::http::header::SET_COOKIE)
            .cloned()
        {
            let mut headers = HeaderMap::new();
            headers.insert(axum::http::header::COOKIE, cookie);
            if let Ok(id) = extract_session_id(&headers, &cfg.session.cookie_name) {
                if let crate::session::store::SessionLookupResult::Found(session) = state
                    .session_store
                    .lock()
                    .await
                    .get_session(&id, InMemorySessionStore::now_epoch_seconds())
                {
                    let mut logged_in = context.clone();
                    logged_in.session_id = Some(id);
                    logged_in.user = Some(crate::config::UserContext {
                        username: session.username,
                        uid: session.uid,
                        gid: session.gid,
                    });
                    audit.set_context(logged_in);
                }
            }
        }
    }
    response.headers_mut().insert(
        "x-tealdrive-audit-write-failures",
        state
            .audit_sink
            .write_failures()
            .to_string()
            .parse()
            .unwrap(),
    );
    let (parts, body) = response.into_parts();
    let stream = futures_util::stream::unfold(
        (body.into_data_stream(), activity, cancellation, audit),
        |(mut body, activity, cancellation, mut audit)| async move {
            tokio::select! {
                biased;
                _ = cancellation.cancelled() => { audit.fail(AuditReason::Shutdown);None },
                item = body.next() => match item {
                    Some(Ok(bytes)) => { audit.chunk(bytes.len());Some((Ok(bytes),(body,activity,cancellation,audit))) },
                    Some(Err(error)) => { audit.fail(AuditReason::StreamFailed);Some((Err(error),(body,activity,cancellation,audit))) },
                    None => { audit.end();None },
                },
            }
        },
    );
    Response::from_parts(parts, axum::body::Body::from_stream(stream))
}

async fn health_check<H>(State(state): State<WsServerState<H>>) -> Response {
    let mut response = "ok".into_response();
    response.headers_mut().insert(
        "x-tealdrive-audit-write-failures",
        state
            .audit_sink
            .write_failures()
            .to_string()
            .parse()
            .unwrap(),
    );
    response
}

async fn websocket_upgrade<H>(
    ws: WebSocketUpgrade,
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    let now = InMemorySessionStore::now_epoch_seconds();
    let upgrade_request = match build_upgrade_request(&state, &headers, now) {
        Ok(request) => request,
        Err(error) => {
            let _ = state.audit_sink.emit(&AuditEvent::websocket_rejection(
                RequestId::new(),
                "unknown",
                0,
                0,
                audit_reason_for_upgrade_error(&error),
            ));
            return map_upgrade_error(error).into_response();
        }
    };

    let (security_cfg, features_cfg) = {
        let cfg = state.config.load();
        (cfg.security.clone(), cfg.features.clone())
    };
    let context = {
        let store = state.session_store.lock().await;
        match accept_session_bound_upgrade(&*store, &security_cfg, features_cfg, upgrade_request) {
            Ok(context) => context,
            Err(error) => {
                let _ = state.audit_sink.emit(&AuditEvent::websocket_rejection(
                    RequestId::new(),
                    "unknown",
                    0,
                    0,
                    audit_reason_for_upgrade_error(&error),
                ));
                return map_upgrade_error(error).into_response();
            }
        }
    };

    {
        let mut store = state.session_store.lock().await;
        let _ = store.touch_session(&context.session_id, now);
    }

    let Some(activity) = state.shutdown.enter() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let mut audit_context = crate::audit::context::current();
    audit_context.request_id = None;
    audit_context.session_id = Some(context.session_id);
    audit_context.user = Some(context.user_context());
    ws.on_upgrade(move |socket| async move {
        let _activity = activity;
        let _ = crate::audit::context::REQUEST
            .scope(
                audit_context,
                handle_socket_connection(socket, state, context),
            )
            .await;
    })
    .into_response()
}

fn build_upgrade_request<H>(
    state: &WsServerState<H>,
    headers: &HeaderMap,
    now: u64,
) -> Result<WsUpgradeRequest, TealDriveError> {
    let cfg = state.config.load();
    if let Some(origin) = headers
        .get(axum::http::header::ORIGIN)
        .and_then(|h| h.to_str().ok())
    {
        if cfg.security.allowed_origins.is_empty() {
            let host = headers
                .get(axum::http::header::HOST)
                .and_then(|h| h.to_str().ok())
                .ok_or(TealDriveError::PermissionDenied)?;
            let scheme = if cfg.server.behind_tls_proxy {
                "https"
            } else {
                "http"
            };
            if origin != format!("{scheme}://{host}") {
                return Err(TealDriveError::PermissionDenied);
            }
        }
    }
    let cookie_name = state.config.load().session.cookie_name.clone();
    let session_id = extract_session_id(headers, &cookie_name)?;
    Ok(WsUpgradeRequest {
        session_id: Some(session_id),
        origin: headers
            .get(axum::http::header::ORIGIN)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned),
        user_agent: headers
            .get(axum::http::header::USER_AGENT)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned),
        ip: extract_client_ip(headers),
        now,
    })
}

pub fn extract_session_id(
    headers: &HeaderMap,
    cookie_name: &str,
) -> Result<SessionId, TealDriveError> {
    let cookie_header = headers
        .get(axum::http::header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .ok_or(TealDriveError::PermissionDenied)?;

    for part in cookie_header.split(';') {
        let (name, value) = match part.trim().split_once('=') {
            Some(pair) => pair,
            None => continue,
        };
        if name == cookie_name {
            let parsed =
                Uuid::parse_str(value.trim()).map_err(|_| TealDriveError::PermissionDenied)?;
            return Ok(SessionId(parsed));
        }
    }

    Err(TealDriveError::PermissionDenied)
}

pub fn extract_client_ip(_headers: &HeaderMap) -> Option<String> {
    crate::audit::context::current().ip
}

fn map_upgrade_error(error: TealDriveError) -> (StatusCode, &'static str) {
    match error {
        TealDriveError::Validation => (StatusCode::BAD_REQUEST, "invalid websocket request"),
        TealDriveError::PermissionDenied => (StatusCode::UNAUTHORIZED, "permission denied"),
        TealDriveError::UnsupportedVersion => (StatusCode::BAD_REQUEST, "unsupported version"),
        TealDriveError::InvalidEncoding => (StatusCode::BAD_REQUEST, "invalid encoding"),
        TealDriveError::InvalidCompression => (StatusCode::BAD_REQUEST, "invalid compression"),
        _ => (StatusCode::BAD_REQUEST, "websocket upgrade rejected"),
    }
}

fn audit_reason_for_upgrade_error(error: &TealDriveError) -> AuditReason {
    match error {
        TealDriveError::Validation => AuditReason::HandshakeFailed,
        TealDriveError::PermissionDenied => AuditReason::HandshakeFailed,
        _ => AuditReason::HandshakeFailed,
    }
}

struct UploadCleanup {
    context: WsConnectionContext,
    sink: Arc<JsonlAuditSink>,
    shutdown: crate::shutdown::Shutdown,
    audit_context: crate::audit::context::AuditContext,
}
impl std::ops::Deref for UploadCleanup {
    type Target = WsConnectionContext;
    fn deref(&self) -> &Self::Target {
        &self.context
    }
}
impl std::ops::DerefMut for UploadCleanup {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.context
    }
}
impl Drop for UploadCleanup {
    fn drop(&mut self) {
        for upload in self.context.active_uploads.values() {
            let _ = std::fs::remove_file(&upload.staging_path);
            let mut event = AuditEvent::new(
                RequestId::new(),
                &self.context.username,
                self.context.uid,
                self.context.gid,
                crate::audit::AuditAction::Upload,
                AuditStatus::Cancelled,
            );
            self.audit_context.enrich(&mut event);
            event.phase = Some("transfer_aborted".into());
            event.reason = Some(if self.shutdown.is_draining() {
                AuditReason::Shutdown
            } else {
                AuditReason::ConnectionClosed
            });
            event.root_id = Some(upload.root_id.clone());
            event.relative_path = Some(crate::config::RelativePath::new(
                if upload.parent_relative_path.0.is_empty() {
                    upload.filename.0.clone()
                } else {
                    format!("{}/{}", upload.parent_relative_path.0, upload.filename.0)
                },
            ));
            event.transfer_id = Some(upload.transfer_id);
            event.bytes_in = upload.bytes_received;
            let _ = self.sink.emit(&event);
        }
        let mut event = AuditEvent::websocket_disconnect(
            RequestId::new(),
            self.context.username.clone(),
            self.context.uid,
            self.context.gid,
        );
        self.audit_context.enrich(&mut event);
        event.reason = Some(if self.shutdown.is_draining() {
            AuditReason::Shutdown
        } else {
            AuditReason::ConnectionClosed
        });
        let _ = self.sink.emit(&event);
    }
}

pub(super) async fn handle_socket_connection<H>(
    socket: WebSocket,
    state: WsServerState<H>,
    context: WsConnectionContext,
) -> Result<(), TealDriveError>
where
    H: WsHelperBackend + 'static,
{
    let WsServerState {
        config,
        session_store,
        helper_client,
        authenticator,
        audit_sink,
        operation_slots,
        shutdown,
        ..
    } = state;
    let mut context = UploadCleanup {
        context,
        sink: audit_sink.clone(),
        shutdown: shutdown.clone(),
        audit_context: crate::audit::context::current(),
    };
    let initial_config = config.load_full();
    let (mut sender, mut receiver) = socket.split();
    if !context.protocol_ready {
        let server_hello = create_server_hello_frame(&initial_config)?;
        send_frame(&mut sender, &shutdown, &server_hello).await?;
    }

    let _ = audit_sink.emit(&AuditEvent::websocket_connect(
        RequestId::new(),
        context.username.clone(),
        context.uid,
        context.gid,
    ));

    let lease = crate::session::lease::SessionLease::with_shutdown_for_session(
        session_store.clone(),
        context.session_id,
        shutdown.cancellation(),
        authenticator.clone(),
    )
    .await;
    let mut shutdown_notified = false;

    loop {
        let message = tokio::select! {
            _=shutdown.draining()=> {
                if !shutdown_notified { notify_shutdown(&mut sender, &shutdown).await; }
                break;
            },
            _=lease.cancellation.cancelled()=>break,
            message=receiver.next()=>match message { Some(message)=>message,None=>break },
        };
        {
            let store = session_store.lock().await;
            if !matches!(
                store.get_session(
                    &context.session_id,
                    InMemorySessionStore::now_epoch_seconds()
                ),
                crate::session::store::SessionLookupResult::Found(_)
            ) {
                break;
            }
        }

        let message = match message {
            Ok(message) => message,
            Err(_) => {
                break;
            }
        };

        let now = InMemorySessionStore::now_epoch_seconds();
        match message {
            Message::Binary(bytes) => {
                let frame = match TdrvFrame::decode(&bytes) {
                    Ok(frame) => frame,
                    Err(_) => {
                        let _ = audit_sink.emit(&AuditEvent::websocket_rejection(
                            RequestId::new(),
                            context.username.clone(),
                            context.uid,
                            context.gid,
                            AuditReason::ProtocolError,
                        ));
                        let error = error_frame(RequestId::new(), "Malformed websocket frame.")?;
                        send_frame(&mut sender, &shutdown, &error).await?;
                        break;
                    }
                };

                let request_id = frame.header.request_id;
                let current_config = config.load_full();
                let authorized = match session_store.lock().await.get_session(
                    &context.session_id,
                    InMemorySessionStore::now_epoch_seconds(),
                ) {
                    crate::session::store::SessionLookupResult::Found(session) => {
                        session.allowed_by(&current_config)
                    }
                    _ => false,
                };
                if !authorized {
                    let _ = audit_sink.emit(&AuditEvent::websocket_rejection(
                        request_id,
                        context.username.clone(),
                        context.uid,
                        context.gid,
                        AuditReason::AccountNotAllowed,
                    ));
                    let response = error_frame(request_id, "Session is no longer authorized.")?;
                    send_frame(&mut sender, &shutdown, &response).await?;
                    break;
                }
                context.features = current_config.features.clone();
                let permit = tokio::select! {
                        _=shutdown.draining()=> {
                            if !shutdown_notified { notify_shutdown(&mut sender, &shutdown).await; }
                            break;
                        },
                _=lease.cancellation.cancelled()=>break,
                        permit=operation_slots.clone().acquire_owned()=>permit.map_err(|_|TealDriveError::Internal)?,
                    };
                let Some(activity) = shutdown.enter() else {
                    if !shutdown_notified {
                        notify_shutdown(&mut sender, &shutdown).await;
                    }
                    break;
                };
                let helper = helper_client.clone();
                let token = lease.cancellation.clone();
                let inbound = frame.clone();
                let mut operation_audit = crate::audit::operation::OperationAudit::new(
                    &frame,
                    &context,
                    &crate::audit::context::current(),
                    audit_sink.clone(),
                    token.clone(),
                );
                let observer = operation_audit.observer.clone();
                let mut job = tokio::task::spawn_blocking(move || {
                    let _activity = activity;
                    let _permit = permit;
                    let result = crate::audit::operation::with_observer(observer, || {
                        crate::session::lease::with_cancellation(token, || {
                            crate::helper::policy::with_snapshot(&current_config, || {
                                handle_decoded_frame_with_service(
                                    &mut context,
                                    inbound,
                                    &current_config,
                                    helper.as_ref(),
                                )
                            })
                        })
                    });
                    operation_audit.finish(&result);
                    (context, result, operation_audit.event.clone())
                });
                let (returned_context, result, delivery_event) = loop {
                    tokio::select! {
                        biased;
                        _=shutdown.draining(), if !shutdown_notified => {
                            shutdown_notified = true;
                            notify_shutdown(&mut sender, &shutdown).await;
                        },
                        result=&mut job => break result.map_err(|_|TealDriveError::Internal)?,
                        _=lease.cancellation.cancelled()=> {
                            let (returned_context,_,delivery_event)=job.await.map_err(|_|TealDriveError::Internal)?;
                            context=returned_context;
                            break (context, Err(TealDriveError::PermissionDenied), delivery_event);
                        }
                    }
                };
                context = returned_context;
                if lease.cancellation.is_cancelled() {
                    if frame.header.message_type == MessageType::DownloadBegin {
                        drop(crate::audit::operation::WsDelivery::new(
                            delivery_event.clone(),
                            audit_sink.clone(),
                        ));
                    }
                    break;
                }
                match result {
                    Ok(DispatchDecision::Outbound(frames)) => {
                        let mut delivery = if frame.header.message_type
                            == MessageType::DownloadBegin
                            && frames
                                .iter()
                                .any(|f| f.header.message_type == MessageType::DownloadEnd)
                        {
                            Some(crate::audit::operation::WsDelivery::new(
                                delivery_event,
                                audit_sink.clone(),
                            ))
                        } else {
                            None
                        };
                        for outbound in frames {
                            send_frame(&mut sender, &shutdown, &outbound).await?;
                            if let Some(delivery) = delivery.as_mut() {
                                delivery.frame(&outbound);
                            }
                        }
                        if let Some(delivery) = delivery.as_mut() {
                            delivery.finish();
                        }
                        let mut store = session_store.lock().await;
                        let _ = store.touch_session(&context.session_id, now);
                    }
                    Ok(DispatchDecision::DispatchPlaceholder(frame)) => {
                        send_frame(&mut sender, &shutdown, &frame).await?;
                    }
                    Err(error) if is_protocol_error(&error) => {
                        let response = error_frame(request_id, protocol_error_message(&error))?;
                        send_frame(&mut sender, &shutdown, &response).await?;
                        let _ = audit_sink.emit(&AuditEvent::websocket_rejection(
                            RequestId::new(),
                            context.username.clone(),
                            context.uid,
                            context.gid,
                            audit_reason_for_socket_error(&error),
                        ));
                        break;
                    }
                    Err(error) => {
                        let response = operation_failed_response(
                            request_id,
                            frame.header.message_type,
                            &error,
                        )?;
                        send_frame(&mut sender, &shutdown, &response).await?;
                        let _ = audit_sink.emit(&AuditEvent::websocket_rejection(
                            RequestId::new(),
                            context.username.clone(),
                            context.uid,
                            context.gid,
                            audit_reason_for_socket_error(&error),
                        ));
                    }
                }
            }
            Message::Ping(payload) => {
                sender
                    .send(Message::Pong(payload))
                    .await
                    .map_err(|_| TealDriveError::Internal)?;
            }
            Message::Pong(_) => {
                let mut store = session_store.lock().await;
                let _ = store.touch_session(&context.session_id, now);
            }
            Message::Close(_) => {
                break;
            }
            Message::Text(_) => {
                let _ = audit_sink.emit(&AuditEvent::websocket_rejection(
                    RequestId::new(),
                    context.username.clone(),
                    context.uid,
                    context.gid,
                    AuditReason::ProtocolError,
                ));
                let response = error_frame(RequestId::new(), "Binary TDRV/1 frames are required.")?;
                send_frame(&mut sender, &shutdown, &response).await?;
                break;
            }
        }
    }

    let close = shutdown
        .is_draining()
        .then(|| axum::extract::ws::CloseFrame {
            code: 1001,
            reason: "Server shutting down".into(),
        });
    let _ = tokio::time::timeout(Duration::from_secs(1), sender.send(Message::Close(close))).await;
    Ok(())
}

fn shutdown_notification_frame() -> Result<TdrvFrame, TealDriveError> {
    let payload = crate::protocol::payload::ConnectionStatusPayload {
        protocol_ready: false,
        message: Some("Server is shutting down. Sign in again after it restarts.".into()),
        shutting_down: true,
        grace_period_seconds: Some(crate::shutdown::DRAIN_TIMEOUT.as_secs()),
    };
    let header = crate::protocol::header::TdrvHeader::new(
        MessageType::ConnectionStatus,
        RequestId::new(),
        0,
    );
    TdrvFrame::new(header, crate::protocol::codec::encode_msgpack(&payload)?)
}

async fn notify_shutdown(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    shutdown: &crate::shutdown::Shutdown,
) {
    if let Ok(frame) = shutdown_notification_frame() {
        // A slow peer must not extend the shutdown deadline or abort its active job.
        let _ = tokio::time::timeout(Duration::from_secs(1), send_frame(sender, shutdown, &frame))
            .await;
    }
}

async fn send_frame(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    shutdown: &crate::shutdown::Shutdown,
    frame: &TdrvFrame,
) -> Result<(), TealDriveError> {
    let bytes = frame.encode()?;
    let cancellation = shutdown.cancellation();
    tokio::select! {
        _ = cancellation.cancelled() => Err(TealDriveError::Internal),
        result = sender.send(Message::Binary(bytes)) => result.map_err(|_| TealDriveError::Internal),
    }
}

fn is_protocol_error(error: &TealDriveError) -> bool {
    matches!(
        error,
        TealDriveError::Validation
            | TealDriveError::Protocol
            | TealDriveError::InvalidMagic
            | TealDriveError::UnsupportedVersion
            | TealDriveError::InvalidHeaderLength
            | TealDriveError::PayloadTooLarge
            | TealDriveError::UnknownMessageType
            | TealDriveError::InvalidMessageDirection
            | TealDriveError::InvalidFlags
            | TealDriveError::InvalidEncoding
            | TealDriveError::InvalidCompression
            | TealDriveError::PayloadLengthMismatch
            | TealDriveError::InvalidChunk
    )
}

fn protocol_error_message(error: &TealDriveError) -> &'static str {
    match error {
        TealDriveError::RevisionConflict => "File revision changed.",
        TealDriveError::Validation => "Validation failed.",
        TealDriveError::Protocol => "Protocol error.",
        TealDriveError::InvalidMagic => "Invalid TDRV magic.",
        TealDriveError::UnsupportedVersion => "Unsupported TDRV version.",
        TealDriveError::InvalidHeaderLength => "Invalid header length.",
        TealDriveError::PayloadTooLarge => "Payload too large.",
        TealDriveError::UnknownMessageType => "Unknown message type.",
        TealDriveError::InvalidMessageDirection => "Invalid message direction.",
        TealDriveError::InvalidFlags => "Invalid flags.",
        TealDriveError::InvalidEncoding => "Invalid encoding.",
        TealDriveError::InvalidCompression => "Invalid compression.",
        TealDriveError::PayloadLengthMismatch => "Payload length mismatch.",
        TealDriveError::InvalidChunk => "Invalid chunk.",
        _ => "Protocol error.",
    }
}

fn operation_failed_response(
    request_id: RequestId,
    message_type: MessageType,
    error: &TealDriveError,
) -> Result<TdrvFrame, TealDriveError> {
    let (error_kind, safe_message) = match error {
        TealDriveError::RevisionConflict => (
            ErrorKind::RevisionConflict,
            "File changed. Reload or compare before saving.",
        ),
        TealDriveError::PermissionDenied => (ErrorKind::PermissionDenied, "Permission denied."),
        TealDriveError::PolicyDenied => (ErrorKind::PolicyDenied, "Operation denied by policy."),
        TealDriveError::FeatureDisabled => (ErrorKind::FeatureDisabled, "Feature disabled."),
        TealDriveError::NotFound => (ErrorKind::NotFound, "Item not found."),
        TealDriveError::InvalidPath => (ErrorKind::InvalidPath, "Invalid path."),
        TealDriveError::AlreadyExists => (ErrorKind::AlreadyExists, "Target already exists."),
        TealDriveError::InvalidTarget => (ErrorKind::InvalidTarget, "Invalid target."),
        TealDriveError::CrossDeviceMove => (
            ErrorKind::CrossDeviceMove,
            "Cross-device move not supported.",
        ),
        TealDriveError::FileTooLarge => (ErrorKind::FileTooLarge, "File is too large."),
        TealDriveError::DiskFull => (ErrorKind::DiskFull, "Disk is full."),
        TealDriveError::UnsupportedBinaryFile => (
            ErrorKind::UnsupportedBinaryFile,
            "Binary file is not supported.",
        ),
        TealDriveError::InvalidTextEncoding => {
            (ErrorKind::InvalidTextEncoding, "Invalid text encoding.")
        }
        TealDriveError::InvalidFilename => (ErrorKind::NameTooLong, "Invalid filename."),
        TealDriveError::ReadOnlyRoot => (ErrorKind::ReadOnlyFilesystem, "Read-only filesystem."),
        TealDriveError::WebrootExecutableDenied => {
            (ErrorKind::PermissionDenied, "Executable upload denied.")
        }
        TealDriveError::HelperSpawnFailed
        | TealDriveError::HelperTimeout
        | TealDriveError::HelperStdinFailed
        | TealDriveError::HelperStdoutFailed
        | TealDriveError::HelperMalformedResponse
        | TealDriveError::HelperNonZeroExit
        | TealDriveError::HelperRequestTooLarge
        | TealDriveError::HelperResponseTooLarge
        | TealDriveError::HelperDecodeFailed
        | TealDriveError::HelperEncodeFailed
        | TealDriveError::HelperCommandNotImplemented
        | TealDriveError::Internal => (ErrorKind::InternalError, "Internal error."),
        _ => (ErrorKind::InternalError, "Operation failed."),
    };

    operation_failed_frame(request_id, message_type, error_kind, safe_message)
}

fn audit_reason_for_socket_error(error: &TealDriveError) -> AuditReason {
    match error {
        TealDriveError::Validation => AuditReason::OperationBeforeReady,
        TealDriveError::InvalidMessageDirection => AuditReason::InvalidMessageDirection,
        TealDriveError::UnsupportedVersion | TealDriveError::InvalidMagic => {
            AuditReason::HandshakeFailed
        }
        _ => AuditReason::HandshakeFailed,
    }
}

pub fn spawn_config_watcher<H: 'static>(
    state: &WsServerState<H>,
) -> Option<notify::RecommendedWatcher> {
    let config_path = state.config.config_path()?.to_path_buf();
    let config = state.config.clone();
    let audit_sink = state.audit_sink.clone();
    let session_store = state.session_store.clone();
    let rate_limiter = state.rate_limiter.clone();
    let helper_timeout_updater = state.helper_timeout_updater.clone();

    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(16);

    let mut watched_paths = vec![config_path.clone()];
    if let Some(path) = state.config.runtime_settings_path() {
        watched_paths.push(path.to_path_buf());
    }
    let matched_paths = watched_paths.clone();
    let watcher_res =
        notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                if !event
                    .paths
                    .iter()
                    .any(|path| matched_paths.iter().any(|watched| path == watched))
                {
                    return;
                }
                use notify::EventKind;
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        let _ = tx.try_send(());
                    }
                    _ => {}
                }
            }
        });

    let mut watcher = match watcher_res {
        Ok(w) => w,
        Err(err) => {
            eprintln!(
                "Failed to initialize file watcher for {}: {}",
                config_path.display(),
                err
            );
            return None;
        }
    };

    use notify::Watcher;
    let parents: std::collections::HashSet<_> = watched_paths
        .iter()
        .map(|path| {
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(std::path::Path::new("."))
        })
        .collect();
    for parent in parents {
        if let Err(err) = watcher.watch(parent, notify::RecursiveMode::NonRecursive) {
            eprintln!(
                "Failed to watch config directory {}: {}",
                parent.display(),
                err
            );
            return None;
        }
    }

    tokio::spawn(async move {
        while rx.recv().await.is_some() {
            tokio::time::sleep(Duration::from_millis(300)).await;
            while rx.try_recv().is_ok() {}

            match config.reload() {
                Ok(reloaded) => {
                    let _ = audit_sink.emit(&AuditEvent::config_reload(
                        RequestId::new(),
                        "config_watcher",
                        0,
                        0,
                        AuditStatus::Success,
                    ));
                    {
                        let mut store = session_store.lock().await;
                        store.update_config(reloaded.session.clone());
                    }
                    if let Some(ref updater) = helper_timeout_updater {
                        updater(Duration::from_secs(reloaded.helper.timeout_seconds));
                    }
                    {
                        let mut limiter = rate_limiter.lock().await;
                        limiter.set_limits(
                            reloaded.rate_limit.max_login_failures,
                            reloaded.rate_limit.lockout_duration_seconds,
                        );
                    }
                    eprintln!(
                        "Config successfully reloaded from {}",
                        config_path.display()
                    );
                }
                Err(err) => {
                    let mut ev = AuditEvent::config_reload(
                        RequestId::new(),
                        "config_watcher",
                        0,
                        0,
                        AuditStatus::Failed,
                    );
                    ev.reason = Some(AuditReason::InvalidConfig);
                    let _ = audit_sink.emit(&ev);
                    eprintln!("Config reload failed: {}", err);
                }
            }
        }
    });

    Some(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::sink::JsonlAuditSink;
    use crate::config::{AllowedRoot, RelativePath, RootId};
    use crate::helper::create_folder::create_folder_with_roots;
    use crate::helper::download::download_file_with_roots;
    use crate::helper::listing::list_directory_with_roots;
    use crate::helper::metadata::file_metadata_with_roots;
    use crate::helper::move_copy::{copy_file_with_roots, move_file_with_roots};
    use crate::helper::rename::rename_with_roots;
    use crate::helper::text::read_text_file_with_roots;
    use crate::helper::types::{HelperRequest, HelperResponse};
    use crate::protocol::header::{Encoding, TdrvHeader};
    use crate::protocol::message_type::MessageType;
    use crate::protocol::payload::ListDirectoryRequest;
    use crate::protocol::schema::{FileFilter, FilterMode, SortMode};
    use crate::ws::upgrade::tests::valid_context;
    use std::fs;
    use std::path::PathBuf;

    #[derive(Clone)]
    struct InProcessHelper {
        roots: Vec<AllowedRoot>,
    }

    impl DirectoryListingHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(list_directory_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl MetadataHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(file_metadata_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl DownloadHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(download_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl TextPreviewHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(read_text_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl CreateFolderHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(create_folder_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl RenameHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(rename_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl MoveHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(move_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl CopyHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(copy_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl MoveToTrashHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(crate::helper::trash::move_file_to_trash_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl RestoreFromTrashHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(crate::helper::trash::restore_file_from_trash_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl DeletePermanentlyHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(crate::helper::trash::delete_permanently_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl ListTrashHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(crate::helper::trash::list_trash_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl ChmodFileHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(crate::helper::chmod::chmod_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl UploadHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(crate::helper::upload::write_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl SaveTextFileHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(crate::helper::upload::write_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl CompressZipHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(crate::helper::archive::compress_zip_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl CompressTarGzHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(crate::helper::archive::compress_targz_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl ExtractArchiveHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(crate::helper::archive::extract_archive_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl SearchFilesHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(crate::helper::search::search_files_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    fn temp_root() -> PathBuf {
        let path = std::env::temp_dir().join(format!("tealdrive-ws-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temp root");
        path
    }

    fn config_for(path: PathBuf) -> AppConfig {
        AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: path,
                read_only: false,
                uploads_allowed: true,
                hidden_files_allowed: false,
                is_web_root: false,
                quota_bytes: None,
            }],
            ..AppConfig::default()
        }
    }

    fn list_request() -> ListDirectoryRequest {
        ListDirectoryRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(""),
            cursor: None,
            limit: 200,
            include_hidden: false,
            sort: SortMode::NameAsc,
            filter: FilterMode {
                extensions: None,
                query: None,
                file_kind: None,
                filter: Some(FileFilter::All),
            },
        }
    }

    #[test]
    fn session_cookie_extraction_works() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            "tealdrive_session=550e8400-e29b-41d4-a716-446655440000"
                .parse()
                .expect("cookie"),
        );

        let session_id = extract_session_id(&headers, "tealdrive_session").expect("session");

        assert_eq!(
            session_id,
            SessionId(Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").expect("uuid"))
        );
    }

    #[test]
    fn live_dispatch_routes_list_directory() {
        let base = temp_root();
        fs::write(base.join("visible.txt"), b"hello").expect("file");
        let config = config_for(base);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let mut context = valid_context();
        context.protocol_ready = true;
        let frame = {
            let mut header = TdrvHeader::new(MessageType::ListDirectory, RequestId::new(), 0);
            header.encoding = Encoding::MessagePack;
            TdrvFrame::new(
                header,
                crate::protocol::codec::encode_msgpack(&list_request()).expect("payload"),
            )
            .expect("frame")
        };

        let decision = handle_decoded_frame_with_service(&mut context, frame, &config, &helper)
            .expect("dispatch");

        assert!(matches!(decision, DispatchDecision::Outbound(_)));
    }

    #[test]
    fn build_upgrade_request_requires_cookie() {
        let config = AppConfig::default();
        let state = WsServerState::new(
            config,
            InMemorySessionStore::new(crate::config::SessionConfig::default()),
            InProcessHelper { roots: vec![] },
            JsonlAuditSink::open(
                std::env::temp_dir()
                    .join(format!("tealdrive-audit-{}.jsonl", uuid::Uuid::new_v4())),
            )
            .expect("audit"),
        );
        let headers = HeaderMap::new();

        assert!(matches!(
            build_upgrade_request(&state, &headers, 1),
            Err(TealDriveError::PermissionDenied)
        ));
    }
    #[test]
    fn upload_cleanup_runs_on_early_error_and_unwind() {
        let root = temp_root();
        let config = config_for(root.clone());
        let policy = crate::policy::decision::PolicyEngine::new(&config);
        let mut context = valid_context();
        let (upload, _) = crate::upload::transfer::upload_begin_request(
            &context.user_context(),
            crate::protocol::payload::UploadBeginRequest {
                root_id: RootId::new("home"),
                relative_path: RelativePath::new(""),
                filename: "incomplete.txt".into(),
                total_size: 10,
                chunk_size: 10,
                sha256: None,
            },
            &config,
            &policy,
        )
        .unwrap();
        let staging = upload.staging_path.clone();
        context.active_uploads.insert(upload.transfer_id, upload);
        assert!(staging.exists());
        let audit_path = root.join("cleanup-audit.jsonl");
        let result = std::panic::catch_unwind(move || {
            let sink = Arc::new(JsonlAuditSink::open(audit_path).unwrap());
            let _context = UploadCleanup {
                context,
                sink,
                shutdown: Default::default(),
                audit_context: Default::default(),
            };
            panic!("dispatch error");
        });
        assert!(result.is_err());
        assert!(!staging.exists());
        assert!(!root.join("incomplete.txt").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn real_listener_drains_and_persists_shutdown_completion() {
        let root = temp_root();
        let config = config_for(root.clone());
        let state = WsServerState::new(
            config.clone(),
            InMemorySessionStore::new(config.session.clone()),
            InProcessHelper {
                roots: config.roots.clone(),
            },
            JsonlAuditSink::open(root.join("audit.jsonl")).unwrap(),
        );
        let shutdown = state.shutdown.clone();
        let work = shutdown.enter().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(run_server_with_shutdown(
            listener,
            state,
            async move {
                rx.await.unwrap();
            },
            Duration::from_millis(10),
        ));
        let response = tokio::task::spawn_blocking(move || {
            use std::io::{Read, Write};
            let mut socket = std::net::TcpStream::connect(address).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            socket
                .write_all(
                    b"GET /api/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
            let mut response = String::new();
            socket.read_to_string(&mut response).unwrap();
            response
        })
        .await
        .unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        tx.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(1), shutdown.cancellation().cancelled())
            .await
            .unwrap();
        assert!(shutdown.enter().is_none());
        drop(work);
        tokio::time::timeout(Duration::from_secs(3), server)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let events = fs::read_to_string(root.join("audit.jsonl")).unwrap();
        let event: serde_json::Value =
            serde_json::from_str(events.lines().last().unwrap()).unwrap();
        assert_eq!(event["action"], "ServerShutdown");
        assert_eq!(event["status"], "Success");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn shutdown_notification_is_a_backward_compatible_connection_status() {
        let frame = shutdown_notification_frame().unwrap();
        assert_eq!(frame.header.message_type, MessageType::ConnectionStatus);
        assert!(frame.header.message_type.validate_inbound_client().is_err());
        let payload: crate::protocol::payload::ConnectionStatusPayload =
            crate::protocol::codec::decode_msgpack(&frame.payload).unwrap();
        assert!(payload.shutting_down);
        assert!(!payload.protocol_ready);
        assert_eq!(payload.grace_period_seconds, Some(30));
        let legacy = crate::protocol::codec::encode_msgpack(&serde_json::json!({
            "protocol_ready": true, "message": "connected"
        }))
        .unwrap();
        let legacy: crate::protocol::payload::ConnectionStatusPayload =
            crate::protocol::codec::decode_msgpack(&legacy).unwrap();
        assert!(!legacy.shutting_down);
        assert_eq!(legacy.grace_period_seconds, None);
    }
}
