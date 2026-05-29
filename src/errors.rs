use thiserror::Error;

#[derive(Error, Debug)]
pub enum TealError {
    #[error("Authentication failed")]
    AuthFailed,
    #[error("Session not found")]
    SessionNotFound,
    #[error("Permission denied")]
    PermissionDenied,
    #[error("Policy denied")]
    PolicyDenied,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Internal error: {0}")]
    Internal(String),
}
