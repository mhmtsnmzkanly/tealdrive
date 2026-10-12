use crate::session::store::{InMemorySessionStore, SessionLookupResult, SessionStore};
use crate::ws::server::{extract_session_id, WsHelperBackend, WsServerState};
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
#[derive(Deserialize)]
pub struct UsageQuery {
    pub root_id: Option<String>,
}
pub async fn usage_handler<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
    Query(query): Query<UsageQuery>,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    let config = state.config();
    let now = InMemorySessionStore::now_epoch_seconds();
    let Ok(id) = extract_session_id(&headers, &config.session.cookie_name) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let SessionLookupResult::Found(session) =
        state.session_store.lock().await.get_session(&id, now)
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !session.allowed_by(&config) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let account = match crate::auth::api::validated_session_account(&state, &session).await {
        Ok(account) => account,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };
    let native = account
        .as_ref()
        .is_some_and(|account| account.kind == crate::accounts::store::AccountKind::Virtual)
        && config.virtual_accounts.enforcement == crate::accounts::virtuals::Enforcement::UserQuota;
    let config = std::sync::Arc::new(match account.as_ref() {
        Some(account) => {
            match crate::policy::home::resolve(&config, account, config.home_template.as_ref()) {
                Ok(config) => config,
                Err(_) => return StatusCode::FORBIDDEN.into_response(),
            }
        }
        None if config.home_template.is_none() => config.as_ref().clone(),
        _ => return StatusCode::FORBIDDEN.into_response(),
    });
    let root_id = query.root_id.unwrap_or_else(|| {
        config
            .roots
            .first()
            .map(|r| r.root_id.0.clone())
            .unwrap_or_default()
    });
    if !config.roots.iter().any(|r| r.root_id.0 == root_id) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Some(activity) = state.shutdown.enter() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Ok(permit) = state.operation_slots.clone().acquire_owned().await else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let lease = crate::session::lease::SessionLease::with_shutdown_for_session(
        state.session_store.clone(),
        id,
        state.shutdown.cancellation(),
        state.authenticator.clone(),
    )
    .await;
    let cancellation = lease.cancellation.clone();
    let helper = state.helper_client.clone();
    let cfg = config.clone();
    let result = tokio::task::spawn_blocking(move || {
        let _activity = activity;
        let _permit = permit;
        let request = crate::helper::types::HelperRequest {
            request_id: crate::protocol::RequestId::new(),
            username: session.username,
            uid: session.uid,
            gid: session.gid,
            command: crate::helper::types::HelperCommand::QuotaUsage,
            root_id: crate::config::RootId::new(root_id),
            relative_path: crate::config::RelativePath::new(""),
            target_relative_path: None,
            policy_context: crate::helper::types::HelperPolicyContext {
                read_only_root: false,
                is_web_root: false,
            },
            limits: crate::helper::types::HelperLimits {
                max_directory_page_size: cfg.limits.max_directory_page_size,
                max_text_edit_size: cfg.limits.max_text_edit_size,
                max_chunk_size: cfg.limits.max_chunk_size,
                max_download_helper_bytes: crate::download::transfer::MAX_DOWNLOAD_HELPER_BYTES,
            },
            list_options: None,
            write_options: None,
            chmod_mode: None,
            archive_source_paths: None,
            search_options: None,
        };
        crate::session::lease::with_cancellation(cancellation, || {
            crate::helper::policy::with_snapshot(&cfg, || {
                crate::fs::metadata::MetadataHelper::execute_helper(&*helper, &request)
            })
        })
    })
    .await;
    let Ok(Ok(response)) = result else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"code":"QUOTA_MEASUREMENT_UNAVAILABLE"})),
        )
            .into_response();
    };
    let crate::helper::types::HelperResponsePayload::Usage {
        used_bytes,
        limit_bytes,
    } = response.payload
    else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"code":"QUOTA_MEASUREMENT_UNAVAILABLE"})),
        )
            .into_response();
    };
    let percent = limit_bytes
        .filter(|limit| *limit > 0)
        .map(|limit| used_bytes as f64 * 100.0 / limit as f64);
    let mut response=Json(serde_json::json!({"used_bytes":used_bytes,"limit_bytes":limit_bytes,"usage_percent":percent,"warning_percent":config.quota_warning_percent,"measured_at":now,"measurement":if native{"allocated_bytes"}else{"logical_bytes"},"source":"helper","stale":false})).into_response();
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
}
