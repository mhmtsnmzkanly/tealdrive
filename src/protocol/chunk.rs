use serde::{Deserialize, Serialize};

use crate::errors::TealDriveError;
use crate::limits::MAX_CHUNK_SIZE;
use crate::protocol::frame::TransferId;

pub const RAW_CHUNK_HEADER_LEN: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawChunkHeader {
    pub transfer_id: TransferId,
    pub chunk_index: u32,
    pub offset: u64,
    pub chunk_len: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawChunk {
    pub transfer_id: TransferId,
    pub chunk_index: u32,
    pub offset: u64,
    pub chunk_bytes: Vec<u8>,
}

impl RawChunk {
    pub fn decode(bytes: &[u8]) -> Result<Self, TealDriveError> {
        if bytes.len() < RAW_CHUNK_HEADER_LEN {
            return Err(TealDriveError::InvalidChunk);
        }

        let transfer_id = TransferId::from_bytes(bytes[0..16].try_into().expect("fixed slice len"));
        let chunk_index = u32::from_be_bytes(bytes[16..20].try_into().expect("fixed slice len"));
        let offset = u64::from_be_bytes(bytes[20..28].try_into().expect("fixed slice len"));
        let chunk_len = u32::from_be_bytes(bytes[28..32].try_into().expect("fixed slice len"));
        let chunk_len_usize = chunk_len as usize;

        if chunk_len_usize > MAX_CHUNK_SIZE {
            return Err(TealDriveError::InvalidChunk);
        }
        if bytes.len() - RAW_CHUNK_HEADER_LEN != chunk_len_usize {
            return Err(TealDriveError::InvalidChunk);
        }

        Ok(Self {
            transfer_id,
            chunk_index,
            offset,
            chunk_bytes: bytes[RAW_CHUNK_HEADER_LEN..].to_vec(),
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>, TealDriveError> {
        if self.chunk_bytes.len() > MAX_CHUNK_SIZE {
            return Err(TealDriveError::InvalidChunk);
        }

        let chunk_len =
            u32::try_from(self.chunk_bytes.len()).map_err(|_| TealDriveError::InvalidChunk)?;
        let mut out = Vec::with_capacity(RAW_CHUNK_HEADER_LEN + self.chunk_bytes.len());
        out.extend_from_slice(self.transfer_id.as_bytes());
        out.extend_from_slice(&self.chunk_index.to_be_bytes());
        out.extend_from_slice(&self.offset.to_be_bytes());
        out.extend_from_slice(&chunk_len.to_be_bytes());
        out.extend_from_slice(&self.chunk_bytes);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::MAX_CHUNK_SIZE;

    #[test]
    fn valid_chunk_roundtrip() {
        let chunk = RawChunk {
            transfer_id: TransferId::new(),
            chunk_index: 7,
            offset: 4096,
            chunk_bytes: b"chunk".to_vec(),
        };

        let encoded = chunk.encode().expect("valid chunk");
        let decoded = RawChunk::decode(&encoded).expect("decoded chunk");

        assert_eq!(decoded, chunk);
    }

    #[test]
    fn truncated_chunk_rejected() {
        assert!(matches!(
            RawChunk::decode(&[0_u8; RAW_CHUNK_HEADER_LEN - 1]),
            Err(TealDriveError::InvalidChunk)
        ));
    }

    #[test]
    fn chunk_len_mismatch_rejected() {
        let chunk = RawChunk {
            transfer_id: TransferId::new(),
            chunk_index: 1,
            offset: 0,
            chunk_bytes: b"abc".to_vec(),
        };
        let mut encoded = chunk.encode().expect("valid chunk");
        encoded[28..32].copy_from_slice(&9_u32.to_be_bytes());

        assert!(matches!(
            RawChunk::decode(&encoded),
            Err(TealDriveError::InvalidChunk)
        ));
    }

    #[test]
    fn oversized_chunk_rejected() {
        let chunk = RawChunk {
            transfer_id: TransferId::new(),
            chunk_index: 1,
            offset: 0,
            chunk_bytes: vec![0_u8; MAX_CHUNK_SIZE + 1],
        };

        assert!(matches!(chunk.encode(), Err(TealDriveError::InvalidChunk)));
    }

    #[test]
    fn big_endian_chunk_fields_parse_correctly() {
        let transfer_id = TransferId::new();
        let chunk = RawChunk {
            transfer_id,
            chunk_index: 0x0102_0304,
            offset: 0x0102_0304_0506_0708,
            chunk_bytes: vec![1, 2, 3],
        };

        let encoded = chunk.encode().expect("valid chunk");
        let decoded = RawChunk::decode(&encoded).expect("decoded chunk");

        assert_eq!(decoded.transfer_id, transfer_id);
        assert_eq!(decoded.chunk_index, 0x0102_0304);
        assert_eq!(decoded.offset, 0x0102_0304_0506_0708);
    }
}
