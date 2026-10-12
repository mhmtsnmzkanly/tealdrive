use std::path::Path;

use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::header::{
    ACCEPT_RANGES, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_SECURITY_POLICY,
    CONTENT_TYPE, RANGE, X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::audit::event::AuditReason;
use crate::config::{RelativePath, RootId, UserContext};
use crate::errors::TealDriveError;
use crate::fs::path::validate_relative_path;
use crate::policy::decision::{FileOperation, PolicyDecision, PolicyEngine};
use crate::policy::roots::require_allowed_root;
use crate::protocol::frame::RequestId;
use crate::session::store::{InMemorySessionStore, SessionLookupResult, SessionStore};
use crate::ui::mime_for_path;
use crate::ws::server::{extract_session_id, WsHelperBackend, WsServerState};

pub const STREAM_CHUNK_SIZE: usize = 64 * 1024;

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadQueryParams {
    pub root_id: Option<String>,
    pub root: Option<String>,
    pub path: String,
}

impl DownloadQueryParams {
    pub fn resolved_root_id(&self) -> Option<&str> {
        self.root_id.as_deref().or(self.root.as_deref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,
}

pub fn parse_range_header(header: &str, file_size: u64) -> Option<Result<ByteRange, ()>> {
    let s = header.trim();
    if !s.starts_with("bytes=") {
        return None;
    }
    let range_part = &s["bytes=".len()..];
    let (start_str, end_str) = range_part.split_once('-')?;
    let start_str = start_str.trim();
    let end_str = end_str.trim();

    if start_str.is_empty() {
        let suffix: u64 = end_str.parse().ok()?;
        if suffix == 0 || file_size == 0 {
            return Some(Err(()));
        }
        let start = file_size.saturating_sub(suffix);
        Some(Ok(ByteRange {
            start,
            end: file_size - 1,
        }))
    } else {
        let start: u64 = start_str.parse().ok()?;
        if start >= file_size {
            return Some(Err(()));
        }
        let end = if end_str.is_empty() {
            file_size - 1
        } else {
            let parsed_end: u64 = end_str.parse().ok()?;
            parsed_end.min(file_size - 1)
        };
        if start > end {
            return Some(Err(()));
        }
        Some(Ok(ByteRange { start, end }))
    }
}

fn range_request(
    user: &UserContext,
    root_id: &RootId,
    path: &RelativePath,
    offset: u64,
    length: usize,
) -> crate::helper::types::HelperRequest {
    use crate::helper::types::*;
    HelperRequest {
        request_id: RequestId::new(),
        username: user.username.clone(),
        uid: user.uid,
        gid: user.gid,
        command: HelperCommand::ReadFileRange,
        root_id: root_id.clone(),
        relative_path: path.clone(),
        target_relative_path: None,
        policy_context: HelperPolicyContext {
            read_only_root: true,
            is_web_root: false,
        },
        limits: HelperLimits {
            max_directory_page_size: 1,
            max_text_edit_size: 1,
            max_chunk_size: length,
            max_download_helper_bytes: length,
        },
        list_options: None,
        write_options: Some(HelperWriteOptions {
            expected_revision: None,
            data: Vec::new(),
            offset,
            truncate: false,
        }),
        chmod_mode: None,
        archive_source_paths: None,
        search_options: None,
    }
}
fn read_range<H: WsHelperBackend>(
    helper: &H,
    request: &crate::helper::types::HelperRequest,
) -> Result<(u64, Vec<u8>), TealDriveError> {
    let response = crate::download::transfer::DownloadHelper::execute_helper(helper, request)?;
    if !response.success {
        return Err(crate::download::transfer::helper_response_to_error(
            &response,
        ));
    }
    match response.payload {
        crate::helper::types::HelperResponsePayload::FileRange {
            total_size, data, ..
        } => Ok((total_size, data)),
        _ => Err(TealDriveError::HelperMalformedResponse),
    }
}
fn create_helper_stream<H: WsHelperBackend + 'static>(
    state: &WsServerState<H>,
    request: crate::helper::types::HelperRequest,
    length: u64,
    policy: crate::config::AppConfig,
    lease: crate::session::lease::SessionLease,
    session_id: crate::session::store::SessionId,
) -> axum::body::Body {
    let helper = state.helper_client.clone();
    let store = state.session_store.clone();
    let slots = state.operation_slots.clone();
    let shutdown = state.shutdown.clone();
    let stream = futures_util::stream::unfold(
        (
            helper,
            request.clone(),
            length,
            policy,
            lease,
            store,
            session_id,
            slots,
            shutdown,
        ),
        |(helper, mut request, remaining, policy, lease, store, session_id, slots, shutdown)| async move {
            if remaining == 0 {
                return None;
            }
            if !matches!(
                store
                    .lock()
                    .await
                    .touch_session(&session_id, InMemorySessionStore::now_epoch_seconds()),
                SessionLookupResult::Found(_)
            ) {
                lease.cancellation.cancel();
                return Some((
                    Err(std::io::Error::other("session revoked or expired")),
                    (
                        helper, request, 0, policy, lease, store, session_id, slots, shutdown,
                    ),
                ));
            }
            request.limits.max_chunk_size = (STREAM_CHUNK_SIZE as u64).min(remaining) as usize;
            let req = request.clone();
            let client = helper.clone();
            let snapshot = policy.clone();
            let token = lease.cancellation.clone();
            let permit = match slots.clone().acquire_owned().await {
                Ok(p) => p,
                Err(_) => return None,
            };
            if lease.cancellation.is_cancelled() {
                return None;
            }
            // An admitted stream may keep reading during the drain period. Its body
            // guard remains tracked; the helper job owns another guard if available.
            let activity = shutdown.enter_existing();
            let result = tokio::task::spawn_blocking(move || {
                let _activity = activity;
                let _permit = permit;
                crate::session::lease::with_cancellation(token, || {
                    crate::helper::policy::with_snapshot(&snapshot, || {
                        read_range(client.as_ref(), &req)
                    })
                })
            })
            .await;
            match result {
                Ok(Ok((_, data))) if !data.is_empty() => {
                    let count = data.len() as u64;
                    request.write_options.as_mut().unwrap().offset += count;
                    Some((
                        Ok::<_, std::io::Error>(Bytes::from(data)),
                        (
                            helper,
                            request,
                            remaining - count,
                            policy,
                            lease,
                            store,
                            session_id,
                            slots,
                            shutdown,
                        ),
                    ))
                }
                _ => Some((
                    Err(std::io::Error::other("helper stream failed")),
                    (
                        helper, request, 0, policy, lease, store, session_id, slots, shutdown,
                    ),
                )),
            }
        },
    );
    axum::body::Body::from_stream(stream)
}

pub async fn download_handler<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
    Query(query): Query<DownloadQueryParams>,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    serve_file_http(&state, &headers, &query, "attachment").await
}

pub async fn preview_handler<H>(
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
    Query(query): Query<DownloadQueryParams>,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    serve_file_http(&state, &headers, &query, "inline").await
}

async fn serve_file_http<H>(
    state: &WsServerState<H>,
    headers: &HeaderMap,
    query: &DownloadQueryParams,
    disposition_type: &str,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    let cfg = state.config.load();
    let now = InMemorySessionStore::now_epoch_seconds();
    let session_id = match extract_session_id(headers, &cfg.session.cookie_name) {
        Ok(id) => id,
        Err(_) => {
            return (StatusCode::UNAUTHORIZED, r#"{"error":"unauthorized"}"#).into_response();
        }
    };

    let session = {
        let store = state.session_store.lock().await;
        match store.get_session(&session_id, now) {
            SessionLookupResult::Found(session) => session,
            SessionLookupResult::NotFound
            | SessionLookupResult::Expired
            | SessionLookupResult::Revoked => {
                return (StatusCode::UNAUTHORIZED, r#"{"error":"unauthorized"}"#).into_response();
            }
        }
    };

    let user_context = UserContext {
        username: session.username.clone(),
        uid: session.uid,
        gid: session.gid,
    };

    let Some(root_name) = query.resolved_root_id() else {
        return (StatusCode::BAD_REQUEST, r#"{"error":"missing_root_id"}"#).into_response();
    };

    let root_id = RootId::new(root_name);
    let root = match require_allowed_root(cfg.allowed_roots(), &root_id) {
        Ok(root) => root,
        Err(_) => {
            return (StatusCode::NOT_FOUND, r#"{"error":"root_not_found"}"#).into_response();
        }
    };

    let relative_path = match validate_relative_path(&RelativePath::new(&query.path)) {
        Ok(path) => path,
        Err(_) => {
            return (StatusCode::BAD_REQUEST, r#"{"error":"invalid_path"}"#).into_response();
        }
    };

    let policy_engine = PolicyEngine::new(&cfg);

    match policy_engine.check_operation(
        FileOperation::DownloadFile,
        &root_id,
        &relative_path,
        None,
        None,
        root.hidden_files_allowed,
    ) {
        Ok(PolicyDecision::Allow | PolicyDecision::WarnAllowed(_)) => {}
        _ => {
            let mut response =
                (StatusCode::FORBIDDEN, r#"{"error":"policy_denied"}"#).into_response();
            response
                .extensions_mut()
                .insert(crate::audit::http::AuditFailure(AuditReason::PolicyDenied));
            return response;
        }
    }

    let lease = crate::session::lease::SessionLease::with_shutdown(
        state.session_store.clone(),
        session_id,
        state.shutdown.cancellation(),
    );
    let probe = range_request(&user_context, &root_id, &relative_path, 0, 0);
    let helper = state.helper_client.clone();
    let policy = cfg.as_ref().clone();
    let permit = match state.operation_slots.clone().acquire_owned().await {
        Ok(p) => p,
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let Some(activity) = state.shutdown.enter() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let token = lease.cancellation.clone();
    let file_size = match tokio::task::spawn_blocking(move || {
        let _activity = activity;
        let _permit = permit;
        crate::session::lease::with_cancellation(token, || {
            crate::helper::policy::with_snapshot(&policy, || read_range(helper.as_ref(), &probe))
        })
    })
    .await
    {
        Ok(Ok((size, _))) => size,
        Ok(Err(TealDriveError::PermissionDenied | TealDriveError::PolicyDenied)) => {
            return StatusCode::FORBIDDEN.into_response()
        }
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    if disposition_type == "inline" && file_size > cfg.limits.max_preview_size as u64 {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }
    let helper_request = range_request(
        &user_context,
        &root_id,
        &relative_path,
        0,
        STREAM_CHUNK_SIZE,
    );

    let filename = Path::new(&relative_path.0)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");
    let safe_filename = filename.replace(['"', '\n', '\r', '\\'], "_");
    let mime = mime_for_path(Path::new(&relative_path.0));
    let lower_filename = safe_filename.to_ascii_lowercase();
    let is_dangerous_web_content = lower_filename.ends_with(".html")
        || lower_filename.ends_with(".htm")
        || lower_filename.ends_with(".svg")
        || lower_filename.ends_with(".xml")
        || lower_filename.ends_with(".xhtml");

    let effective_disposition_type = if disposition_type == "inline" && is_dangerous_web_content {
        "attachment"
    } else {
        disposition_type
    };
    let disposition_val = format!("{effective_disposition_type}; filename=\"{safe_filename}\"");

    let range_header = headers.get(RANGE).and_then(|v| v.to_str().ok());

    let (status, body, content_range, content_length) = if let Some(range_str) = range_header {
        match parse_range_header(range_str, file_size) {
            Some(Ok(range)) => {
                let mut helper_request = helper_request.clone();
                helper_request.write_options.as_mut().unwrap().offset = range.start;
                let length = range.end - range.start + 1;
                let body = create_helper_stream(
                    state,
                    helper_request,
                    length,
                    cfg.as_ref().clone(),
                    lease,
                    session_id,
                );
                let cr_val = format!("bytes {}-{}/{}", range.start, range.end, file_size);
                (StatusCode::PARTIAL_CONTENT, body, Some(cr_val), length)
            }
            Some(Err(())) => {
                let mut resp = StatusCode::RANGE_NOT_SATISFIABLE.into_response();
                let cr_val = format!("bytes */{file_size}");
                if let Ok(val) = HeaderValue::from_str(&cr_val) {
                    resp.headers_mut().insert(CONTENT_RANGE, val);
                }
                return resp;
            }
            None => {
                let body = create_helper_stream(
                    state,
                    helper_request,
                    file_size,
                    cfg.as_ref().clone(),
                    lease,
                    session_id,
                );
                (StatusCode::OK, body, None, file_size)
            }
        }
    } else {
        let body = create_helper_stream(
            state,
            helper_request,
            file_size,
            cfg.as_ref().clone(),
            lease,
            session_id,
        );
        (StatusCode::OK, body, None, file_size)
    };

    let mut response = (status, body).into_response();
    let resp_headers = response.headers_mut();

    if let Ok(val) = HeaderValue::from_str(mime) {
        resp_headers.insert(CONTENT_TYPE, val);
    }
    if let Ok(val) = HeaderValue::from_str(&disposition_val) {
        resp_headers.insert(CONTENT_DISPOSITION, val);
    }
    resp_headers.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    resp_headers.insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&content_length.to_string()).unwrap(),
    );
    resp_headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    if disposition_type == "inline" {
        resp_headers.insert(
            CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("sandbox; default-src 'none'; img-src 'self' data:; media-src 'self' data:; style-src 'unsafe-inline'"),
        );
    }

    if let Some(cr) = content_range {
        if let Ok(val) = HeaderValue::from_str(&cr) {
            resp_headers.insert(CONTENT_RANGE, val);
        }
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_range_header_full_range() {
        assert_eq!(
            parse_range_header("bytes=0-499", 1000),
            Some(Ok(ByteRange { start: 0, end: 499 }))
        );
    }

    #[test]
    fn test_parse_range_header_open_end() {
        assert_eq!(
            parse_range_header("bytes=500-", 1000),
            Some(Ok(ByteRange {
                start: 500,
                end: 999
            }))
        );
    }

    #[test]
    fn test_parse_range_header_suffix() {
        assert_eq!(
            parse_range_header("bytes=-200", 1000),
            Some(Ok(ByteRange {
                start: 800,
                end: 999
            }))
        );
    }

    #[test]
    fn test_parse_range_header_unsatisfiable() {
        assert_eq!(parse_range_header("bytes=1000-1500", 1000), Some(Err(())));
        assert_eq!(parse_range_header("bytes=600-500", 1000), Some(Err(())));
    }

    #[test]
    fn test_parse_range_header_invalid_syntax() {
        assert_eq!(parse_range_header("invalid", 1000), None);
        assert_eq!(parse_range_header("items=0-10", 1000), None);
    }

    #[test]
    fn test_preview_disposition_security() {
        let is_dangerous = |name: &str| {
            let lower = name.to_ascii_lowercase();
            lower.ends_with(".html")
                || lower.ends_with(".htm")
                || lower.ends_with(".svg")
                || lower.ends_with(".xml")
                || lower.ends_with(".xhtml")
        };

        assert!(is_dangerous("exploit.html"));
        assert!(is_dangerous("IMAGE.SVG"));
        assert!(is_dangerous("doc.xml"));
        assert!(!is_dangerous("photo.png"));
        assert!(!is_dangerous("video.mp4"));
        assert!(!is_dangerous("document.pdf"));
    }
}
