use crate::config::AppConfig;
use crate::errors::TealDriveError;
use crate::limits::Limits;
use crate::protocol::codec::{decode_msgpack, encode_msgpack};
use crate::protocol::compression::Compression;
use crate::protocol::frame::{RequestId, TdrvFrame};
use crate::protocol::header::{Encoding, TdrvHeader};
use crate::protocol::message_type::MessageType;
use crate::protocol::payload::{ClientHelloPayload, ProtocolReadyPayload, ServerHelloPayload};
use crate::ws::connection::{WsConnectionContext, WsSelectedProtocol};

pub const TDRV_PROTOCOL_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeState {
    WaitingForClientHello,
    Ready,
}

pub fn create_server_hello_frame(config: &AppConfig) -> Result<TdrvFrame, TealDriveError> {
    let payload = ServerHelloPayload {
        protocol_version: TDRV_PROTOCOL_VERSION,
        server_name: "TealDrive".to_owned(),
        selected_encoding: Some(Encoding::MessagePack),
        supported_encodings: vec![Encoding::MessagePack, Encoding::RawBinary],
        supported_compressions: server_supported_compressions(),
        max_frame_size: config.limits.max_frame_payload,
        max_control_payload_size: config.limits.max_control_payload,
        max_chunk_size: config.limits.max_chunk_size,
        feature_flags: config.features.clone(),
    };
    let bytes = encode_msgpack(&payload)?;
    let mut header = TdrvHeader::new(MessageType::ServerHello, RequestId::new(), 0);
    header.encoding = Encoding::MessagePack;
    TdrvFrame::new(header, bytes)
}

pub fn decode_client_hello(frame: &TdrvFrame) -> Result<ClientHelloPayload, TealDriveError> {
    if frame.header.message_type != MessageType::ClientHello {
        return Err(TealDriveError::InvalidMessageDirection);
    }
    if frame.header.encoding != Encoding::MessagePack {
        return Err(TealDriveError::InvalidEncoding);
    }
    decode_msgpack(&frame.payload)
}

pub fn validate_client_hello(
    payload: &ClientHelloPayload,
    server_limits: &Limits,
) -> Result<WsSelectedProtocol, TealDriveError> {
    if payload.protocol_version != TDRV_PROTOCOL_VERSION {
        return Err(TealDriveError::UnsupportedVersion);
    }
    if !payload.supported_encodings.contains(&Encoding::MessagePack) {
        return Err(TealDriveError::InvalidEncoding);
    }

    payload.validate(server_limits)?;

    let selected_compression = select_compression(&payload.supported_compressions)?;
    Ok(WsSelectedProtocol {
        protocol_version: TDRV_PROTOCOL_VERSION,
        selected_encoding: Encoding::MessagePack,
        selected_compression,
        effective_limits: *server_limits,
    })
}

pub fn create_protocol_ready_frame(
    selected: &WsSelectedProtocol,
) -> Result<TdrvFrame, TealDriveError> {
    let payload = ProtocolReadyPayload {
        protocol_version: TDRV_PROTOCOL_VERSION,
        selected_encoding: selected.selected_encoding,
        selected_compression: selected.selected_compression,
        limits: selected.effective_limits,
        server_time: None,
    };
    let bytes = encode_msgpack(&payload)?;
    let mut header = TdrvHeader::new(MessageType::ProtocolReady, RequestId::new(), 0);
    header.encoding = Encoding::MessagePack;
    TdrvFrame::new(header, bytes)
}

pub fn apply_protocol_ready(context: &mut WsConnectionContext, selected: WsSelectedProtocol) {
    context.selected_protocol = Some(selected);
    context.protocol_ready = true;
}

fn select_compression(client: &[Compression]) -> Result<Compression, TealDriveError> {
    if client.contains(&Compression::None) {
        Ok(Compression::None)
    } else if client.contains(&Compression::Gzip) {
        Ok(Compression::Gzip)
    } else {
        Err(TealDriveError::InvalidCompression)
    }
}

pub fn server_supported_compressions() -> Vec<Compression> {
    vec![Compression::None, Compression::Gzip]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::limits::MAX_CHUNK_SIZE;
    use crate::protocol::codec::encode_msgpack;

    fn client_hello() -> ClientHelloPayload {
        let limits = Limits::default();
        ClientHelloPayload {
            protocol_version: 1,
            supported_encodings: vec![Encoding::MessagePack, Encoding::RawBinary],
            supported_compressions: vec![Compression::None],
            max_frame_size: limits.max_frame_payload,
            max_control_payload_size: limits.max_control_payload,
            max_chunk_size: limits.max_chunk_size,
            feature_flags: Default::default(),
        }
    }

    #[test]
    fn server_hello_frame_creation() {
        let frame = create_server_hello_frame(&AppConfig::default()).expect("server hello");

        assert_eq!(frame.header.message_type, MessageType::ServerHello);
        assert_eq!(frame.header.encoding, Encoding::MessagePack);
    }

    #[test]
    fn client_hello_decode_and_validate_success() {
        let payload = client_hello();
        let bytes = encode_msgpack(&payload).expect("encoded");
        let header = TdrvHeader::new(MessageType::ClientHello, RequestId::new(), 0);
        let frame = TdrvFrame::new(header, bytes).expect("frame");
        let decoded = decode_client_hello(&frame).expect("decoded");
        let selected = validate_client_hello(&decoded, &Limits::default()).expect("valid");

        assert_eq!(selected.selected_encoding, Encoding::MessagePack);
        assert_eq!(selected.selected_compression, Compression::None);
    }

    #[test]
    fn client_hello_unsupported_version_rejected() {
        let mut payload = client_hello();
        payload.protocol_version = 2;

        assert!(matches!(
            validate_client_hello(&payload, &Limits::default()),
            Err(TealDriveError::UnsupportedVersion)
        ));
    }

    #[test]
    fn protocol_ready_creation() {
        let selected = validate_client_hello(&client_hello(), &Limits::default()).expect("valid");
        let frame = create_protocol_ready_frame(&selected).expect("ready");

        assert_eq!(frame.header.message_type, MessageType::ProtocolReady);
        assert_eq!(frame.header.encoding, Encoding::MessagePack);
    }

    #[test]
    fn protocol_ready_state_transition() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let selected = validate_client_hello(&client_hello(), &Limits::default()).expect("valid");

        apply_protocol_ready(&mut context, selected);

        assert!(context.protocol_ready);
        assert!(context.selected_protocol.is_some());
    }

    #[test]
    fn client_limits_above_server_limits_rejected() {
        let mut payload = client_hello();
        payload.max_chunk_size = MAX_CHUNK_SIZE + 1;

        assert!(matches!(
            validate_client_hello(&payload, &Limits::default()),
            Err(TealDriveError::Validation)
        ));
    }

    #[test]
    fn messagepack_required() {
        let mut payload = client_hello();
        payload.supported_encodings = vec![Encoding::RawBinary];

        assert!(matches!(
            validate_client_hello(&payload, &Limits::default()),
            Err(TealDriveError::InvalidEncoding)
        ));
    }
}
