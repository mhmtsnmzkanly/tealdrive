//! Own-account history: the protected helper derives account scope and current path visibility.
use crate::{
    accounts::journal::HistoryQuery,
    helper::account_gateway::{AccountCommand, AccountReply, GatewayFailure},
    session::store::{InMemorySessionStore, SessionLookupResult, SessionStore},
    ws::server::{extract_session_id, WsHelperBackend, WsServerState},
};
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
pub async fn history_handler<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
    Query(query): Query<HistoryQuery>,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    if query.validate().is_err() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"code":"INVALID_ARGUMENT"})),
        )
            .into_response();
    }
    let config = state.config();
    if !config.web_accounts.enabled || !config.journal.enabled {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"code":"FEATURE_DISABLED"})),
        )
            .into_response();
    }
    let Ok(id) = extract_session_id(&headers, &config.session.cookie_name) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let SessionLookupResult::Found(session) = state
        .session_store
        .lock()
        .await
        .get_session(&id, InMemorySessionStore::now_epoch_seconds())
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !session.allowed_by(&config) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Some(grant) = session.grant.as_deref().cloned() else {
        return StatusCode::FORBIDDEN.into_response();
    };
    let Some(activity) = state.shutdown.enter() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Ok(permit) = state.operation_slots.clone().acquire_owned().await else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let auth = state.authenticator.clone();
    let cancellation = state.shutdown.cancellation();
    let result = tokio::task::spawn_blocking(move || {
        let _activity = activity;
        let _permit = permit;
        crate::session::lease::with_cancellation(cancellation, || {
            auth.account_command(AccountCommand::History { grant, query })
        })
    })
    .await;
    match result {
        Ok(Ok(AccountReply::History(page))) => Json(page).into_response(),
        Ok(Err(GatewayFailure::Disabled)) => (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"code":"FEATURE_DISABLED"})),
        )
            .into_response(),
        Ok(Err(GatewayFailure::Forbidden)) => StatusCode::FORBIDDEN.into_response(),
        _ => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
