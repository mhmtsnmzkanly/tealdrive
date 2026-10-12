//! Request outcomes and streaming bodies are audited even when dropped mid-response.
use super::sink::JsonlAuditSink;
use super::{context::AuditContext, AuditAction, AuditEvent, AuditReason, AuditStatus};
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use std::sync::Arc;

#[derive(Clone)]
pub struct AuditFailure(pub AuditReason);

pub struct HttpAudit {
    event: AuditEvent,
    context: AuditContext,
    sink: Arc<JsonlAuditSink>,
    enabled: bool,
    expected: Option<u64>,
    written: u64,
    ended: bool,
    failure: Option<AuditReason>,
}
impl HttpAudit {
    pub fn new(
        method: &Method,
        uri: &Uri,
        context: AuditContext,
        sink: Arc<JsonlAuditSink>,
    ) -> Self {
        let path = uri.path();
        let action = match path {
            "/api/download" => AuditAction::Download,
            "/api/preview" => AuditAction::FilePreview,
            _ => AuditAction::HttpRequest,
        };
        let mut event = AuditEvent::new(
            context.request_id.unwrap_or_default(),
            "unknown",
            0,
            0,
            action,
            AuditStatus::Started,
        );
        context.enrich(&mut event);
        event.phase = Some("request_started".into());
        event.http_method = Some(method.to_string());
        event.endpoint = Some(path.chars().take(512).collect());
        if matches!(action, AuditAction::Download | AuditAction::FilePreview) {
            if let Ok(axum::extract::Query(query)) =
                axum::extract::Query::<crate::download::api::DownloadQueryParams>::try_from_uri(uri)
            {
                event.root_id = query.resolved_root_id().map(crate::config::RootId::new);
                event.relative_path = Some(crate::config::RelativePath::new(
                    query.path.chars().take(1024).collect::<String>(),
                ));
            }
        }
        let enabled = path != "/api/health" && (path.starts_with("/api/") || path == "/ws");
        if enabled && path != "/ws" {
            let _ = sink.emit(&event);
        }
        Self {
            event,
            context,
            sink,
            enabled,
            expected: None,
            written: 0,
            ended: false,
            failure: None,
        }
    }
    pub fn response(&mut self, status: StatusCode, headers: &HeaderMap) {
        self.event.http_status = Some(status.as_u16());
        if status == StatusCode::SWITCHING_PROTOCOLS {
            self.enabled = false;
        }
        if status.is_client_error() || status.is_server_error() {
            self.enabled = true;
        }
        self.expected = headers
            .get(axum::http::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok());
        if self.event.http_method.as_deref() == Some("HEAD")
            || matches!(
                status,
                StatusCode::NO_CONTENT | StatusCode::RESET_CONTENT | StatusCode::NOT_MODIFIED
            )
        {
            self.expected = Some(0);
        }
        if status.is_success()
            && matches!(
                self.event.action,
                AuditAction::Download | AuditAction::FilePreview
            )
        {
            let mut start = self.event.clone();
            start.phase = Some("transfer_started".into());
            if self.enabled {
                let _ = self.sink.emit(&start);
            }
        }
    }
    pub fn set_context(&mut self, context: AuditContext) {
        context.enrich(&mut self.event);
        self.context = context;
    }
    pub fn rejected(&mut self, reason: AuditReason) {
        self.event.reason = Some(reason);
    }
    pub fn chunk(&mut self, length: usize) {
        self.written = self.written.saturating_add(length as u64);
    }
    pub fn end(&mut self) {
        self.ended = true;
    }
    pub fn fail(&mut self, reason: AuditReason) {
        self.failure = Some(reason);
    }
}
impl Drop for HttpAudit {
    fn drop(&mut self) {
        if !self.enabled {
            return;
        }
        let code = self.event.http_status.unwrap_or(503);
        let body_complete = self.ended || self.expected == Some(self.written);
        self.event.bytes_out = self.written;
        if let Some(reason) = self.failure {
            self.event.status = AuditStatus::Failed;
            self.event.reason = Some(reason);
            self.event.phase = Some("response_interrupted".into());
        } else if code >= 400 {
            self.event.status = if matches!(code, 401 | 403 | 429) {
                AuditStatus::Denied
            } else {
                AuditStatus::Failed
            };
            self.event.reason = self.event.reason.or(Some(match code {
                401 => AuditReason::Unauthenticated,
                403 => AuditReason::PermissionDenied,
                429 => AuditReason::RateLimited,
                404 => AuditReason::NotFound,
                400 | 405 | 413 | 416 | 422 => AuditReason::ValidationError,
                _ => AuditReason::InternalError,
            }));
            self.event.phase = Some("request_rejected".into());
        } else if !body_complete {
            self.event.status = AuditStatus::Failed;
            self.event.reason = Some(AuditReason::ConnectionClosed);
            self.event.phase = Some("response_interrupted".into());
        } else {
            self.event.status = AuditStatus::Success;
            self.event.phase = Some(
                if matches!(
                    self.event.action,
                    AuditAction::Download | AuditAction::FilePreview
                ) && self.event.http_method.as_deref() != Some("HEAD")
                {
                    "transfer_completed"
                } else {
                    "request_completed"
                }
                .into(),
            );
        }
        let _ = self.sink.emit(&self.event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(length: usize, complete: bool) -> Vec<AuditEvent> {
        let path = std::env::temp_dir().join(format!(
            "tdrv-http-audit-{}.jsonl",
            crate::protocol::frame::RequestId::new().0
        ));
        let sink = Arc::new(JsonlAuditSink::open(&path).unwrap());
        {
            let mut guard = HttpAudit::new(
                &Method::GET,
                &"/api/download?root_id=test&path=file.txt".parse().unwrap(),
                AuditContext::default(),
                sink,
            );
            let mut headers = HeaderMap::new();
            headers.insert("content-length", "10".parse().unwrap());
            guard.response(StatusCode::OK, &headers);
            guard.chunk(length);
            if complete {
                guard.end();
            }
        }
        std::fs::read_to_string(&path)
            .inspect(|_| {
                let _ = std::fs::remove_file(&path);
            })
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
    #[test]
    fn completed_and_dropped_downloads_have_distinct_outcomes() {
        let events = run(10, true);
        let last = events.last().unwrap();
        assert_eq!(last.status, AuditStatus::Success);
        assert_eq!(last.bytes_out, 10);
        assert_eq!(last.phase.as_deref(), Some("transfer_completed"));
        let events = run(3, false);
        let last = events.last().unwrap();
        assert_eq!(last.status, AuditStatus::Failed);
        assert_eq!(last.reason, Some(AuditReason::ConnectionClosed));
        assert_eq!(last.bytes_out, 3);
    }
    #[test]
    fn bodyless_protocol_responses_are_not_reported_as_interrupted() {
        for status in [
            StatusCode::NO_CONTENT,
            StatusCode::RESET_CONTENT,
            StatusCode::NOT_MODIFIED,
        ] {
            let path = std::env::temp_dir().join(format!(
                "tdrv-bodyless-audit-{}.jsonl",
                uuid::Uuid::new_v4()
            ));
            let sink = Arc::new(JsonlAuditSink::open(&path).unwrap());
            {
                let mut audit = HttpAudit::new(
                    &Method::POST,
                    &"/api/session/bind".parse().unwrap(),
                    AuditContext::default(),
                    sink,
                );
                audit.response(status, &HeaderMap::new());
                // Hyper can finish a bodyless response without ever polling its body wrapper.
            }
            let lines = std::fs::read_to_string(&path).unwrap();
            let last: AuditEvent = serde_json::from_str(lines.lines().last().unwrap()).unwrap();
            assert_eq!(last.status, AuditStatus::Success);
            assert_eq!(last.bytes_out, 0);
            assert_eq!(last.reason, None);
            std::fs::remove_file(path).unwrap();
        }
    }
}
