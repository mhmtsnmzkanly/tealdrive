use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::TealDriveError;
use crate::limits::MAX_FRAME_PAYLOAD;
use crate::protocol::header::TdrvHeader;
use crate::protocol::header::HEADER_LEN;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestId(pub Uuid);

impl RequestId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransferId(pub Uuid);

impl TransferId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl Default for TransferId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TdrvFrame {
    pub header: TdrvHeader,
    pub payload: Vec<u8>,
}

impl TdrvFrame {
    pub fn new(mut header: TdrvHeader, payload: Vec<u8>) -> Result<Self, TealDriveError> {
        if payload.len() > MAX_FRAME_PAYLOAD {
            return Err(TealDriveError::PayloadTooLarge);
        }
        header.payload_len =
            u32::try_from(payload.len()).map_err(|_| TealDriveError::PayloadTooLarge)?;
        Ok(Self { header, payload })
    }

    pub fn encode(&self) -> Result<Vec<u8>, TealDriveError> {
        if self.payload.len() > MAX_FRAME_PAYLOAD {
            return Err(TealDriveError::PayloadTooLarge);
        }
        if self.header.payload_len as usize != self.payload.len() {
            return Err(TealDriveError::PayloadLengthMismatch);
        }

        let mut out = Vec::with_capacity(HEADER_LEN + self.payload.len());
        out.extend_from_slice(&self.header.encode());
        out.extend_from_slice(&self.payload);
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, TealDriveError> {
        if bytes.len() < HEADER_LEN {
            return Err(TealDriveError::InvalidHeaderLength);
        }

        let header = TdrvHeader::decode(&bytes[..HEADER_LEN])?;
        let payload = bytes[HEADER_LEN..].to_vec();
        if header.payload_len as usize != payload.len() {
            return Err(TealDriveError::PayloadLengthMismatch);
        }
        if payload.len() > MAX_FRAME_PAYLOAD {
            return Err(TealDriveError::PayloadTooLarge);
        }

        Ok(Self { header, payload })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::MAX_FRAME_PAYLOAD;
    use crate::protocol::header::TdrvHeader;
    use crate::protocol::message_type::MessageType;

    #[test]
    fn frame_roundtrip() {
        let payload = b"hello".to_vec();
        let header = TdrvHeader::new(MessageType::ListDirectory, RequestId::new(), 0);
        let frame = TdrvFrame::new(header, payload).expect("valid frame");
        let encoded = frame.encode().expect("encoded frame");
        let decoded = TdrvFrame::decode(&encoded).expect("decoded frame");

        assert_eq!(decoded, frame);
    }

    #[test]
    fn payload_len_mismatch_rejected_on_encode() {
        let payload = b"hello".to_vec();
        let header = TdrvHeader::new(MessageType::ListDirectory, RequestId::new(), 99);
        let frame = TdrvFrame { header, payload };

        assert!(matches!(
            frame.encode(),
            Err(TealDriveError::PayloadLengthMismatch)
        ));
    }

    #[test]
    fn payload_len_mismatch_rejected_on_decode() {
        let payload = b"hello".to_vec();
        let header = TdrvHeader::new(MessageType::ListDirectory, RequestId::new(), 4);
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&header.encode());
        encoded.extend_from_slice(&payload);

        assert!(matches!(
            TdrvFrame::decode(&encoded),
            Err(TealDriveError::PayloadLengthMismatch)
        ));
    }

    #[test]
    fn oversized_payload_rejected() {
        let payload = vec![0_u8; MAX_FRAME_PAYLOAD + 1];
        let header = TdrvHeader::new(MessageType::ListDirectory, RequestId::new(), 0);

        assert!(matches!(
            TdrvFrame::new(header, payload),
            Err(TealDriveError::PayloadTooLarge)
        ));
    }
}
