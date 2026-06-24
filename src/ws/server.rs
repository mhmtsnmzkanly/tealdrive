use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::Mutex;
use tokio::time::timeout;
use uuid::Uuid;

use crate::audit::event::{AuditEvent, AuditReason};
use crate::audit::sink::JsonlAuditSink;
use crate::config::AppConfig;
use crate::download::transfer::DownloadHelper;
use crate::errors::{ErrorKind, TealDriveError};
use crate::fs::create_folder::CreateFolderHelper;
use crate::fs::listing::DirectoryListingHelper;
use crate::fs::metadata::MetadataHelper;
use crate::fs::move_copy::{CopyHelper, MoveHelper};
use crate::fs::rename::RenameHelper;
use crate::fs::text::TextPreviewHelper;
use crate::protocol::frame::{RequestId, TdrvFrame};
use crate::protocol::message_type::MessageType;
use crate::session::store::{InMemorySessionStore, SessionId, SessionStore};
use crate::ws::connection::WsConnectionContext;
use crate::ws::dispatch::{
    error_frame, handle_decoded_frame_with_service, operation_failed_frame, DispatchDecision,
};
use crate::ws::handshake::create_server_hello_frame;
use crate::ws::upgrade::{accept_session_bound_upgrade, WsUpgradeRequest};

pub trait WsHelperBackend:
    DirectoryListingHelper
    + MetadataHelper
    + DownloadHelper
    + TextPreviewHelper
    + CreateFolderHelper
    + RenameHelper
    + MoveHelper
    + CopyHelper
    + Send
    + Sync
{
}

impl<T> WsHelperBackend for T where
    T: DirectoryListingHelper
        + MetadataHelper
        + DownloadHelper
        + TextPreviewHelper
        + CreateFolderHelper
        + RenameHelper
        + MoveHelper
        + CopyHelper
        + Send
        + Sync
{
}

#[derive(Debug)]
pub struct WsServerState<H> {
    pub config: Arc<AppConfig>,
    pub session_store: Arc<Mutex<InMemorySessionStore>>,
    pub helper_client: Arc<H>,
    pub audit_sink: Arc<JsonlAuditSink>,
}

impl<H> WsServerState<H> {
    pub fn new(
        config: AppConfig,
        session_store: InMemorySessionStore,
        helper_client: H,
        audit_sink: JsonlAuditSink,
    ) -> Self {
        Self {
            config: Arc::new(config),
            session_store: Arc::new(Mutex::new(session_store)),
            helper_client: Arc::new(helper_client),
            audit_sink: Arc::new(audit_sink),
        }
    }
}

impl<H> Clone for WsServerState<H> {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            session_store: self.session_store.clone(),
            helper_client: self.helper_client.clone(),
            audit_sink: self.audit_sink.clone(),
        }
    }
}

pub fn build_router<H>(state: WsServerState<H>) -> Router
where
    H: WsHelperBackend + Clone + 'static,
{
    Router::new()
        .route("/", get(health_check))
        .route("/ws", get(websocket_upgrade::<H>))
        .with_state(state)
}

pub async fn run_server<H>(
    bind_addr: SocketAddr,
    state: WsServerState<H>,
) -> Result<(), TealDriveError>
where
    H: WsHelperBackend + Clone + 'static,
{
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .map_err(|_| TealDriveError::Internal)?;
    let app = build_router(state);
    axum::serve(listener, app.into_make_service())
        .await
        .map_err(|_| TealDriveError::Internal)
}

async fn health_check() -> &'static str {
    "ok"
}

async fn websocket_upgrade<H>(
    ws: WebSocketUpgrade,
    State(state): State<WsServerState<H>>,
    headers: HeaderMap,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    let now = InMemorySessionStore::now_epoch_seconds();
    let upgrade_request = match build_upgrade_request(&state, &headers, now) {
        Ok(request) => request,
        Err(error) => {
            let _ = state.audit_sink.emit(&AuditEvent::websocket_rejection(
                RequestId::new(),
                "unknown",
                0,
                0,
                audit_reason_for_upgrade_error(&error),
            ));
            return map_upgrade_error(error).into_response();
        }
    };

    let context = {
        let store = state.session_store.lock().await;
        match accept_session_bound_upgrade(
            &*store,
            &state.config.security,
            state.config.features.clone(),
            upgrade_request,
        ) {
            Ok(context) => context,
            Err(error) => {
                let _ = state.audit_sink.emit(&AuditEvent::websocket_rejection(
                    RequestId::new(),
                    "unknown",
                    0,
                    0,
                    audit_reason_for_upgrade_error(&error),
                ));
                return map_upgrade_error(error).into_response();
            }
        }
    };

    {
        let mut store = state.session_store.lock().await;
        let _ = store.touch_session(&context.session_id, now);
    }

    let audit_sink = state.audit_sink.clone();
    let config = state.config.clone();
    let helper_client = state.helper_client.clone();
    let session_store = state.session_store.clone();
    ws.on_upgrade(move |socket| async move {
        let _ = handle_socket_connection(
            socket,
            config,
            session_store,
            helper_client,
            audit_sink,
            context,
        )
        .await;
    })
    .into_response()
}

fn build_upgrade_request<H>(
    state: &WsServerState<H>,
    headers: &HeaderMap,
    now: u64,
) -> Result<WsUpgradeRequest, TealDriveError> {
    let session_id = extract_session_id(headers, &state.config.session.cookie_name)?;
    Ok(WsUpgradeRequest {
        session_id: Some(session_id),
        origin: headers
            .get(axum::http::header::ORIGIN)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned),
        user_agent: headers
            .get(axum::http::header::USER_AGENT)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned),
        ip: extract_client_ip(headers),
        now,
    })
}

fn extract_session_id(headers: &HeaderMap, cookie_name: &str) -> Result<SessionId, TealDriveError> {
    let cookie_header = headers
        .get(axum::http::header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .ok_or(TealDriveError::PermissionDenied)?;

    for part in cookie_header.split(';') {
        let (name, value) = match part.trim().split_once('=') {
            Some(pair) => pair,
            None => continue,
        };
        if name == cookie_name {
            let parsed =
                Uuid::parse_str(value.trim()).map_err(|_| TealDriveError::PermissionDenied)?;
            return Ok(SessionId(parsed));
        }
    }

    Err(TealDriveError::PermissionDenied)
}

fn extract_client_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        })
}

fn map_upgrade_error(error: TealDriveError) -> (StatusCode, &'static str) {
    match error {
        TealDriveError::Validation => (StatusCode::BAD_REQUEST, "invalid websocket request"),
        TealDriveError::PermissionDenied => (StatusCode::UNAUTHORIZED, "permission denied"),
        TealDriveError::UnsupportedVersion => (StatusCode::BAD_REQUEST, "unsupported version"),
        TealDriveError::InvalidEncoding => (StatusCode::BAD_REQUEST, "invalid encoding"),
        TealDriveError::InvalidCompression => (StatusCode::BAD_REQUEST, "invalid compression"),
        _ => (StatusCode::BAD_REQUEST, "websocket upgrade rejected"),
    }
}

fn audit_reason_for_upgrade_error(error: &TealDriveError) -> AuditReason {
    match error {
        TealDriveError::Validation => AuditReason::HandshakeFailed,
        TealDriveError::PermissionDenied => AuditReason::HandshakeFailed,
        _ => AuditReason::HandshakeFailed,
    }
}

async fn handle_socket_connection<H>(
    socket: WebSocket,
    config: Arc<AppConfig>,
    session_store: Arc<Mutex<InMemorySessionStore>>,
    helper_client: Arc<H>,
    audit_sink: Arc<JsonlAuditSink>,
    mut context: WsConnectionContext,
) -> Result<(), TealDriveError>
where
    H: WsHelperBackend + 'static,
{
    let (mut sender, mut receiver) = socket.split();
    let server_hello = create_server_hello_frame(&config)?;
    send_frame(&mut sender, &server_hello).await?;

    let _ = audit_sink.emit(&AuditEvent::websocket_connect(
        RequestId::new(),
        context.username.clone(),
        context.uid,
        context.gid,
    ));

    let idle_timeout = Duration::from_secs(config.session.idle_timeout_seconds);
    let mut handshake_completed = false;

    loop {
        let message = match timeout(idle_timeout, receiver.next()).await {
            Ok(Some(message)) => message,
            Ok(None) => break,
            Err(_) => break,
        };

        let message = match message {
            Ok(message) => message,
            Err(_) => {
                break;
            }
        };

        let now = InMemorySessionStore::now_epoch_seconds();
        match message {
            Message::Binary(bytes) => {
                let frame = match TdrvFrame::decode(&bytes) {
                    Ok(frame) => frame,
                    Err(_) => {
                        let error = error_frame(RequestId::new(), "Malformed websocket frame.")?;
                        send_frame(&mut sender, &error).await?;
                        break;
                    }
                };

                let request_id = frame.header.request_id;
                match handle_decoded_frame_with_service(
                    &mut context,
                    frame.clone(),
                    &config,
                    helper_client.as_ref(),
                ) {
                    Ok(DispatchDecision::Outbound(frames)) => {
                        for outbound in frames {
                            send_frame(&mut sender, &outbound).await?;
                        }
                        if context.protocol_ready && !handshake_completed {
                            handshake_completed = true;
                            let _ = audit_sink.emit(&AuditEvent::websocket_handshake_success(
                                RequestId::new(),
                                context.username.clone(),
                                context.uid,
                                context.gid,
                            ));
                        }
                        let mut store = session_store.lock().await;
                        let _ = store.touch_session(&context.session_id, now);
                    }
                    Ok(DispatchDecision::DispatchPlaceholder(frame)) => {
                        send_frame(&mut sender, &frame).await?;
                    }
                    Err(error) if is_protocol_error(&error) => {
                        let response = error_frame(request_id, protocol_error_message(&error))?;
                        send_frame(&mut sender, &response).await?;
                        let _ = audit_sink.emit(&AuditEvent::websocket_rejection(
                            RequestId::new(),
                            context.username.clone(),
                            context.uid,
                            context.gid,
                            audit_reason_for_socket_error(&error),
                        ));
                        break;
                    }
                    Err(error) => {
                        let response = operation_failed_response(
                            request_id,
                            frame.header.message_type,
                            &error,
                        )?;
                        send_frame(&mut sender, &response).await?;
                        let _ = audit_sink.emit(&AuditEvent::websocket_rejection(
                            RequestId::new(),
                            context.username.clone(),
                            context.uid,
                            context.gid,
                            audit_reason_for_socket_error(&error),
                        ));
                    }
                }
            }
            Message::Ping(payload) => {
                sender
                    .send(Message::Pong(payload))
                    .await
                    .map_err(|_| TealDriveError::Internal)?;
            }
            Message::Pong(_) => {
                let mut store = session_store.lock().await;
                let _ = store.touch_session(&context.session_id, now);
            }
            Message::Close(_) => {
                break;
            }
            Message::Text(_) => {
                let response = error_frame(RequestId::new(), "Binary TDRV/1 frames are required.")?;
                send_frame(&mut sender, &response).await?;
                break;
            }
        }
    }

    let _ = audit_sink.emit(&AuditEvent::websocket_disconnect(
        RequestId::new(),
        context.username,
        context.uid,
        context.gid,
    ));
    let _ = sender.send(Message::Close(None)).await;
    Ok(())
}

async fn send_frame(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    frame: &TdrvFrame,
) -> Result<(), TealDriveError> {
    let bytes = frame.encode()?;
    sender
        .send(Message::Binary(bytes))
        .await
        .map_err(|_| TealDriveError::Internal)
}

fn is_protocol_error(error: &TealDriveError) -> bool {
    matches!(
        error,
        TealDriveError::Validation
            | TealDriveError::Protocol
            | TealDriveError::InvalidMagic
            | TealDriveError::UnsupportedVersion
            | TealDriveError::InvalidHeaderLength
            | TealDriveError::PayloadTooLarge
            | TealDriveError::UnknownMessageType
            | TealDriveError::InvalidMessageDirection
            | TealDriveError::InvalidFlags
            | TealDriveError::InvalidEncoding
            | TealDriveError::InvalidCompression
            | TealDriveError::PayloadLengthMismatch
            | TealDriveError::InvalidChunk
    )
}

fn protocol_error_message(error: &TealDriveError) -> &'static str {
    match error {
        TealDriveError::Validation => "Validation failed.",
        TealDriveError::Protocol => "Protocol error.",
        TealDriveError::InvalidMagic => "Invalid TDRV magic.",
        TealDriveError::UnsupportedVersion => "Unsupported TDRV version.",
        TealDriveError::InvalidHeaderLength => "Invalid header length.",
        TealDriveError::PayloadTooLarge => "Payload too large.",
        TealDriveError::UnknownMessageType => "Unknown message type.",
        TealDriveError::InvalidMessageDirection => "Invalid message direction.",
        TealDriveError::InvalidFlags => "Invalid flags.",
        TealDriveError::InvalidEncoding => "Invalid encoding.",
        TealDriveError::InvalidCompression => "Invalid compression.",
        TealDriveError::PayloadLengthMismatch => "Payload length mismatch.",
        TealDriveError::InvalidChunk => "Invalid chunk.",
        _ => "Protocol error.",
    }
}

fn operation_failed_response(
    request_id: RequestId,
    message_type: MessageType,
    error: &TealDriveError,
) -> Result<TdrvFrame, TealDriveError> {
    let (error_kind, safe_message) = match error {
        TealDriveError::PermissionDenied => (ErrorKind::PermissionDenied, "Permission denied."),
        TealDriveError::PolicyDenied => (ErrorKind::PolicyDenied, "Operation denied by policy."),
        TealDriveError::FeatureDisabled => (ErrorKind::FeatureDisabled, "Feature disabled."),
        TealDriveError::NotFound => (ErrorKind::NotFound, "Item not found."),
        TealDriveError::InvalidPath => (ErrorKind::InvalidPath, "Invalid path."),
        TealDriveError::AlreadyExists => (ErrorKind::AlreadyExists, "Target already exists."),
        TealDriveError::InvalidTarget => (ErrorKind::InvalidTarget, "Invalid target."),
        TealDriveError::CrossDeviceMove => (
            ErrorKind::CrossDeviceMove,
            "Cross-device move not supported.",
        ),
        TealDriveError::FileTooLarge => (ErrorKind::FileTooLarge, "File is too large."),
        TealDriveError::DiskFull => (ErrorKind::DiskFull, "Disk is full."),
        TealDriveError::UnsupportedBinaryFile => (
            ErrorKind::UnsupportedBinaryFile,
            "Binary file is not supported.",
        ),
        TealDriveError::InvalidTextEncoding => {
            (ErrorKind::InvalidTextEncoding, "Invalid text encoding.")
        }
        TealDriveError::InvalidFilename => (ErrorKind::NameTooLong, "Invalid filename."),
        TealDriveError::ReadOnlyRoot => (ErrorKind::ReadOnlyFilesystem, "Read-only filesystem."),
        TealDriveError::WebrootExecutableDenied => {
            (ErrorKind::PermissionDenied, "Executable upload denied.")
        }
        TealDriveError::HelperSpawnFailed
        | TealDriveError::HelperTimeout
        | TealDriveError::HelperStdinFailed
        | TealDriveError::HelperStdoutFailed
        | TealDriveError::HelperMalformedResponse
        | TealDriveError::HelperNonZeroExit
        | TealDriveError::HelperRequestTooLarge
        | TealDriveError::HelperResponseTooLarge
        | TealDriveError::HelperDecodeFailed
        | TealDriveError::HelperEncodeFailed
        | TealDriveError::HelperCommandNotImplemented
        | TealDriveError::Internal => (ErrorKind::InternalError, "Internal error."),
        _ => (ErrorKind::InternalError, "Operation failed."),
    };

    operation_failed_frame(request_id, message_type, error_kind, safe_message)
}

fn audit_reason_for_socket_error(error: &TealDriveError) -> AuditReason {
    match error {
        TealDriveError::Validation => AuditReason::OperationBeforeReady,
        TealDriveError::InvalidMessageDirection => AuditReason::InvalidMessageDirection,
        TealDriveError::UnsupportedVersion | TealDriveError::InvalidMagic => {
            AuditReason::HandshakeFailed
        }
        _ => AuditReason::HandshakeFailed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::sink::JsonlAuditSink;
    use crate::config::{AllowedRoot, RelativePath, RootId};
    use crate::helper::create_folder::create_folder_with_roots;
    use crate::helper::download::download_file_with_roots;
    use crate::helper::listing::list_directory_with_roots;
    use crate::helper::metadata::file_metadata_with_roots;
    use crate::helper::move_copy::{copy_file_with_roots, move_file_with_roots};
    use crate::helper::rename::rename_with_roots;
    use crate::helper::text::read_text_file_with_roots;
    use crate::helper::types::{HelperRequest, HelperResponse};
    use crate::protocol::header::{Encoding, TdrvHeader};
    use crate::protocol::message_type::MessageType;
    use crate::protocol::payload::ListDirectoryRequest;
    use crate::protocol::schema::{FileFilter, FilterMode, SortMode};
    use crate::ws::upgrade::tests::valid_context;
    use std::fs;
    use std::path::PathBuf;

    struct InProcessHelper {
        roots: Vec<AllowedRoot>,
    }

    impl DirectoryListingHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(list_directory_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl MetadataHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(file_metadata_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl DownloadHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(download_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl TextPreviewHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(read_text_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl CreateFolderHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(create_folder_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl RenameHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(rename_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl MoveHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(move_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl CopyHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(copy_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    fn temp_root() -> PathBuf {
        let path = std::env::temp_dir().join(format!("tealdrive-ws-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temp root");
        path
    }

    fn config_for(path: PathBuf) -> AppConfig {
        AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: path,
                read_only: false,
                uploads_allowed: true,
                hidden_files_allowed: false,
                is_web_root: false,
            }],
            ..AppConfig::default()
        }
    }

    fn list_request() -> ListDirectoryRequest {
        ListDirectoryRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(""),
            cursor: None,
            limit: 200,
            include_hidden: false,
            sort: SortMode::NameAsc,
            filter: FilterMode {
                query: None,
                file_kind: None,
                filter: Some(FileFilter::All),
            },
        }
    }

    #[test]
    fn session_cookie_extraction_works() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            "tealdrive_session=550e8400-e29b-41d4-a716-446655440000"
                .parse()
                .expect("cookie"),
        );

        let session_id = extract_session_id(&headers, "tealdrive_session").expect("session");

        assert_eq!(
            session_id,
            SessionId(Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").expect("uuid"))
        );
    }

    #[test]
    fn live_dispatch_routes_list_directory() {
        let base = temp_root();
        fs::write(base.join("visible.txt"), b"hello").expect("file");
        let config = config_for(base);
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let mut context = valid_context();
        context.protocol_ready = true;
        let frame = {
            let mut header = TdrvHeader::new(MessageType::ListDirectory, RequestId::new(), 0);
            header.encoding = Encoding::MessagePack;
            TdrvFrame::new(
                header,
                crate::protocol::codec::encode_msgpack(&list_request()).expect("payload"),
            )
            .expect("frame")
        };

        let decision = handle_decoded_frame_with_service(&mut context, frame, &config, &helper)
            .expect("dispatch");

        assert!(matches!(decision, DispatchDecision::Outbound(_)));
    }

    #[test]
    fn build_upgrade_request_requires_cookie() {
        let config = AppConfig::default();
        let state = WsServerState::new(
            config,
            InMemorySessionStore::new(crate::config::SessionConfig::default()),
            InProcessHelper { roots: vec![] },
            JsonlAuditSink::open(
                std::env::temp_dir()
                    .join(format!("tealdrive-audit-{}.jsonl", uuid::Uuid::new_v4())),
            )
            .expect("audit"),
        );
        let headers = HeaderMap::new();

        assert!(matches!(
            build_upgrade_request(&state, &headers, 1),
            Err(TealDriveError::PermissionDenied)
        ));
    }
}
