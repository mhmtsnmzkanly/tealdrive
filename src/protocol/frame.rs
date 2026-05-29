use bytes::{Buf, BufMut, BytesMut};
use crate::protocol::message_type::MessageType;
use uuid::Uuid;
use std::convert::TryInto;

pub const HEADER_SIZE: usize = 32;
pub const MAGIC: &[u8; 4] = b"TDRV";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    pub version: u8,
    pub flags: u16,
    pub message_type: MessageType,
    pub compression: u8,
    pub encoding: u8,
    pub request_id: Uuid,
    pub payload_len: u32,
}

impl FrameHeader {
    pub fn new(message_type: MessageType, request_id: Uuid, payload_len: u32) -> Self {
        Self {
            version: 1,
            flags: 0,
            message_type,
            compression: 0,
            encoding: 1, // Default to MessagePack
            request_id,
            payload_len,
        }
    }

    pub fn decode(src: &mut BytesMut) -> anyhow::Result<Option<Self>> {
        if src.len() < HEADER_SIZE {
            return Ok(None);
        }

        let magic = &src[0..4];
        if magic != MAGIC {
            return Err(anyhow::anyhow!("Invalid magic bytes"));
        }

        let mut buf = &src[4..HEADER_SIZE];
        let version = buf.get_u8();
        let flags = buf.get_u16();
        let message_type_raw = buf.get_u16();
        let message_type = MessageType::from(message_type_raw);
        let compression = buf.get_u8();
        let encoding = buf.get_u8();
        let _reserved = buf.get_u8();
        
        let mut uuid_bytes = [0u8; 16];
        uuid_bytes.copy_from_slice(&src[12..28]);
        let request_id = Uuid::from_bytes(uuid_bytes);
        
        let payload_len = (&src[28..32]).get_u32();

        src.advance(HEADER_SIZE);

        Ok(Some(Self {
            version,
            flags,
            message_type,
            compression,
            encoding,
            request_id,
            payload_len,
        }))
    }

    pub fn encode(&self, dst: &mut BytesMut) {
        dst.put_slice(MAGIC);
        dst.put_u8(self.version);
        dst.put_u16(self.flags);
        dst.put_u16(self.message_type as u16);
        dst.put_u8(self.compression);
        dst.put_u8(self.encoding);
        dst.put_u8(0); // Reserved
        dst.put_slice(self.request_id.as_bytes());
        dst.put_u32(self.payload_len);
    }
}
