use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

use crate::errors::TealDriveError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Compression {
    None = 0,
    Gzip = 1,
    DeflateFuture = 2,
    ZstdFuture = 3,
}

impl Compression {
    pub fn is_v1_supported(self) -> bool {
        matches!(self, Self::None | Self::Gzip)
    }
}

impl TryFrom<u8> for Compression {
    type Error = TealDriveError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Gzip),
            2 => Ok(Self::DeflateFuture),
            3 => Ok(Self::ZstdFuture),
            _ => Err(TealDriveError::InvalidCompression),
        }
    }
}

impl From<Compression> for u8 {
    fn from(value: Compression) -> Self {
        value as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_compression_values_parse() {
        assert_eq!(Compression::try_from(0), Ok(Compression::None));
        assert_eq!(Compression::try_from(1), Ok(Compression::Gzip));
        assert_eq!(Compression::try_from(2), Ok(Compression::DeflateFuture));
        assert_eq!(Compression::try_from(3), Ok(Compression::ZstdFuture));
    }

    #[test]
    fn unknown_compression_rejected() {
        assert!(matches!(
            Compression::try_from(9),
            Err(TealDriveError::InvalidCompression)
        ));
    }

    #[test]
    fn deflate_and_zstd_are_not_supported_in_v1() {
        assert!(Compression::None.is_v1_supported());
        assert!(Compression::Gzip.is_v1_supported());
        assert!(!Compression::DeflateFuture.is_v1_supported());
        assert!(!Compression::ZstdFuture.is_v1_supported());
    }
}
