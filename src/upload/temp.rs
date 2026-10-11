use crate::protocol::frame::TransferId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TempUploadName(pub String);

impl TempUploadName {
    pub fn new(transfer_id: &TransferId) -> Self {
        Self(format!(".tdrv-upload-{}.tmp", transfer_id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for TempUploadName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TempUploadName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_upload_name_format() {
        let transfer_id = TransferId::new();
        let name = TempUploadName::new(&transfer_id);

        assert!(name.as_str().starts_with(".tdrv-upload-"));
        assert!(name.as_str().ends_with(".tmp"));
        assert_eq!(name.to_string(), name.as_str());
        assert_eq!(name.as_ref(), name.as_str());
    }
}
