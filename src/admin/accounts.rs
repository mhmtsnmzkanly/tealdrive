//! Web account metadata management. All authority is derived again by the root helper.
use crate::{
    accounts::store::AccountKind,
    helper::account_gateway::{AccountCommand, AccountReply, GatewayFailure},
    session::store::{InMemorySessionStore, SessionLookupResult, SessionStore},
    ws::server::{extract_session_id, WsHelperBackend, WsServerState},
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccessPayload {
    account_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalPayload {
    username: String,
}
enum Action {
    List(AccountKind),
    Access(AccountKind, String, bool),
    Approve(String),
}
pub async fn list_system<H>(State(state): State<WsServerState<H>>, headers: HeaderMap) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    dispatch(state, headers, Action::List(AccountKind::Linux)).await
}
pub async fn list_virtual<H>(State(state): State<WsServerState<H>>, headers: HeaderMap) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    dispatch(state, headers, Action::List(AccountKind::Virtual)).await
}
pub async fn enable<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
    Json(payload): Json<AccessPayload>,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    dispatch(
        state,
        headers,
        Action::Access(AccountKind::Linux, payload.account_id, true),
    )
    .await
}
pub async fn disable<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
    Json(payload): Json<AccessPayload>,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    dispatch(
        state,
        headers,
        Action::Access(AccountKind::Linux, payload.account_id, false),
    )
    .await
}
pub async fn enable_virtual<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
    Json(payload): Json<AccessPayload>,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    dispatch(
        state,
        headers,
        Action::Access(AccountKind::Virtual, payload.account_id, true),
    )
    .await
}
pub async fn disable_virtual<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
    Json(payload): Json<AccessPayload>,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    dispatch(
        state,
        headers,
        Action::Access(AccountKind::Virtual, payload.account_id, false),
    )
    .await
}
pub async fn approve<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
    Json(payload): Json<ApprovalPayload>,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    dispatch(state, headers, Action::Approve(payload.username)).await
}
async fn dispatch<H>(state: WsServerState<H>, headers: HeaderMap, action: Action) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
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
            SessionLookupResult::Found(s) => s,
            _ => return StatusCode::UNAUTHORIZED.into_response(),
        }
    };
    if !session.allowed_by(&cfg) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if !matches!(action, Action::List(_))
        && !crate::session::csrf::validate_header(&session, &headers)
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(grant) = session.grant.as_deref().cloned() else {
        return StatusCode::FORBIDDEN.into_response();
    };
    let operation = match &action {
        Action::List(_) => None,
        Action::Approve(_) => Some("approve_linux_account"),
        Action::Access(_, _, true) => Some("enable_web_access"),
        Action::Access(_, _, false) => Some("suspend_web_access"),
    };
    let command = match action {
        Action::List(kind) => AccountCommand::ListAccounts { grant, kind },
        Action::Access(kind, account_id, enabled) => AccountCommand::SetAccountAccess {
            grant,
            account_id,
            kind,
            enabled,
        },
        Action::Approve(username) => {
            if username.is_empty()
                || username.len() > 256
                || username
                    .chars()
                    .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '/' | '\\' | ':'))
            {
                return StatusCode::BAD_REQUEST.into_response();
            }
            AccountCommand::ApproveLinux { grant, username }
        }
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
    let auth = state.authenticator.clone();
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let _activity = activity;
        auth.account_command(command)
    })
    .await
    {
        Ok(Ok(AccountReply::Accounts(accounts))) => Json(accounts).into_response(),
        Ok(Ok(AccountReply::AccountUpdated(account))) => {
            if let Some(operation) = operation {
                let mut event = crate::audit::event::AuditEvent::new(
                    crate::protocol::frame::RequestId::new(),
                    &session.username,
                    session.uid,
                    session.gid,
                    match operation {
                        "approve_linux_account" => {
                            crate::audit::event::AuditAction::AccountApproved
                        }
                        "enable_web_access" => crate::audit::event::AuditAction::AccountEnabled,
                        _ => crate::audit::event::AuditAction::AccountSuspended,
                    },
                    crate::audit::event::AuditStatus::Success,
                );
                crate::audit::context::current().enrich(&mut event);
                event.session_id_hash = crate::audit::context::session_hash(session.session_id);
                event.operation = Some(operation.into());
                event.target_account_id = Some(account.id.clone());
                event.target_username = Some(account.username.clone());
                let _ = state.audit_sink.emit(&event);
            }
            Json(account).into_response()
        }
        Ok(Err(GatewayFailure::Forbidden)) => StatusCode::FORBIDDEN.into_response(),
        Ok(Err(GatewayFailure::InvalidCredentials)) => StatusCode::UNAUTHORIZED.into_response(),
        Ok(Err(GatewayFailure::Conflict)) => StatusCode::CONFLICT.into_response(),
        _ => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
