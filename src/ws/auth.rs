//! Bounded WS-first authentication. Files are never dispatched before identity verification.
use crate::protocol::codec::{decode_msgpack, encode_msgpack};
use crate::protocol::{MessageType, TdrvFrame, TdrvHeader};
use crate::session::store::{InMemorySessionStore, SessionId, SessionLookupResult, SessionStore};
use crate::ws::server::{extract_client_ip, extract_session_id, WsHelperBackend, WsServerState};
use axum::extract::{
    ws::{Message, WebSocket, WebSocketUpgrade},
    State,
};
use axum::http::{
    header::{COOKIE, HOST, ORIGIN, SET_COOKIE},
    HeaderMap, StatusCode,
};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct CookieBridge {
    pub session: SessionId,
    pub expires: u64,
    pub ip: Option<String>,
}
#[derive(Deserialize)]
pub struct BindPayload {
    pub token: String,
}

fn origin_allowed<H>(state: &WsServerState<H>, headers: &HeaderMap) -> bool {
    let cfg = state.config();
    let origin = headers.get(ORIGIN).and_then(|v| v.to_str().ok());
    if crate::ws::upgrade::validate_origin(&cfg.security, origin).is_err() {
        return false;
    }
    if cfg.security.allowed_origins.is_empty() {
        let Some(host) = headers.get(HOST).and_then(|v| v.to_str().ok()) else {
            return false;
        };
        let scheme = if cfg.server.behind_tls_proxy {
            "https"
        } else {
            "http"
        };
        return origin == Some(format!("{scheme}://{host}").as_str());
    }
    true
}

pub async fn bind_cookie<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
    Json(payload): Json<BindPayload>,
) -> Response {
    if !origin_allowed(&state, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let bridge = state.auth_bridges.lock().await.remove(&payload.token);
    let now = InMemorySessionStore::now_epoch_seconds();
    let Some(bridge) = bridge else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if bridge.expires <= now || bridge.ip != extract_client_ip(&headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let session = state
        .session_store
        .lock()
        .await
        .get_session(&bridge.session, now);
    let SessionLookupResult::Found(session) = session else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let cfg = state.config();
    if !cfg.account_policy.allows_login(&session.username) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let cookie = crate::session::cookie::SessionCookieConfig::from_session_config(&cfg.session)
        .build_set_cookie_value(&bridge.session);
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(SET_COOKIE, cookie.parse().unwrap());
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
}

pub async fn upgrade<H>(
    ws: WebSocketUpgrade,
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    if !origin_allowed(&state, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Ok(permit) = state.pre_auth_slots.clone().try_acquire_owned() else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };
    let Some(activity) = state.shutdown.enter() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let audit = crate::audit::context::current();
    ws.max_message_size(
        state.config().limits.max_frame_payload + crate::protocol::header::HEADER_LEN,
    )
    .max_frame_size(state.config().limits.max_frame_payload + crate::protocol::header::HEADER_LEN)
    .on_upgrade(move |socket| async move {
        let _activity = activity;
        let _ = crate::audit::context::REQUEST
            .scope(audit, async move {
                let _ = authenticate_socket(socket, state, headers, permit).await;
            })
            .await;
    })
    .into_response()
}

async fn send<T: Serialize>(
    socket: &mut WebSocket,
    kind: MessageType,
    id: crate::protocol::RequestId,
    value: &T,
) -> Result<(), crate::errors::TealDriveError> {
    let frame = TdrvFrame::new(TdrvHeader::new(kind, id, 0), encode_msgpack(value)?)?;
    socket
        .send(Message::Binary(frame.encode()?))
        .await
        .map_err(|_| crate::errors::TealDriveError::Internal)
}

async fn authenticate_socket<H>(
    mut socket: WebSocket,
    state: WsServerState<H>,
    mut headers: HeaderMap,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> Result<(), crate::errors::TealDriveError>
where
    H: WsHelperBackend + Clone + 'static,
{
    use crate::errors::TealDriveError;
    let cfg = state.config();
    let hello = crate::ws::handshake::create_server_hello_frame(&cfg)?;
    socket
        .send(Message::Binary(hello.encode()?))
        .await
        .map_err(|_| TealDriveError::Internal)?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let mut selected = None;
    let mut attempts = 0;
    loop {
        let message = tokio::select! {
            _=state.shutdown.draining()=>return Ok(()),
            message=tokio::time::timeout_at(deadline,socket.recv())=>message.map_err(|_|TealDriveError::Internal)?,
        };
        let Some(Ok(Message::Binary(bytes))) = message else {
            return Ok(());
        };
        if bytes.len() > 65536 {
            return Err(TealDriveError::HelperRequestTooLarge);
        }
        let bytes = zeroize::Zeroizing::new(bytes);
        let mut frame = TdrvFrame::decode(&bytes)?;
        let id = frame.header.request_id;
        match frame.header.message_type {
            MessageType::ClientHello if selected.is_none() => {
                let request = crate::ws::handshake::decode_client_hello(&frame)?;
                let protocol =
                    crate::ws::handshake::validate_client_hello(&request, &state.config().limits)?;
                let mut ready = crate::ws::handshake::create_protocol_ready_frame(&protocol)?;
                ready.header.request_id = id;
                socket
                    .send(Message::Binary(ready.encode()?))
                    .await
                    .map_err(|_| TealDriveError::Internal)?;
                selected = Some(protocol);
                // Resume an existing HttpOnly session only after protocol negotiation.
                if extract_session_id(&headers, &cfg.session.cookie_name).is_ok() {
                    if let Some(context) =
                        authenticated_context(&state, &headers, selected.as_ref().unwrap()).await
                    {
                        send(
                            &mut socket,
                            MessageType::AuthResult,
                            id,
                            &serde_json::json!({"authenticated":true}),
                        )
                        .await?;
                        drop(permit);
                        return authenticated_socket(socket, state, context).await;
                    }
                }
            }
            MessageType::AuthLogin if selected.is_some() => {
                attempts += 1;
                if attempts > 5 {
                    return Ok(());
                }
                let mut request: crate::protocol::payload::AuthLoginRequest =
                    decode_msgpack(&frame.payload)?;
                use zeroize::Zeroize;
                frame.payload.zeroize();
                let response = crate::auth::api::login_handler(
                    State(state.clone()),
                    headers.clone(),
                    Json(crate::auth::api::LoginPayload {
                        username: request.username.clone(),
                        password: std::mem::take(&mut request.password),
                    }),
                )
                .await;
                let status = response.status();
                let response_headers = response.headers().clone();
                let body = axum::body::to_bytes(response.into_body(), 65536)
                    .await
                    .map_err(|_| TealDriveError::Internal)?;
                let body: serde_json::Value =
                    serde_json::from_slice(&body).map_err(|_| TealDriveError::Internal)?;
                if !status.is_success() {
                    send(&mut socket,MessageType::AuthResult,id,&serde_json::json!({"authenticated":false,"code":body["code"],"status":status.as_u16()})).await?;
                    continue;
                }
                let cookie = response_headers
                    .get(SET_COOKIE)
                    .ok_or(TealDriveError::Internal)?
                    .clone();
                headers.insert(COOKIE, cookie);
                let session = extract_session_id(&headers, &cfg.session.cookie_name)?;
                let context = authenticated_context(&state, &headers, selected.as_ref().unwrap())
                    .await
                    .ok_or(TealDriveError::PermissionDenied)?;
                let token = uuid::Uuid::new_v4().to_string();
                let now = InMemorySessionStore::now_epoch_seconds();
                {
                    let mut bridges = state.auth_bridges.lock().await;
                    bridges.retain(|_, bridge| bridge.expires > now);
                    if bridges.len() >= 256 {
                        return Err(TealDriveError::Internal);
                    }
                    bridges.insert(
                        token.clone(),
                        CookieBridge {
                            session,
                            expires: now + 30,
                            ip: extract_client_ip(&headers),
                        },
                    );
                }
                send(
                    &mut socket,
                    MessageType::AuthResult,
                    id,
                    &serde_json::json!({"authenticated":true,"session":body,"bind_token":token}),
                )
                .await?;
                drop(permit);
                return authenticated_socket(socket, state, context).await;
            }
            _ => {
                send(
                    &mut socket,
                    MessageType::AuthResult,
                    id,
                    &serde_json::json!({"authenticated":false,"code":"AUTH_REQUIRED","status":401}),
                )
                .await?;
            }
        }
    }
}

async fn authenticated_context<H>(
    state: &WsServerState<H>,
    headers: &HeaderMap,
    selected: &crate::ws::connection::WsSelectedProtocol,
) -> Option<crate::ws::connection::WsConnectionContext> {
    let cfg = state.config();
    let session = extract_session_id(headers, &cfg.session.cookie_name).ok()?;
    let request = crate::ws::upgrade::WsUpgradeRequest {
        session_id: Some(session),
        origin: headers.get(ORIGIN)?.to_str().ok().map(str::to_owned),
        user_agent: headers
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned),
        ip: extract_client_ip(headers),
        now: InMemorySessionStore::now_epoch_seconds(),
    };
    let store = state.session_store.lock().await;
    let mut context = crate::ws::upgrade::accept_session_bound_upgrade(
        &*store,
        &cfg.security,
        cfg.features.clone(),
        request,
    )
    .ok()?;
    if !cfg.account_policy.allows_login(&context.username) {
        return None;
    }
    context.protocol_ready = true;
    context.selected_protocol = Some(selected.clone());
    Some(context)
}

async fn authenticated_socket<H>(
    socket: WebSocket,
    state: WsServerState<H>,
    context: crate::ws::connection::WsConnectionContext,
) -> Result<(), crate::errors::TealDriveError>
where
    H: WsHelperBackend + 'static,
{
    let mut audit = crate::audit::context::current();
    audit.request_id = None;
    audit.session_id = Some(context.session_id);
    audit.user = Some(context.user_context());
    crate::audit::context::REQUEST
        .scope(
            audit,
            crate::ws::server::handle_socket_connection(socket, state, context),
        )
        .await
}
