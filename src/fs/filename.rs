use serde::{Deserialize, Serialize};

use crate::errors::TealDriveError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SafeFileName(pub String);

pub type SafeFilename = SafeFileName;

impl SafeFileName {
    pub fn parse(value: &str) -> Result<Self, TealDriveError> {
        if value.is_empty()
            || value.contains('\0')
            || value.contains('/')
            || value == "."
            || value == ".."
            || is_reserved_name(value)
        {
            return Err(filename_error(value));
        }
        Ok(Self(value.to_owned()))
    }
}

pub fn is_reserved_name(value: &str) -> bool {
    matches!(value, "." | "..")
}

fn filename_error(value: &str) -> TealDriveError {
    if value.contains('\0') {
        TealDriveError::NullByteRejected
    } else if is_reserved_name(value) {
        TealDriveError::ReservedNameRejected
    } else {
        TealDriveError::InvalidFilename
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_with_slash_rejected() {
        assert_eq!(
            SafeFileName::parse("bad/name"),
            Err(TealDriveError::InvalidFilename)
        );
    }

    #[test]
    fn filename_with_nul_rejected() {
        assert_eq!(
            SafeFileName::parse("bad\0name"),
            Err(TealDriveError::NullByteRejected)
        );
    }

    #[test]
    fn filename_parent_component_rejected() {
        assert_eq!(
            SafeFileName::parse(".."),
            Err(TealDriveError::ReservedNameRejected)
        );
    }

    #[test]
    fn valid_filename_accepted() {
        assert_eq!(
            SafeFileName::parse("notes.txt"),
            Ok(SafeFileName("notes.txt".to_owned()))
        );
    }
}
