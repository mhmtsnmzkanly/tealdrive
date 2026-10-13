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
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreferencesUpdate {
    pub hours: Option<u64>,
    pub expected_revision: u64,
}
pub async fn preferences_handler<H: WsHelperBackend + Clone + 'static>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
) -> Response {
    preferences(state, headers, OwnCommand::GetPreferences).await
}
pub async fn set_preferences_handler<H: WsHelperBackend + Clone + 'static>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
    Json(update): Json<PreferencesUpdate>,
) -> Response {
    if update
        .hours
        .is_some_and(|h| !(1..=state.config().journal.undo_max_hours).contains(&h))
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"code":"INVALID_ARGUMENT"})),
        )
            .into_response();
    }
    preferences(state, headers, OwnCommand::SetPreferences(update)).await
}
async fn preferences<H: WsHelperBackend + Clone + 'static>(
    state: WsServerState<H>,
    headers: HeaderMap,
    update: OwnCommand,
) -> Response {
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
    let command = match update {
        OwnCommand::SetPreferences(update) => AccountCommand::SetUndoPreferences {
            grant,
            hours: update.hours,
            expected_revision: update.expected_revision,
        },
        OwnCommand::GetPreferences => AccountCommand::UndoPreferences { grant },
        OwnCommand::ApplyUndo(update) => AccountCommand::ApplyUndo {
            grant,
            operation_id: update.operation_id,
            expected_state: update.expected_state,
        },
    };
    match tokio::task::spawn_blocking(move || {
        let _activity = activity;
        let _permit = permit;
        crate::session::lease::with_cancellation(cancellation, || auth.account_command(command))
    })
    .await
    {
        Ok(Ok(AccountReply::UndoPreferences(view))) => Json(view).into_response(),
        Ok(Ok(AccountReply::UndoApplied(response))) => {
            if response.success {
                Json(serde_json::json!({"success":true,"message":response.safe_message}))
                    .into_response()
            } else {
                (StatusCode::CONFLICT, Json(serde_json::json!({"code":format!("{:?}",response.error_kind.unwrap_or(crate::errors::ErrorKind::InternalError)),"message":response.safe_message}))).into_response()
            }
        }
        Ok(Err(GatewayFailure::Conflict)) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"code":"CONFLICT"})),
        )
            .into_response(),
        Ok(Err(GatewayFailure::Disabled)) => (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"code":"FEATURE_DISABLED"})),
        )
            .into_response(),
        Ok(Err(GatewayFailure::Forbidden)) => StatusCode::FORBIDDEN.into_response(),
        Ok(Err(GatewayFailure::InvalidCredentials)) => StatusCode::UNAUTHORIZED.into_response(),
        _ => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
pub async fn history_handler<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
    Query(query): Query<HistoryQuery>,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    history(state, headers, query, false).await
}
pub async fn undo_list_handler<H: WsHelperBackend + Clone + 'static>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
    Query(query): Query<HistoryQuery>,
) -> Response {
    history(state, headers, query, true).await
}
async fn history<H: WsHelperBackend + Clone + 'static>(
    state: WsServerState<H>,
    headers: HeaderMap,
    query: HistoryQuery,
    undo_only: bool,
) -> Response {
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
            auth.account_command(AccountCommand::History {
                grant,
                query,
                undo_only,
            })
        })
    })
    .await;
    match result {
        Ok(Ok(AccountReply::History(page))) => Json(page).into_response(),
        Ok(Ok(AccountReply::UndoPage {
            items,
            next_cursor,
            has_more,
        })) => Json(serde_json::json!({"items":items,"nextCursor":next_cursor,"hasMore":has_more}))
            .into_response(),
        Ok(Err(GatewayFailure::Disabled)) => (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"code":"FEATURE_DISABLED"})),
        )
            .into_response(),
        Ok(Err(GatewayFailure::Forbidden)) => StatusCode::FORBIDDEN.into_response(),
        _ => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UndoApply {
    pub operation_id: String,
    pub expected_state: String,
}
enum OwnCommand {
    GetPreferences,
    SetPreferences(PreferencesUpdate),
    ApplyUndo(UndoApply),
}
pub async fn undo_apply_handler<H: WsHelperBackend + Clone + 'static>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
    Json(update): Json<UndoApply>,
) -> Response {
    if uuid::Uuid::parse_str(&update.operation_id).is_err()
        || update.expected_state.len() != 64
        || !update.expected_state.bytes().all(|c| c.is_ascii_hexdigit())
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"code":"INVALID_ARGUMENT"})),
        )
            .into_response();
    }
    preferences(state, headers, OwnCommand::ApplyUndo(update)).await
}
