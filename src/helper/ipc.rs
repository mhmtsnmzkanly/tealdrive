use std::io::{Read, Write};

use crate::errors::TealDriveError;
use crate::helper::types::{command_disabled_in_v1, HelperRequest, HelperResponse};

pub const MAX_HELPER_REQUEST_SIZE: usize = 1024 * 1024;
pub const MAX_HELPER_RESPONSE_SIZE: usize = 1024 * 1024;
pub const HELPER_LEN_PREFIX_SIZE: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelperIpc;

pub fn encode_helper_request(request: &HelperRequest) -> Result<Vec<u8>, TealDriveError> {
    encode_length_prefixed(
        request,
        MAX_HELPER_REQUEST_SIZE,
        TealDriveError::HelperRequestTooLarge,
    )
}

pub fn decode_helper_request(bytes: &[u8]) -> Result<HelperRequest, TealDriveError> {
    decode_length_prefixed(
        bytes,
        MAX_HELPER_REQUEST_SIZE,
        TealDriveError::HelperRequestTooLarge,
    )
    .map_err(helper_decode_error)
}

pub fn encode_helper_response(response: &HelperResponse) -> Result<Vec<u8>, TealDriveError> {
    encode_length_prefixed(
        response,
        MAX_HELPER_RESPONSE_SIZE,
        TealDriveError::HelperResponseTooLarge,
    )
}

pub fn decode_helper_response(bytes: &[u8]) -> Result<HelperResponse, TealDriveError> {
    decode_length_prefixed(
        bytes,
        MAX_HELPER_RESPONSE_SIZE,
        TealDriveError::HelperResponseTooLarge,
    )
    .map_err(helper_decode_error)
}

pub fn read_helper_request<R: Read>(reader: &mut R) -> Result<HelperRequest, TealDriveError> {
    read_length_prefixed(
        reader,
        MAX_HELPER_REQUEST_SIZE,
        TealDriveError::HelperRequestTooLarge,
    )
    .and_then(|bytes| rmp_serde::from_slice(&bytes).map_err(|_| TealDriveError::HelperDecodeFailed))
}

pub fn write_helper_response<W: Write>(
    writer: &mut W,
    response: &HelperResponse,
) -> Result<(), TealDriveError> {
    let bytes = encode_helper_response(response)?;
    writer
        .write_all(&bytes)
        .map_err(|_| TealDriveError::HelperStdoutFailed)
}

pub fn read_helper_response<R: Read>(reader: &mut R) -> Result<HelperResponse, TealDriveError> {
    read_length_prefixed(
        reader,
        MAX_HELPER_RESPONSE_SIZE,
        TealDriveError::HelperResponseTooLarge,
    )
    .and_then(|bytes| rmp_serde::from_slice(&bytes).map_err(|_| TealDriveError::HelperDecodeFailed))
}

pub fn run_helper_once<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
) -> Result<(), TealDriveError> {
    let request = read_helper_request(reader)?;
    let response = handle_helper_request(&request);
    write_helper_response(writer, &response)
}

pub fn handle_helper_request(request: &HelperRequest) -> HelperResponse {
    if request.uid == 0 {
        return HelperResponse::permission_denied(request, "Helper refuses uid 0.");
    }
    if command_disabled_in_v1(request.command) {
        return HelperResponse::feature_disabled(request);
    }
    HelperResponse::not_implemented(request)
}

fn encode_length_prefixed<T: serde::Serialize>(
    value: &T,
    max_size: usize,
    too_large_error: TealDriveError,
) -> Result<Vec<u8>, TealDriveError> {
    let payload = rmp_serde::to_vec_named(value).map_err(|_| TealDriveError::HelperEncodeFailed)?;
    if payload.is_empty() {
        return Err(TealDriveError::HelperEncodeFailed);
    }
    if payload.len() > max_size {
        return Err(too_large_error);
    }
    let len = u32::try_from(payload.len()).map_err(|_| too_large_error)?;
    let mut frame = Vec::with_capacity(HELPER_LEN_PREFIX_SIZE + payload.len());
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

fn decode_length_prefixed<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    max_size: usize,
    too_large_error: TealDriveError,
) -> Result<T, TealDriveError> {
    if bytes.len() < HELPER_LEN_PREFIX_SIZE {
        return Err(TealDriveError::HelperMalformedResponse);
    }
    let len = u32::from_be_bytes(bytes[0..4].try_into().expect("fixed length")) as usize;
    if len == 0 {
        return Err(TealDriveError::HelperMalformedResponse);
    }
    if len > max_size {
        return Err(too_large_error);
    }
    if bytes.len() - HELPER_LEN_PREFIX_SIZE != len {
        return Err(TealDriveError::HelperMalformedResponse);
    }
    rmp_serde::from_slice(&bytes[4..]).map_err(|_| TealDriveError::HelperDecodeFailed)
}

fn read_length_prefixed<R: Read>(
    reader: &mut R,
    max_size: usize,
    too_large_error: TealDriveError,
) -> Result<Vec<u8>, TealDriveError> {
    let mut len_bytes = [0_u8; HELPER_LEN_PREFIX_SIZE];
    reader
        .read_exact(&mut len_bytes)
        .map_err(|_| TealDriveError::HelperMalformedResponse)?;
    let len = u32::from_be_bytes(len_bytes) as usize;
    if len == 0 {
        return Err(TealDriveError::HelperMalformedResponse);
    }
    if len > max_size {
        return Err(too_large_error);
    }
    let mut payload = vec![0_u8; len];
    reader
        .read_exact(&mut payload)
        .map_err(|_| TealDriveError::HelperMalformedResponse)?;
    Ok(payload)
}

fn helper_decode_error(error: TealDriveError) -> TealDriveError {
    match error {
        TealDriveError::HelperRequestTooLarge => TealDriveError::HelperRequestTooLarge,
        TealDriveError::HelperResponseTooLarge => TealDriveError::HelperResponseTooLarge,
        TealDriveError::HelperMalformedResponse => TealDriveError::HelperMalformedResponse,
        _ => TealDriveError::HelperDecodeFailed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::ErrorKind;
    use crate::helper::ipc::tests_support::sample_request;
    use crate::helper::types::{HelperCommand, HelperResponse};

    #[test]
    fn helper_request_length_prefixed_roundtrip() {
        let request = sample_request(HelperCommand::ListDirectory);
        let encoded = encode_helper_request(&request).expect("encoded");
        let decoded = decode_helper_request(&encoded).expect("decoded");

        assert_eq!(decoded, request);
        assert_eq!(
            u32::from_be_bytes(encoded[0..4].try_into().unwrap()) as usize,
            encoded.len() - 4
        );
    }

    #[test]
    fn helper_response_length_prefixed_roundtrip() {
        let request = sample_request(HelperCommand::ListDirectory);
        let response = HelperResponse::not_implemented(&request);
        let encoded = encode_helper_response(&response).expect("encoded");
        let decoded = decode_helper_response(&encoded).expect("decoded");

        assert_eq!(decoded, response);
    }

    #[test]
    fn truncated_request_rejected() {
        assert_eq!(
            decode_helper_request(&[0, 0, 0, 10, 1, 2]),
            Err(TealDriveError::HelperMalformedResponse)
        );
    }

    #[test]
    fn oversized_request_rejected() {
        let len = (MAX_HELPER_REQUEST_SIZE as u32) + 1;
        assert_eq!(
            decode_helper_request(&len.to_be_bytes()),
            Err(TealDriveError::HelperRequestTooLarge)
        );
    }

    #[test]
    fn malformed_msgpack_rejected() {
        let mut frame = Vec::new();
        frame.extend_from_slice(&1_u32.to_be_bytes());
        frame.push(0xc1);

        assert_eq!(
            decode_helper_request(&frame),
            Err(TealDriveError::HelperDecodeFailed)
        );
    }

    #[test]
    fn big_endian_length_parsing() {
        let request = sample_request(HelperCommand::ReadFile);
        let encoded = encode_helper_request(&request).expect("encoded");
        let parsed = u32::from_be_bytes(encoded[0..4].try_into().unwrap()) as usize;

        assert_eq!(parsed, encoded.len() - 4);
    }

    #[test]
    fn uid_zero_request_rejected() {
        let mut request = sample_request(HelperCommand::ListDirectory);
        request.uid = 0;
        let response = handle_helper_request(&request);

        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::PermissionDenied));
    }

    #[test]
    fn unsupported_command_returns_not_implemented() {
        let request = sample_request(HelperCommand::ListDirectory);
        let response = handle_helper_request(&request);

        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::NotImplemented));
    }

    #[test]
    fn disabled_command_returns_feature_disabled() {
        let request = sample_request(HelperCommand::ExtractArchive);
        let response = handle_helper_request(&request);

        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::FeatureDisabled));
    }

    #[test]
    fn run_helper_once_writes_one_response() {
        let request = sample_request(HelperCommand::ListDirectory);
        let input = encode_helper_request(&request).expect("request");
        let mut output = Vec::new();

        run_helper_once(&mut input.as_slice(), &mut output).expect("run");
        let response = decode_helper_response(&output).expect("response");

        assert_eq!(response.error_kind, Some(ErrorKind::NotImplemented));
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    use crate::config::{RelativePath, RootId};
    use crate::helper::types::{HelperCommand, HelperLimits, HelperPolicyContext, HelperRequest};
    use crate::protocol::frame::RequestId;

    pub fn sample_request(command: HelperCommand) -> HelperRequest {
        HelperRequest {
            request_id: RequestId::new(),
            username: "alice".to_owned(),
            uid: 1000,
            gid: 1000,
            command,
            root_id: RootId::new("home"),
            relative_path: RelativePath::new("docs"),
            target_relative_path: None,
            policy_context: HelperPolicyContext {
                read_only_root: false,
                is_web_root: false,
            },
            limits: HelperLimits {
                max_directory_page_size: 500,
                max_text_edit_size: 5 * 1024 * 1024,
            },
        }
    }
}
