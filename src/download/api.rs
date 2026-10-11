use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::header::{
    ACCEPT_RANGES, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE,
    X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::audit::event::{AuditEvent, AuditReason, AuditStatus};
use crate::config::{RelativePath, RootId, UserContext};
use crate::fs::path::{resolve_existing_path, validate_relative_path};
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

struct FileStreamState {
    file: File,
    remaining: u64,
}

pub fn create_stream_body(file: File, remaining: u64) -> axum::body::Body {
    let state = FileStreamState { file, remaining };
    let stream = futures_util::stream::unfold(state, |mut s| async move {
        if s.remaining == 0 {
            return None;
        }
        let to_read = (STREAM_CHUNK_SIZE as u64).min(s.remaining) as usize;
        let res = tokio::task::spawn_blocking(move || {
            let mut buf = vec![0u8; to_read];
            match s.file.read(&mut buf) {
                Ok(0) => Ok((buf, s, 0)),
                Ok(n) => {
                    buf.truncate(n);
                    Ok((buf, s, n))
                }
                Err(e) => Err((e, s)),
            }
        })
        .await;

        match res {
            Ok(Ok((buf, mut s, n))) if n > 0 => {
                s.remaining -= n as u64;
                Some((Ok::<_, std::io::Error>(Bytes::from(buf)), s))
            }
            _ => None,
        }
    });
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
            let _ = state.audit_sink.emit(&AuditEvent::download(
                RequestId::new(),
                &user_context,
                root_id.clone(),
                relative_path.clone(),
                AuditStatus::Denied,
                Some(AuditReason::PolicyDenied),
                0,
            ));
            return (StatusCode::FORBIDDEN, r#"{"error":"policy_denied"}"#).into_response();
        }
    }

    let resolution = match resolve_existing_path(root, &relative_path, false) {
        Ok(res) => res,
        Err(_) => {
            return (StatusCode::NOT_FOUND, r#"{"error":"file_not_found"}"#).into_response();
        }
    };

    let metadata = match resolution.resolved_path.metadata() {
        Ok(meta) if meta.is_file() => meta,
        _ => {
            return (StatusCode::BAD_REQUEST, r#"{"error":"not_a_file"}"#).into_response();
        }
    };

    let file_size = metadata.len();
    if disposition_type == "inline" && file_size > cfg.limits.max_preview_size as u64 {
        return (StatusCode::PAYLOAD_TOO_LARGE, r#"{"error":"preview_too_large"}"#).into_response();
    }

    let mut file = match File::open(&resolution.resolved_path) {
        Ok(f) => f,
        Err(_) => {
            return (StatusCode::NOT_FOUND, r#"{"error":"open_failed"}"#).into_response();
        }
    };

    let filename = Path::new(&relative_path.0)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");
    let safe_filename = filename.replace(['"', '\n', '\r', '\\'], "_");
    let mime = mime_for_path(&resolution.resolved_path);
    let disposition_val = format!("{disposition_type}; filename=\"{safe_filename}\"");

    let range_header = headers.get(RANGE).and_then(|v| v.to_str().ok());

    let (status, body, content_range, content_length) = if let Some(range_str) = range_header {
        match parse_range_header(range_str, file_size) {
            Some(Ok(range)) => {
                if file.seek(SeekFrom::Start(range.start)).is_err() {
                    return (StatusCode::INTERNAL_SERVER_ERROR, "seek error").into_response();
                }
                let length = range.end - range.start + 1;
                let body = create_stream_body(file, length);
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
                let body = create_stream_body(file, file_size);
                (StatusCode::OK, body, None, file_size)
            }
        }
    } else {
        let body = create_stream_body(file, file_size);
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

    if let Some(cr) = content_range {
        if let Ok(val) = HeaderValue::from_str(&cr) {
            resp_headers.insert(CONTENT_RANGE, val);
        }
    }

    let _ = state.audit_sink.emit(&AuditEvent::download(
        RequestId::new(),
        &user_context,
        root_id,
        relative_path,
        AuditStatus::Success,
        None,
        content_length,
    ));

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
}
