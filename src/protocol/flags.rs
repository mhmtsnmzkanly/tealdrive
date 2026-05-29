use serde::{Deserialize, Serialize};

use crate::errors::TealDriveError;

pub const FLAG_ERROR: u16 = 0x0001;
pub const FLAG_STREAM: u16 = 0x0002;
pub const FLAG_FINAL: u16 = 0x0004;
pub const FLAG_ACK_REQUIRED: u16 = 0x0008;
pub const FLAG_COMPRESSED: u16 = 0x0010;
pub const KNOWN_FLAGS_MASK: u16 =
    FLAG_ERROR | FLAG_STREAM | FLAG_FINAL | FLAG_ACK_REQUIRED | FLAG_COMPRESSED;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TdrvFlags {
    pub error: bool,
    pub stream: bool,
    pub final_frame: bool,
    pub ack_required: bool,
    pub compressed: bool,
}

impl TdrvFlags {
    pub const fn empty() -> Self {
        Self {
            error: false,
            stream: false,
            final_frame: false,
            ack_required: false,
            compressed: false,
        }
    }

    pub const fn compressed() -> Self {
        Self {
            compressed: true,
            ..Self::empty()
        }
    }

    pub fn bits(self) -> u16 {
        let mut bits = 0;
        if self.error {
            bits |= FLAG_ERROR;
        }
        if self.stream {
            bits |= FLAG_STREAM;
        }
        if self.final_frame {
            bits |= FLAG_FINAL;
        }
        if self.ack_required {
            bits |= FLAG_ACK_REQUIRED;
        }
        if self.compressed {
            bits |= FLAG_COMPRESSED;
        }
        bits
    }

    pub fn from_bits(bits: u16) -> Result<Self, TealDriveError> {
        if bits & !KNOWN_FLAGS_MASK != 0 {
            return Err(TealDriveError::InvalidFlags);
        }

        Ok(Self {
            error: bits & FLAG_ERROR != 0,
            stream: bits & FLAG_STREAM != 0,
            final_frame: bits & FLAG_FINAL != 0,
            ack_required: bits & FLAG_ACK_REQUIRED != 0,
            compressed: bits & FLAG_COMPRESSED != 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_flags_roundtrip() {
        let flags = TdrvFlags {
            error: true,
            stream: true,
            final_frame: false,
            ack_required: true,
            compressed: true,
        };

        assert_eq!(TdrvFlags::from_bits(flags.bits()), Ok(flags));
    }

    #[test]
    fn unknown_flags_rejected() {
        assert!(matches!(
            TdrvFlags::from_bits(0x8000),
            Err(TealDriveError::InvalidFlags)
        ));
    }
}
