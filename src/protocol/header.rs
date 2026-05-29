use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

use crate::errors::TealDriveError;
use crate::limits::MAX_FRAME_PAYLOAD;
use crate::protocol::compression::Compression;
use crate::protocol::flags::TdrvFlags;
use crate::protocol::frame::RequestId;
use crate::protocol::message_type::MessageType;

pub const TDRV_MAGIC: [u8; 4] = *b"TDRV";
pub const TDRV_VERSION: u8 = 1;
pub const HEADER_LEN: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TdrvHeader {
    pub magic: [u8; 4],
    pub version: u8,
    pub flags: TdrvFlags,
    pub message_type: MessageType,
    pub compression: Compression,
    pub encoding: Encoding,
    pub reserved: u8,
    pub request_id: RequestId,
    pub payload_len: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Encoding {
    None = 0,
    MessagePack = 1,
    RawBinary = 2,
}

impl TryFrom<u8> for Encoding {
    type Error = TealDriveError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::MessagePack),
            2 => Ok(Self::RawBinary),
            _ => Err(TealDriveError::InvalidEncoding),
        }
    }
}

impl From<Encoding> for u8 {
    fn from(value: Encoding) -> Self {
        value as u8
    }
}

impl TdrvHeader {
    pub fn new(message_type: MessageType, request_id: RequestId, payload_len: u32) -> Self {
        Self {
            magic: TDRV_MAGIC,
            version: TDRV_VERSION,
            flags: TdrvFlags::empty(),
            message_type,
            compression: Compression::None,
            encoding: Encoding::MessagePack,
            reserved: 0,
            request_id,
            payload_len,
        }
    }

    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut out = [0_u8; HEADER_LEN];
        out[0..4].copy_from_slice(&self.magic);
        out[4] = self.version;
        out[5..7].copy_from_slice(&self.flags.bits().to_be_bytes());
        out[7..9].copy_from_slice(&u16::from(self.message_type).to_be_bytes());
        out[9] = u8::from(self.compression);
        out[10] = u8::from(self.encoding);
        out[11] = self.reserved;
        out[12..28].copy_from_slice(self.request_id.as_bytes());
        out[28..32].copy_from_slice(&self.payload_len.to_be_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, TealDriveError> {
        if bytes.len() != HEADER_LEN {
            return Err(TealDriveError::InvalidHeaderLength);
        }

        let magic = [bytes[0], bytes[1], bytes[2], bytes[3]];
        if magic != TDRV_MAGIC {
            return Err(TealDriveError::InvalidMagic);
        }

        let version = bytes[4];
        if version != TDRV_VERSION {
            return Err(TealDriveError::UnsupportedVersion);
        }

        let flags = TdrvFlags::from_bits(u16::from_be_bytes([bytes[5], bytes[6]]))?;
        let message_type = MessageType::try_from(u16::from_be_bytes([bytes[7], bytes[8]]))?;
        let compression = Compression::try_from(bytes[9])?;
        if !compression.is_v1_supported() {
            return Err(TealDriveError::InvalidCompression);
        }
        let encoding = Encoding::try_from(bytes[10])?;
        let reserved = bytes[11];
        if reserved != 0 {
            return Err(TealDriveError::ReservedFieldNonZero);
        }

        if flags.compressed != (compression != Compression::None) {
            return Err(TealDriveError::InvalidFlags);
        }

        validate_encoding_for_message(message_type, encoding)?;

        let request_id = RequestId::from_bytes(bytes[12..28].try_into().expect("fixed slice len"));
        let payload_len = u32::from_be_bytes([bytes[28], bytes[29], bytes[30], bytes[31]]);
        if payload_len as usize > MAX_FRAME_PAYLOAD {
            return Err(TealDriveError::PayloadTooLarge);
        }

        Ok(Self {
            magic,
            version,
            flags,
            message_type,
            compression,
            encoding,
            reserved,
            request_id,
            payload_len,
        })
    }
}

pub fn validate_encoding_for_message(
    message_type: MessageType,
    encoding: Encoding,
) -> Result<(), TealDriveError> {
    match message_type {
        MessageType::UploadChunk | MessageType::DownloadChunk => {
            if encoding == Encoding::RawBinary {
                Ok(())
            } else {
                Err(TealDriveError::InvalidEncoding)
            }
        }
        _ => {
            if encoding == Encoding::RawBinary {
                Err(TealDriveError::InvalidEncoding)
            } else {
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::MAX_FRAME_PAYLOAD;

    fn valid_header() -> TdrvHeader {
        TdrvHeader::new(MessageType::ListDirectory, RequestId::new(), 12)
    }

    #[test]
    fn valid_header_roundtrip() {
        let header = valid_header();
        let encoded = header.encode();
        let decoded = TdrvHeader::decode(&encoded).expect("valid header");

        assert_eq!(decoded, header);
        assert_eq!(encoded.len(), HEADER_LEN);
    }

    #[test]
    fn invalid_magic_rejected() {
        let mut encoded = valid_header().encode();
        encoded[0..4].copy_from_slice(b"NOPE");

        assert!(matches!(
            TdrvHeader::decode(&encoded),
            Err(TealDriveError::InvalidMagic)
        ));
    }

    #[test]
    fn unsupported_version_rejected() {
        let mut encoded = valid_header().encode();
        encoded[4] = 2;

        assert!(matches!(
            TdrvHeader::decode(&encoded),
            Err(TealDriveError::UnsupportedVersion)
        ));
    }

    #[test]
    fn reserved_non_zero_rejected() {
        let mut encoded = valid_header().encode();
        encoded[11] = 1;

        assert!(matches!(
            TdrvHeader::decode(&encoded),
            Err(TealDriveError::ReservedFieldNonZero)
        ));
    }

    #[test]
    fn payload_too_large_rejected() {
        let mut encoded = valid_header().encode();
        let too_large = (MAX_FRAME_PAYLOAD as u32) + 1;
        encoded[28..32].copy_from_slice(&too_large.to_be_bytes());

        assert!(matches!(
            TdrvHeader::decode(&encoded),
            Err(TealDriveError::PayloadTooLarge)
        ));
    }

    #[test]
    fn big_endian_fields_parse_correctly() {
        let mut header = TdrvHeader::new(MessageType::UploadChunk, RequestId::new(), 0x0001_0203);
        header.encoding = Encoding::RawBinary;
        header.flags.stream = true;
        let encoded = header.encode();
        let decoded = TdrvHeader::decode(&encoded).expect("valid big-endian header");

        assert_eq!(u16::from(decoded.message_type), 0x0201);
        assert_eq!(decoded.payload_len, 0x0001_0203);
        assert!(decoded.flags.stream);
    }

    #[test]
    fn compressed_flag_must_match_compression() {
        let mut encoded = valid_header().encode();
        encoded[9] = u8::from(Compression::Gzip);

        assert!(matches!(
            TdrvHeader::decode(&encoded),
            Err(TealDriveError::InvalidFlags)
        ));
    }

    #[test]
    fn compression_must_match_compressed_flag() {
        let mut header = valid_header();
        header.flags.compressed = true;
        let encoded = header.encode();

        assert!(matches!(
            TdrvHeader::decode(&encoded),
            Err(TealDriveError::InvalidFlags)
        ));
    }

    #[test]
    fn known_encoding_values_parse() {
        assert_eq!(Encoding::try_from(0), Ok(Encoding::None));
        assert_eq!(Encoding::try_from(1), Ok(Encoding::MessagePack));
        assert_eq!(Encoding::try_from(2), Ok(Encoding::RawBinary));
    }

    #[test]
    fn unknown_encoding_rejected() {
        assert!(matches!(
            Encoding::try_from(9),
            Err(TealDriveError::InvalidEncoding)
        ));
    }

    #[test]
    fn control_messages_reject_raw_binary_encoding() {
        assert!(matches!(
            validate_encoding_for_message(MessageType::ListDirectory, Encoding::RawBinary),
            Err(TealDriveError::InvalidEncoding)
        ));
    }

    #[test]
    fn chunk_messages_require_raw_binary_encoding() {
        assert!(
            validate_encoding_for_message(MessageType::UploadChunk, Encoding::RawBinary).is_ok()
        );
        assert!(matches!(
            validate_encoding_for_message(MessageType::UploadChunk, Encoding::MessagePack),
            Err(TealDriveError::InvalidEncoding)
        ));
    }

    #[test]
    fn unsupported_future_compression_rejected_in_v1_header() {
        let mut encoded = valid_header().encode();
        encoded[5..7].copy_from_slice(&crate::protocol::flags::FLAG_COMPRESSED.to_be_bytes());
        encoded[9] = u8::from(Compression::DeflateFuture);

        assert!(matches!(
            TdrvHeader::decode(&encoded),
            Err(TealDriveError::InvalidCompression)
        ));
    }
}
