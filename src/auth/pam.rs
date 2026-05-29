use crate::auth::account::{AuthError, AuthRequest, AuthSuccess, Authenticator};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PamAuthenticator;

impl PamAuthenticator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PamAuthenticator {
    fn default() -> Self {
        Self::new()
    }
}

impl Authenticator for PamAuthenticator {
    fn authenticate(&self, _request: &AuthRequest) -> Result<AuthSuccess, AuthError> {
        #[cfg(feature = "pam-auth")]
        {
            Err(AuthError::AuthBackendUnavailable)
        }

        #[cfg(not(feature = "pam-auth"))]
        {
            Err(AuthError::AuthBackendUnavailable)
        }
    }
}
