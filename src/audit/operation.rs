//! One audit guard surrounds every dispatched browser command. Only allowlisted
//! metadata is extracted; contents, credentials and arbitrary payloads are never logged.
use super::context::AuditContext;
use super::sink::JsonlAuditSink;
use super::{AuditAction, AuditEvent, AuditReason, AuditStatus};
use crate::errors::{ErrorKind, TealDriveError};
use crate::protocol::{frame::TdrvFrame, message_type::MessageType};
use crate::ws::{connection::WsConnectionContext, dispatch::DispatchDecision};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct Observations {
    read: u64,
    written: u64,
    error: Option<ErrorKind>,
}
thread_local! { static OBSERVER: RefCell<Option<Arc<Mutex<Observations>>>> = const { RefCell::new(None) }; }
pub fn observe(response: &crate::helper::types::HelperResponse) {
    OBSERVER.with(|slot| {
        if let Some(observer) = slot.borrow().as_ref() {
            let mut o = observer.lock().unwrap();
            o.read = o.read.saturating_add(response.bytes_read);
            o.written = o.written.saturating_add(response.bytes_written);
            if !response.success {
                o.error = response.error_kind;
            }
        }
    });
}
pub fn with_observer<T>(observer: Arc<Mutex<Observations>>, f: impl FnOnce() -> T) -> T {
    struct Reset(Option<Arc<Mutex<Observations>>>);
    impl Drop for Reset {
        fn drop(&mut self) {
            OBSERVER.with(|s| *s.borrow_mut() = self.0.take());
        }
    }
    let _reset = Reset(OBSERVER.with(|s| s.replace(Some(observer))));
    f()
}
pub struct OperationAudit {
    pub event: AuditEvent,
    sink: Arc<JsonlAuditSink>,
    pub observer: Arc<Mutex<Observations>>,
    cancellation: crate::session::lease::Cancellation,
    finished: bool,
}
fn text(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|v| v.as_str())
        .map(|v| v.chars().take(1024).collect())
}
pub fn action(message: MessageType) -> AuditAction {
    use MessageType::*;
    match message {
        ClientHello => AuditAction::WebSocketHandshake,
        ListDirectory => AuditAction::ListDirectory,
        FileMetadataRequest => AuditAction::FileMetadata,
        ReadTextFile => AuditAction::FilePreview,
        SaveTextFile => AuditAction::SaveTextFile,
        SearchFiles => AuditAction::SearchFiles,
        ListTrash => AuditAction::ListTrash,
        UploadBegin | UploadChunk | UploadEnd | UploadAbort => AuditAction::Upload,
        DownloadBegin => AuditAction::Download,
        CreateFolder => AuditAction::CreateFolder,
        RenameFile => AuditAction::Rename,
        MoveFile => AuditAction::MoveFile,
        CopyFile => AuditAction::CopyFile,
        MoveToTrash => AuditAction::MoveToTrash,
        RestoreFromTrash => AuditAction::RestoreFromTrash,
        DeletePermanently => AuditAction::DeletePermanently,
        ChmodFile => AuditAction::Chmod,
        CompressZip => AuditAction::CompressZip,
        CompressTarGz => AuditAction::CompressTarGz,
        ExtractArchive => AuditAction::ExtractArchive,
        _ => AuditAction::WebSocketMessageRejected,
    }
}
pub fn error_kind(error: &TealDriveError) -> ErrorKind {
    match error {
        TealDriveError::PermissionDenied => ErrorKind::PermissionDenied,
        TealDriveError::PolicyDenied
        | TealDriveError::TraversalRejected
        | TealDriveError::PathEscapesRoot
        | TealDriveError::SymlinkEscape
        | TealDriveError::SymlinkDenied
        | TealDriveError::SensitiveFileDenied
        | TealDriveError::HiddenFileDenied
        | TealDriveError::ReadOnlyRoot
        | TealDriveError::WebrootExecutableDenied => ErrorKind::PolicyDenied,
        TealDriveError::Protocol
        | TealDriveError::InvalidMagic
        | TealDriveError::UnsupportedVersion
        | TealDriveError::InvalidHeaderLength
        | TealDriveError::ReservedFieldNonZero
        | TealDriveError::UnknownMessageType
        | TealDriveError::InvalidMessageDirection
        | TealDriveError::InvalidFlags
        | TealDriveError::InvalidEncoding
        | TealDriveError::InvalidCompression
        | TealDriveError::PayloadLengthMismatch
        | TealDriveError::InvalidChunk => ErrorKind::ProtocolError,
        TealDriveError::AlreadyExists => ErrorKind::AlreadyExists,
        TealDriveError::InvalidTarget => ErrorKind::InvalidTarget,
        TealDriveError::CrossDeviceMove => ErrorKind::CrossDeviceMove,
        TealDriveError::FileTooLarge => ErrorKind::FileTooLarge,
        TealDriveError::DiskFull => ErrorKind::DiskFull,
        TealDriveError::UnsupportedBinaryFile => ErrorKind::UnsupportedBinaryFile,
        TealDriveError::InvalidTextEncoding => ErrorKind::InvalidTextEncoding,
        TealDriveError::InvalidPath => ErrorKind::InvalidPath,
        TealDriveError::FeatureDisabled => ErrorKind::FeatureDisabled,
        TealDriveError::NotFound => ErrorKind::NotFound,
        TealDriveError::HelperStdinFailed
        | TealDriveError::HelperStdoutFailed
        | TealDriveError::HelperMalformedResponse
        | TealDriveError::HelperDecodeFailed
        | TealDriveError::HelperEncodeFailed
        | TealDriveError::Internal
        | TealDriveError::HelperTimeout
        | TealDriveError::HelperSpawnFailed
        | TealDriveError::HelperNonZeroExit => ErrorKind::InternalError,
        TealDriveError::NotImplemented | TealDriveError::HelperCommandNotImplemented => {
            ErrorKind::NotImplemented
        }
        _ => ErrorKind::ValidationError,
    }
}
pub fn mark_error(event: &mut AuditEvent, kind: ErrorKind) {
    event.error_kind = Some(kind);
    event.status = if matches!(
        kind,
        ErrorKind::PermissionDenied
            | ErrorKind::PolicyDenied
            | ErrorKind::FeatureDisabled
            | ErrorKind::RateLimited
    ) {
        AuditStatus::Denied
    } else {
        AuditStatus::Failed
    };
    event.reason = Some(match kind {
        ErrorKind::PermissionDenied => AuditReason::PermissionDenied,
        ErrorKind::PolicyDenied => AuditReason::PolicyDenied,
        ErrorKind::FeatureDisabled => AuditReason::FeatureDisabled,
        ErrorKind::NotFound => AuditReason::NotFound,
        ErrorKind::RateLimited => AuditReason::RateLimited,
        ErrorKind::InternalError => AuditReason::InternalError,
        ErrorKind::ProtocolError => AuditReason::ProtocolError,
        _ => AuditReason::ValidationError,
    });
}
impl OperationAudit {
    pub fn new(
        frame: &TdrvFrame,
        context: &WsConnectionContext,
        audit_context: &AuditContext,
        sink: Arc<JsonlAuditSink>,
        cancellation: crate::session::lease::Cancellation,
    ) -> Self {
        let message = frame.header.message_type;
        let mut event = AuditEvent::new(
            frame.header.request_id,
            &context.username,
            context.uid,
            context.gid,
            action(message),
            AuditStatus::Started,
        );
        audit_context.enrich(&mut event);
        event.request_id = frame.header.request_id;
        event.operation = Some(format!("{message:?}"));
        event.phase = Some("command_started".into());
        event.bytes_in = frame.payload.len() as u64;
        if frame.header.encoding == crate::protocol::header::Encoding::MessagePack {
            if let Ok(value) =
                crate::protocol::codec::decode_msgpack::<serde_json::Value>(&frame.payload)
            {
                event.root_id = text(&value, "root_id").map(crate::config::RootId::new);
                event.relative_path = text(&value, "relative_path")
                    .or_else(|| text(&value, "parent_relative_path"))
                    .or_else(|| text(&value, "base_path"))
                    .or_else(|| text(&value, "archive_relative_path"))
                    .map(crate::config::RelativePath::new);
                event.target_root_id =
                    text(&value, "target_root_id").map(crate::config::RootId::new);
                event.target_relative_path =
                    text(&value, "target_relative_path").map(crate::config::RelativePath::new);
                event.trash_id = text(&value, "trash_id");
                event.mode = value
                    .get("mode")
                    .and_then(|v| v.as_u64())
                    .and_then(|v| v.try_into().ok());
                if let Some(paths) = value.get("relative_paths").and_then(|v| v.as_array()) {
                    event.source_count = Some(paths.len());
                    event.source_relative_paths = paths
                        .iter()
                        .take(100)
                        .filter_map(|v| {
                            v.as_str().map(|v| {
                                crate::config::RelativePath::new(
                                    v.chars().take(1024).collect::<String>(),
                                )
                            })
                        })
                        .collect();
                }
                let leaf = if message == MessageType::UploadBegin {
                    text(&value, "filename")
                } else if message == MessageType::CreateFolder {
                    text(&value, "folder_name")
                } else if message == MessageType::RenameFile {
                    text(&value, "new_name")
                } else {
                    None
                };
                if let Some(leaf) = leaf {
                    let source = event.relative_path.as_ref().map_or("", |p| p.0.as_str());
                    let parent = if message == MessageType::RenameFile {
                        source.rsplit_once('/').map_or("", |(p, _)| p)
                    } else {
                        source
                    };
                    event.target_relative_path =
                        Some(crate::config::RelativePath::new(if parent.is_empty() {
                            leaf
                        } else {
                            format!("{parent}/{leaf}")
                        }));
                }
            }
        }
        if message == MessageType::UploadBegin {
            event.relative_path = event.target_relative_path.clone();
        }
        let transfer = match message {
            MessageType::UploadChunk => crate::protocol::chunk::RawChunk::decode(&frame.payload)
                .ok()
                .map(|p| p.transfer_id),
            MessageType::UploadEnd => crate::protocol::codec::decode_msgpack::<
                crate::protocol::payload::UploadEndRequest,
            >(&frame.payload)
            .ok()
            .map(|p| p.transfer_id),
            MessageType::UploadAbort => crate::protocol::codec::decode_msgpack::<
                crate::protocol::payload::UploadAbortRequest,
            >(&frame.payload)
            .ok()
            .map(|p| p.transfer_id),
            _ => None,
        };
        if let Some(id) = transfer {
            event.transfer_id = Some(id);
            if let Some(upload) = context.active_uploads.get(&id) {
                event.root_id = Some(upload.root_id.clone());
                event.relative_path = Some(crate::config::RelativePath::new(
                    if upload.parent_relative_path.0.is_empty() {
                        upload.filename.0.clone()
                    } else {
                        format!("{}/{}", upload.parent_relative_path.0, upload.filename.0)
                    },
                ));
            }
        }
        let _ = sink.emit(&event);
        Self {
            event,
            sink,
            observer: Arc::new(Mutex::new(Observations::default())),
            cancellation,
            finished: false,
        }
    }
    pub fn finish(&mut self, result: &Result<DispatchDecision, TealDriveError>) {
        self.event.status = AuditStatus::Success;
        self.event.phase = Some("command_completed".into());
        let observed = self.observer.lock().unwrap();
        self.event.helper_bytes_read = observed.read;
        self.event.helper_bytes_written = observed.written;
        if let Some(kind) = observed.error {
            mark_error(&mut self.event, kind);
        }
        match result {
            Err(error) => mark_error(
                &mut self.event,
                observed.error.unwrap_or_else(|| error_kind(error)),
            ),
            Ok(DispatchDecision::Outbound(frames)) => {
                for frame in frames {
                    if matches!(
                        frame.header.message_type,
                        MessageType::OperationFailed
                            | MessageType::PermissionDenied
                            | MessageType::PolicyDenied
                            | MessageType::ValidationError
                            | MessageType::Error
                    ) {
                        if let Ok(p) = crate::protocol::codec::decode_msgpack::<
                            crate::protocol::payload::OperationFailedPayload,
                        >(&frame.payload)
                        {
                            mark_error(&mut self.event, p.error.error_kind);
                        } else {
                            mark_error(&mut self.event, ErrorKind::ProtocolError);
                        }
                    }
                }
                if self.event.status == AuditStatus::Success {
                    if self.event.operation.as_deref() == Some("MoveToTrash") {
                        if let Some(frame) = frames.first() {
                            if let Ok(payload) = crate::protocol::codec::decode_msgpack::<
                                crate::protocol::payload::MoveToTrashPayload,
                            >(&frame.payload)
                            {
                                self.event.trash_id = Some(payload.trash_id);
                            }
                        }
                    }
                    if self.event.operation.as_deref() == Some("DownloadBegin") {
                        if let Some(frame) = frames
                            .iter()
                            .find(|f| f.header.message_type == MessageType::DownloadEnd)
                        {
                            if let Ok(payload) = crate::protocol::codec::decode_msgpack::<
                                crate::protocol::payload::DownloadEndPayload,
                            >(&frame.payload)
                            {
                                self.event.transfer_id = Some(payload.transfer_id);
                            }
                        }
                    }
                }
                match self.event.operation.as_deref() {
                    Some("UploadBegin") if self.event.status == AuditStatus::Success => {
                        self.event.phase = Some("transfer_opened".into());
                        if let Some(frame) = frames.first() {
                            if let Ok(p) = crate::protocol::codec::decode_msgpack::<
                                crate::protocol::payload::UploadBeginResponse,
                            >(&frame.payload)
                            {
                                self.event.transfer_id = Some(p.transfer_id);
                            }
                        }
                    }
                    Some("UploadChunk") if self.event.status == AuditStatus::Success => {
                        self.event.phase = Some("chunk_received".into());
                    }
                    Some("UploadEnd") if self.event.status == AuditStatus::Success => {
                        self.event.phase = Some("transfer_completed".into());
                    }
                    Some("UploadAbort") if self.event.status == AuditStatus::Success => {
                        self.event.status = AuditStatus::Cancelled;
                        self.event.reason = Some(AuditReason::ClientAborted);
                        self.event.phase = Some("transfer_aborted".into());
                    }
                    Some("DownloadBegin") if self.event.status == AuditStatus::Success => {
                        self.event.status = AuditStatus::Started;
                        self.event.phase = Some("transfer_prepared".into());
                    }
                    _ => {}
                }
            }
            Ok(DispatchDecision::DispatchPlaceholder(_)) => {
                mark_error(&mut self.event, ErrorKind::NotImplemented);
            }
        }
        if let Some(kind) = observed.error {
            mark_error(&mut self.event, kind);
        }
        if self.cancellation.is_cancelled() && self.event.status != AuditStatus::Success {
            self.event.status = AuditStatus::Cancelled;
            self.event.reason = Some(AuditReason::Cancelled);
            self.event.phase = Some("command_cancelled".into());
        }
        drop(observed);
        self.finished = true;
        let _ = self.sink.emit(&self.event);
    }
}
impl Drop for OperationAudit {
    fn drop(&mut self) {
        if !self.finished {
            self.event.status = AuditStatus::Failed;
            self.event.reason = Some(if self.cancellation.is_cancelled() {
                AuditReason::Cancelled
            } else {
                AuditReason::InternalError
            });
            self.event.phase = Some("command_interrupted".into());
            let _ = self.sink.emit(&self.event);
        }
    }
}
/// Delivery completion is separate from filesystem execution/preparation.
pub struct WsDelivery {
    event: AuditEvent,
    sink: Arc<JsonlAuditSink>,
    finished: bool,
}
impl WsDelivery {
    pub fn new(mut event: AuditEvent, sink: Arc<JsonlAuditSink>) -> Self {
        event.bytes_out = 0;
        Self {
            event,
            sink,
            finished: false,
        }
    }
    pub fn frame(&mut self, frame: &TdrvFrame) {
        if frame.header.message_type == MessageType::DownloadChunk {
            if let Ok(chunk) = crate::protocol::chunk::RawChunk::decode(&frame.payload) {
                self.event.bytes_out += chunk.chunk_bytes.len() as u64;
            }
        }
    }
    pub fn finish(&mut self) {
        self.finished = true;
        self.event.status = AuditStatus::Success;
        self.event.phase = Some("transfer_completed".into());
        let _ = self.sink.emit(&self.event);
    }
}
impl Drop for WsDelivery {
    fn drop(&mut self) {
        if !self.finished {
            self.event.status = AuditStatus::Failed;
            self.event.reason = Some(AuditReason::ConnectionClosed);
            self.event.phase = Some("transfer_interrupted".into());
            let _ = self.sink.emit(&self.event);
        }
    }
}
