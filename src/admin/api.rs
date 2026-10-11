use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::audit::event::{AuditEvent, AuditStatus};
use crate::config::ConfigPatch;
use crate::protocol::frame::RequestId;
use crate::session::store::{InMemorySessionStore, SessionLookupResult, SessionStore};
use crate::ws::server::{extract_session_id, WsHelperBackend, WsServerState};

#[derive(Debug, Deserialize, Default)]
pub struct PatchConfigQuery {
    #[serde(default)]
    pub save: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AdminErrorResponse {
    pub error: String,
    pub code: String,
}

pub async fn get_config_handler<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    let now = InMemorySessionStore::now_epoch_seconds();
    let session_id = match extract_session_id(&headers, &state.config.load().session.cookie_name) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(AdminErrorResponse {
                    error: "Unauthorized".to_owned(),
                    code: "unauthorized".to_owned(),
                }),
            )
                .into_response();
        }
    };

    let lookup = {
        let store = state.session_store.lock().await;
        store.get_session(&session_id, now)
    };

    let session = match lookup {
        SessionLookupResult::Found(s) => s,
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(AdminErrorResponse {
                    error: "Unauthorized".to_owned(),
                    code: "unauthorized".to_owned(),
                }),
            )
                .into_response();
        }
    };

    let cfg = state.config.load();
    if !cfg.account_policy.is_admin(&session.username) {
        return (
            StatusCode::FORBIDDEN,
            Json(AdminErrorResponse {
                error: "Forbidden: Admin privileges required".to_owned(),
                code: "forbidden".to_owned(),
            }),
        )
            .into_response();
    }

    let config = state.config.load_full();
    Json((*config).clone()).into_response()
}

pub async fn patch_config_handler<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
    Query(query): Query<PatchConfigQuery>,
    Json(patch): Json<ConfigPatch>,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    let now = InMemorySessionStore::now_epoch_seconds();
    let session_id = match extract_session_id(&headers, &state.config.load().session.cookie_name) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(AdminErrorResponse {
                    error: "Unauthorized".to_owned(),
                    code: "unauthorized".to_owned(),
                }),
            )
                .into_response();
        }
    };

    let session = {
        let store = state.session_store.lock().await;
        match store.get_session(&session_id, now) {
            SessionLookupResult::Found(s) => s,
            _ => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(AdminErrorResponse {
                        error: "Unauthorized".to_owned(),
                        code: "unauthorized".to_owned(),
                    }),
                )
                    .into_response();
            }
        }
    };

    let cfg = state.config.load();
    if !cfg.account_policy.is_admin(&session.username) {
        return (
            StatusCode::FORBIDDEN,
            Json(AdminErrorResponse {
                error: "Forbidden: Admin privileges required".to_owned(),
                code: "forbidden".to_owned(),
            }),
        )
            .into_response();
    }

    if !crate::session::csrf::validate_header(&session, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    match state.config.apply_patch_saved(patch, query.save) {
        Ok(updated) => {
            let _ = state.audit_sink.emit(&AuditEvent::config_patch(
                RequestId::new(),
                session.username,
                session.uid,
                session.gid,
                AuditStatus::Success,
            ));

            // Sync live session store, helper timeout, and rate limiter
            {
                let mut store = state.session_store.lock().await;
                store.update_config(updated.session.clone());
            }
            state.set_helper_timeout(std::time::Duration::from_secs(
                updated.helper.timeout_seconds,
            ));
            {
                let mut limiter = state.rate_limiter.lock().await;
                limiter.set_limits(
                    updated.rate_limit.max_login_failures,
                    updated.rate_limit.lockout_duration_seconds,
                );
            }

            Json((*updated).clone()).into_response()
        }
        Err(err) => {
            let _ = state.audit_sink.emit(&AuditEvent::config_patch(
                RequestId::new(),
                session.username,
                session.uid,
                session.gid,
                AuditStatus::Failed,
            ));
            (
                if err == crate::errors::TealDriveError::Validation {
                    StatusCode::BAD_REQUEST
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                },
                Json(AdminErrorResponse {
                    error: if err == crate::errors::TealDriveError::Validation {
                        "Invalid configuration."
                    } else {
                        "Configuration could not be applied or persisted. Check the log path and file permissions."
                    }
                    .to_owned(),
                    code: if err == crate::errors::TealDriveError::Validation {
                        "invalid_config"
                    } else {
                        "config_save_failed"
                    }
                    .to_owned(),
                }),
            )
                .into_response()
        }
    }
}

pub async fn reload_config_handler<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    let now = InMemorySessionStore::now_epoch_seconds();
    let session_id = match extract_session_id(&headers, &state.config.load().session.cookie_name) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(AdminErrorResponse {
                    error: "Unauthorized".to_owned(),
                    code: "unauthorized".to_owned(),
                }),
            )
                .into_response();
        }
    };

    let session = {
        let store = state.session_store.lock().await;
        match store.get_session(&session_id, now) {
            SessionLookupResult::Found(s) => s,
            _ => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(AdminErrorResponse {
                        error: "Unauthorized".to_owned(),
                        code: "unauthorized".to_owned(),
                    }),
                )
                    .into_response();
            }
        }
    };

    let cfg = state.config.load();
    if !cfg.account_policy.is_admin(&session.username) {
        return (
            StatusCode::FORBIDDEN,
            Json(AdminErrorResponse {
                error: "Forbidden: Admin privileges required".to_owned(),
                code: "forbidden".to_owned(),
            }),
        )
            .into_response();
    }

    if !crate::session::csrf::validate_header(&session, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    match state.config.reload() {
        Ok(reloaded) => {
            let _ = state.audit_sink.emit(&AuditEvent::config_reload(
                RequestId::new(),
                session.username,
                session.uid,
                session.gid,
                AuditStatus::Success,
            ));

            // Sync live session store, helper timeout, and rate limiter
            {
                let mut store = state.session_store.lock().await;
                store.update_config(reloaded.session.clone());
            }
            state.set_helper_timeout(std::time::Duration::from_secs(
                reloaded.helper.timeout_seconds,
            ));
            {
                let mut limiter = state.rate_limiter.lock().await;
                limiter.set_limits(
                    reloaded.rate_limit.max_login_failures,
                    reloaded.rate_limit.lockout_duration_seconds,
                );
            }

            Json((*reloaded).clone()).into_response()
        }
        Err(err) => {
            let _ = state.audit_sink.emit(&AuditEvent::config_reload(
                RequestId::new(),
                session.username,
                session.uid,
                session.gid,
                AuditStatus::Failed,
            ));
            (
                StatusCode::BAD_REQUEST,
                Json(AdminErrorResponse {
                    error: format!("Failed to reload config: {err}"),
                    code: "reload_failed".to_owned(),
                }),
            )
                .into_response()
        }
    }
}

pub async fn get_users_handler<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    let authorization = get_config_handler(State(state.clone()), headers).await;
    if authorization.status() != StatusCode::OK {
        return authorization;
    }
    let minimum_uid = state.config.load().account_policy.minimum_regular_uid;
    let Ok(permit) = state.operation_slots.clone().acquire_owned().await else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Some(activity) = state.shutdown.enter() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let _activity = activity;
        crate::admin::users::discover(minimum_uid)
    })
    .await
    {
        Ok(Ok(users)) => {
            let mut response = Json(users).into_response();
            response.headers_mut().insert(
                axum::http::header::CACHE_CONTROL,
                "no-store".parse().unwrap(),
            );
            response
        }
        _ => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(AdminErrorResponse {
                error: "Unable to discover local users".into(),
                code: "USER_DISCOVERY_UNAVAILABLE".into(),
            }),
        )
            .into_response(),
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
    use crate::policy::feature_gate::FeatureFlags;
    use crate::rate_limit::login::InMemoryLoginRateLimiter;
    use crate::session::store::{InMemorySessionStore, SessionCreateInput};
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

    async fn test_state_with_session(
        config_path: Option<std::path::PathBuf>,
    ) -> (WsServerState<DummyHelper>, HeaderMap) {
        let auth = MockAuthenticator::new(AccountPolicyConfig {
            allowed_users: vec!["admin".into()],
            ..Default::default()
        })
        .with_user(MockUser::regular("admin", "secret123"));
        let config = AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: std::path::PathBuf::from("/home"),
                read_only: false,
                uploads_allowed: true,
                hidden_files_allowed: false,
                is_web_root: false,
                quota_bytes: None,
            }],
            account_policy: AccountPolicyConfig {
                admin_users: vec!["admin".into()],
                allowed_users: vec!["admin".into()],
                ..Default::default()
            },
            ..AppConfig::default()
        };
        let session_store = InMemorySessionStore::new(config.session.clone());
        let audit_sink = JsonlAuditSink::open_default().expect("sink");
        let state = WsServerState::with_authenticator_and_path(
            config,
            config_path,
            session_store,
            DummyHelper,
            audit_sink,
            Arc::new(auth),
            InMemoryLoginRateLimiter::new(5),
        );

        let session = {
            let mut store = state.session_store.lock().await;
            store.create_session(
                SessionCreateInput {
                    username: "admin".to_owned(),
                    uid: 1000,
                    gid: 1000,
                    user_agent: None,
                    ip: None,
                },
                InMemorySessionStore::now_epoch_seconds(),
            )
        };

        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            format!("tealdrive_session={}", session.session_id.0)
                .parse()
                .unwrap(),
        );

        headers.insert("x-csrf-token", session.csrf_token.0.parse().unwrap());
        (state, headers)
    }

    #[tokio::test]
    async fn test_get_config_unauthorized() {
        let (state, _) = test_state_with_session(None).await;
        let response = get_config_handler(State(state), HeaderMap::new()).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_get_config_authorized() {
        let (state, headers) = test_state_with_session(None).await;
        let response = get_config_handler(State(state), headers).await;
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = to_bytes(response.into_body(), 8192).await.unwrap();
        let cfg: AppConfig = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(cfg.roots.len(), 1);
        assert_eq!(cfg.roots[0].root_id.0, "home");
    }

    #[tokio::test]
    async fn test_patch_and_reload_config() {
        let temp_dir =
            std::env::temp_dir().join(format!("tdrv-admin-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let config_file = temp_dir.join("config.toml");

        let initial_config = AppConfig::default();
        let toml_str = toml::to_string_pretty(&initial_config).unwrap();
        std::fs::write(&config_file, toml_str).unwrap();

        let (state, headers) = test_state_with_session(Some(config_file.clone())).await;

        // 1. Patch config (features, trash_retention_days, rate_limit, helper, search) with save=true
        let patch = ConfigPatch {
            features: Some(
                FeatureFlags {
                    save_text_file: false,
                    chmod_file: false,
                    extract_archive: true,
                    delete_permanently: false,
                    chown: false,
                }
                .into(),
            ),
            trash_retention_days: Some(Some(45)),
            rate_limit: Some(
                crate::config::RateLimitConfig {
                    max_login_failures: 8,
                    lockout_duration_seconds: 600,
                }
                .into(),
            ),
            helper: Some(
                crate::config::HelperConfig {
                    timeout_seconds: 25,
                }
                .into(),
            ),
            search: Some(
                crate::config::SearchConfig {
                    max_depth: 30,
                    default_max_results: 150,
                    max_results_limit: 2000,
                }
                .into(),
            ),
            ..ConfigPatch::default()
        };

        let patch_resp = patch_config_handler(
            State(state.clone()),
            headers.clone(),
            Query(PatchConfigQuery { save: true }),
            Json(patch),
        )
        .await;
        assert_eq!(patch_resp.status(), StatusCode::OK);

        // Verify in-memory state updated
        assert!(!state.config.load().features.save_text_file);
        assert_eq!(state.config.load().trash_retention_days, Some(45));
        assert_eq!(state.config.load().rate_limit.max_login_failures, 8);
        assert_eq!(state.config.load().helper.timeout_seconds, 25);
        assert_eq!(state.config.load().search.max_depth, 30);

        // Verify persisted to disk
        let disk_content = std::fs::read_to_string(&config_file).unwrap();
        assert!(disk_content.contains("save_text_file = false"));
        assert!(disk_content.contains("trash_retention_days = 45"));
        assert!(disk_content.contains("max_login_failures = 8"));
        assert!(disk_content.contains("timeout_seconds = 25"));

        // 2. Modify disk file externally and test reload endpoint
        let modified_disk =
            disk_content.replace("trash_retention_days = 45", "trash_retention_days = 60");
        std::fs::write(&config_file, modified_disk).unwrap();

        let reload_resp = reload_config_handler(State(state.clone()), headers).await;
        assert_eq!(reload_resp.status(), StatusCode::OK);

        // Verify in-memory state reloaded
        assert_eq!(state.config.load().trash_retention_days, Some(60));

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn test_admin_endpoint_forbidden_for_non_admin() {
        let (state, headers) = test_state_with_session(None).await;

        // Patch config to only allow "superuser" as admin
        let patch = ConfigPatch {
            admin_users: Some(vec!["superuser".to_owned()]),
            ..ConfigPatch::default()
        };
        let _ = state.config.apply_patch(patch);

        // Current session is "admin", who is not "superuser"
        let get_resp = get_config_handler(State(state.clone()), headers.clone()).await;
        assert_eq!(get_resp.status(), StatusCode::FORBIDDEN);

        let patch_resp = patch_config_handler(
            State(state.clone()),
            headers.clone(),
            Query(PatchConfigQuery { save: false }),
            Json(ConfigPatch::default()),
        )
        .await;
        assert_eq!(patch_resp.status(), StatusCode::FORBIDDEN);

        let reload_resp = reload_config_handler(State(state), headers).await;
        assert_eq!(reload_resp.status(), StatusCode::FORBIDDEN);
    }
}
