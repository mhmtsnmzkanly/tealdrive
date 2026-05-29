use crate::errors::{ErrorKind, ErrorPayload, TealDriveError};
use crate::policy::feature_gate::{FeatureGate, FeatureGateResult};
use crate::protocol::codec::encode_msgpack;
use crate::protocol::frame::{RequestId, TdrvFrame};
use crate::protocol::header::{Encoding, TdrvHeader};
use crate::protocol::message_type::MessageType;
use crate::protocol::payload::{ClientHelloPayload, OperationFailedPayload};
use crate::ws::connection::{classify_message, InboundFrameKind, WsConnectionContext};
use crate::ws::handshake::{
    apply_protocol_ready, create_protocol_ready_frame, decode_client_hello, validate_client_hello,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchDecision {
    Outbound(Vec<TdrvFrame>),
    DispatchPlaceholder(TdrvFrame),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchSkeleton;

pub fn handle_inbound_frame(
    context: &mut WsConnectionContext,
    bytes: &[u8],
) -> Result<DispatchDecision, TealDriveError> {
    let frame = TdrvFrame::decode(bytes)?;
    frame.header.message_type.validate_inbound_client()?;

    match classify_message(frame.header.message_type) {
        InboundFrameKind::Handshake(MessageType::ClientHello) => {
            handle_client_hello(context, &frame)
        }
        InboundFrameKind::Handshake(_) => Err(TealDriveError::InvalidMessageDirection),
        InboundFrameKind::Operation(_) if !context.protocol_ready => {
            Err(TealDriveError::Validation)
        }
        InboundFrameKind::Operation(message_type) => {
            placeholder_dispatch(context, frame, message_type)
        }
        InboundFrameKind::Protocol(_) => {
            Ok(DispatchDecision::Outbound(vec![connection_status_frame(
                context,
                "connected",
            )?]))
        }
    }
}

fn handle_client_hello(
    context: &mut WsConnectionContext,
    frame: &TdrvFrame,
) -> Result<DispatchDecision, TealDriveError> {
    let payload: ClientHelloPayload = decode_client_hello(frame)?;
    let limits = context
        .selected_protocol
        .as_ref()
        .map_or_else(Default::default, |selected| selected.effective_limits);
    let selected = validate_client_hello(&payload, &limits)?;
    let ready = create_protocol_ready_frame(&selected)?;
    apply_protocol_ready(context, selected);
    Ok(DispatchDecision::Outbound(vec![ready]))
}

fn placeholder_dispatch(
    context: &WsConnectionContext,
    frame: TdrvFrame,
    message_type: MessageType,
) -> Result<DispatchDecision, TealDriveError> {
    match FeatureGate::check(message_type, &context.features) {
        FeatureGateResult::Disabled { .. } => {
            Ok(DispatchDecision::Outbound(vec![operation_failed_frame(
                frame.header.request_id,
                message_type,
                ErrorKind::FeatureDisabled,
                "This operation is disabled in TealDrive V1.",
            )?]))
        }
        FeatureGateResult::Allowed => Ok(DispatchDecision::Outbound(vec![operation_failed_frame(
            frame.header.request_id,
            message_type,
            ErrorKind::NotImplemented,
            "This operation is not implemented yet.",
        )?])),
    }
}

pub fn error_frame(request_id: RequestId, message: &str) -> Result<TdrvFrame, TealDriveError> {
    let payload = ErrorPayload {
        code: "PROTOCOL_ERROR".to_owned(),
        error_kind: ErrorKind::ProtocolError,
        safe_message: message.to_owned(),
        request_id,
        operation: None,
        retryable: false,
        policy_reason: None,
        debug_ref: None,
    };
    let bytes = encode_msgpack(&payload)?;
    let mut header = TdrvHeader::new(MessageType::Error, request_id, 0);
    header.encoding = Encoding::MessagePack;
    TdrvFrame::new(header, bytes)
}

pub fn operation_failed_frame(
    request_id: RequestId,
    message_type: MessageType,
    error_kind: ErrorKind,
    safe_message: &str,
) -> Result<TdrvFrame, TealDriveError> {
    let payload = OperationFailedPayload {
        request_id,
        operation: format!("{message_type:?}"),
        error: ErrorPayload {
            code: format!("{error_kind:?}").to_uppercase(),
            error_kind,
            safe_message: safe_message.to_owned(),
            request_id,
            operation: Some(format!("{message_type:?}")),
            retryable: false,
            policy_reason: None,
            debug_ref: None,
        },
    };
    let bytes = encode_msgpack(&payload)?;
    let mut header = TdrvHeader::new(MessageType::OperationFailed, request_id, 0);
    header.encoding = Encoding::MessagePack;
    TdrvFrame::new(header, bytes)
}

pub fn connection_status_frame(
    context: &WsConnectionContext,
    message: &str,
) -> Result<TdrvFrame, TealDriveError> {
    let payload = crate::protocol::payload::ConnectionStatusPayload {
        protocol_ready: context.protocol_ready,
        message: Some(message.to_owned()),
    };
    let bytes = encode_msgpack(&payload)?;
    let mut header = TdrvHeader::new(MessageType::ConnectionStatus, RequestId::new(), 0);
    header.encoding = Encoding::MessagePack;
    TdrvFrame::new(header, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::Limits;
    use crate::protocol::compression::Compression;
    use crate::protocol::flags::FLAG_COMPRESSED;
    use crate::protocol::header::{TDRV_MAGIC, TDRV_VERSION};
    use crate::protocol::payload::ListDirectoryRequest;
    use crate::protocol::schema::{FilterMode, SortMode};
    use crate::{config::RelativePath, config::RootId};

    fn client_frame(message_type: MessageType, payload: Vec<u8>) -> TdrvFrame {
        let mut header = TdrvHeader::new(message_type, RequestId::new(), 0);
        header.encoding = Encoding::MessagePack;
        TdrvFrame::new(header, payload).expect("frame")
    }

    fn client_hello_frame() -> Vec<u8> {
        let limits = Limits::default();
        let payload = ClientHelloPayload {
            protocol_version: 1,
            supported_encodings: vec![Encoding::MessagePack],
            supported_compressions: vec![Compression::None],
            max_frame_size: limits.max_frame_payload,
            max_control_payload_size: limits.max_control_payload,
            max_chunk_size: limits.max_chunk_size,
            feature_flags: Default::default(),
        };
        client_frame(
            MessageType::ClientHello,
            encode_msgpack(&payload).expect("payload"),
        )
        .encode()
        .expect("encoded")
    }

    #[test]
    fn client_sent_server_hello_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let frame = client_frame(MessageType::ServerHello, Vec::new())
            .encode()
            .expect("encoded");

        assert!(matches!(
            handle_inbound_frame(&mut context, &frame),
            Err(TealDriveError::InvalidMessageDirection)
        ));
    }

    #[test]
    fn client_sent_directory_list_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let frame = client_frame(MessageType::DirectoryList, Vec::new())
            .encode()
            .expect("encoded");

        assert!(matches!(
            handle_inbound_frame(&mut context, &frame),
            Err(TealDriveError::InvalidMessageDirection)
        ));
    }

    #[test]
    fn list_directory_before_protocol_ready_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let payload = ListDirectoryRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(""),
            cursor: None,
            limit: 200,
            include_hidden: false,
            sort: SortMode::NameAsc,
            filter: FilterMode::default(),
        };
        let frame = client_frame(
            MessageType::ListDirectory,
            encode_msgpack(&payload).expect("payload"),
        )
        .encode()
        .expect("encoded");

        assert!(matches!(
            handle_inbound_frame(&mut context, &frame),
            Err(TealDriveError::Validation)
        ));
    }

    #[test]
    fn client_hello_moves_connection_to_ready() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let decision = handle_inbound_frame(&mut context, &client_hello_frame()).expect("decision");

        assert!(context.protocol_ready);
        assert!(
            matches!(decision, DispatchDecision::Outbound(frames) if frames[0].header.message_type == MessageType::ProtocolReady)
        );
    }

    #[test]
    fn invalid_magic_frame_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let mut bytes = client_hello_frame();
        bytes[0..4].copy_from_slice(b"NOPE");

        assert!(matches!(
            handle_inbound_frame(&mut context, &bytes),
            Err(TealDriveError::InvalidMagic)
        ));
    }

    #[test]
    fn unsupported_encoding_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let mut bytes = [0_u8; crate::protocol::header::HEADER_LEN].to_vec();
        bytes[0..4].copy_from_slice(&TDRV_MAGIC);
        bytes[4] = TDRV_VERSION;
        bytes[7..9].copy_from_slice(&u16::from(MessageType::ClientHello).to_be_bytes());
        bytes[10] = 9;

        assert!(matches!(
            handle_inbound_frame(&mut context, &bytes),
            Err(TealDriveError::InvalidEncoding)
        ));
    }

    #[test]
    fn unsupported_compression_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let mut bytes = client_hello_frame();
        bytes[5..7].copy_from_slice(&FLAG_COMPRESSED.to_be_bytes());
        bytes[9] = u8::from(Compression::ZstdFuture);

        assert!(matches!(
            handle_inbound_frame(&mut context, &bytes),
            Err(TealDriveError::InvalidCompression)
        ));
    }

    #[test]
    fn payload_too_large_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let mut bytes = client_hello_frame();
        let too_large = (crate::limits::MAX_FRAME_PAYLOAD as u32) + 1;
        bytes[28..32].copy_from_slice(&too_large.to_be_bytes());

        assert!(matches!(
            handle_inbound_frame(&mut context, &bytes),
            Err(TealDriveError::PayloadTooLarge)
        ));
    }
}
