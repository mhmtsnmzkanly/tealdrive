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
pub async fn list_system<H>(State(state): State<WsServerState<H>>, headers: HeaderMap) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    dispatch(state, headers, Some(AccountKind::Linux), None).await
}
pub async fn list_virtual<H>(State(state): State<WsServerState<H>>, headers: HeaderMap) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    dispatch(state, headers, Some(AccountKind::Virtual), None).await
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
        Some(AccountKind::Linux),
        Some((payload.account_id, true)),
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
        Some(AccountKind::Linux),
        Some((payload.account_id, false)),
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
        Some(AccountKind::Virtual),
        Some((payload.account_id, true)),
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
        Some(AccountKind::Virtual),
        Some((payload.account_id, false)),
    )
    .await
}
async fn dispatch<H>(
    state: WsServerState<H>,
    headers: HeaderMap,
    kind: Option<AccountKind>,
    mutation: Option<(String, bool)>,
) -> Response
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
    if mutation.is_some() && !crate::session::csrf::validate_header(&session, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(grant) = session.grant.as_deref().cloned() else {
        return StatusCode::FORBIDDEN.into_response();
    };
    let command = match (kind, mutation) {
        (Some(kind), None) => AccountCommand::ListAccounts { grant, kind },
        (Some(kind), Some((account_id, enabled))) => AccountCommand::SetAccountAccess {
            grant,
            account_id,
            kind,
            enabled,
        },
        _ => return StatusCode::BAD_REQUEST.into_response(),
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
        Ok(Ok(AccountReply::AccountUpdated(account))) => Json(account).into_response(),
        Ok(Err(GatewayFailure::Forbidden)) => StatusCode::FORBIDDEN.into_response(),
        Ok(Err(GatewayFailure::InvalidCredentials)) => StatusCode::UNAUTHORIZED.into_response(),
        Ok(Err(GatewayFailure::Conflict)) => StatusCode::CONFLICT.into_response(),
        _ => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
