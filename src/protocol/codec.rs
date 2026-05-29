use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::errors::TealDriveError;
use crate::limits::MAX_CONTROL_PAYLOAD;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessagePackControlPayload;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawBinaryPayload;

pub fn encode_msgpack<T: Serialize>(value: &T) -> Result<Vec<u8>, TealDriveError> {
    let bytes = rmp_serde::to_vec_named(value).map_err(|_| TealDriveError::Protocol)?;
    if bytes.len() > MAX_CONTROL_PAYLOAD {
        return Err(TealDriveError::PayloadTooLarge);
    }
    Ok(bytes)
}

pub fn decode_msgpack<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, TealDriveError> {
    if bytes.len() > MAX_CONTROL_PAYLOAD {
        return Err(TealDriveError::PayloadTooLarge);
    }
    rmp_serde::from_slice(bytes).map_err(|_| TealDriveError::Protocol)
}
