use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::protocol::frame::RequestId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorKind {
    ValidationError,
    PermissionDenied,
    PolicyDenied,
    FeatureDisabled,
    NotFound,
    InvalidPath,
    RateLimited,
    ProtocolError,
    NotImplemented,
    InternalError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyReason {
    FeatureDisabled,
    SensitivePath,
    ReadOnlyRoot,
    WebRootExecutableUpload,
    PathTraversal,
    SymlinkEscape,
    AccountRejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub code: String,
    pub error_kind: ErrorKind,
    pub safe_message: String,
    pub request_id: RequestId,
    pub operation: Option<String>,
    pub retryable: bool,
    pub policy_reason: Option<PolicyReason>,
    pub debug_ref: Option<String>,
}

impl ErrorPayload {
    pub fn feature_disabled(request_id: RequestId, operation: impl Into<String>) -> Self {
        Self {
            code: "FEATURE_DISABLED".to_owned(),
            error_kind: ErrorKind::FeatureDisabled,
            safe_message: "This operation is disabled in TealDrive V1.".to_owned(),
            request_id,
            operation: Some(operation.into()),
            retryable: false,
            policy_reason: Some(PolicyReason::FeatureDisabled),
            debug_ref: None,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TealDriveError {
    #[error("validation error")]
    Validation,
    #[error("permission denied")]
    PermissionDenied,
    #[error("policy denied")]
    PolicyDenied,
    #[error("feature disabled")]
    FeatureDisabled,
    #[error("protocol error")]
    Protocol,
    #[error("not implemented")]
    NotImplemented,
    #[error("invalid TDRV magic")]
    InvalidMagic,
    #[error("unsupported TDRV version")]
    UnsupportedVersion,
    #[error("invalid TDRV header length")]
    InvalidHeaderLength,
    #[error("reserved TDRV header field must be zero")]
    ReservedFieldNonZero,
    #[error("payload too large")]
    PayloadTooLarge,
    #[error("unknown TDRV message type")]
    UnknownMessageType,
    #[error("invalid message direction")]
    InvalidMessageDirection,
    #[error("invalid TDRV flags")]
    InvalidFlags,
    #[error("invalid TDRV encoding")]
    InvalidEncoding,
    #[error("invalid TDRV compression")]
    InvalidCompression,
    #[error("payload length mismatch")]
    PayloadLengthMismatch,
    #[error("invalid TDRV raw chunk")]
    InvalidChunk,
    #[error("internal error")]
    Internal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_disabled_error_payload_is_safe() {
        let request_id = RequestId::new();
        let payload = ErrorPayload::feature_disabled(request_id, "SAVE_TEXT_FILE");

        assert_eq!(payload.code, "FEATURE_DISABLED");
        assert_eq!(payload.error_kind, ErrorKind::FeatureDisabled);
        assert_eq!(payload.policy_reason, Some(PolicyReason::FeatureDisabled));
        assert!(!payload.safe_message.contains('/'));
    }
}
