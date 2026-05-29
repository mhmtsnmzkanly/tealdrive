use pam::Authenticator;
use tracing::{debug, error};

pub struct PamAuth;

impl PamAuth {
    pub fn authenticate(username: &str, password: &str) -> bool {
        let mut authenticator = match Authenticator::with_password("system-auth") {
            Ok(auth) => auth,
            Err(_) => {
                match Authenticator::with_password("login") {
                    Ok(auth) => auth,
                    Err(e) => {
                        error!("Failed to initialize PAM authenticator: {:?}", e);
                        return false;
                    }
                }
            }
        };

        // Let's try to set credentials through the handler. 
        // Based on some versions of the 'pam' crate, it might be different.
        // If handler_mut() doesn't work, we'll try a different approach.
        authenticator.get_handler().set_credentials(username, password);

        match authenticator.authenticate() {
            Ok(_) => {
                debug!("PAM authentication successful for user: {}", username);
                true
            }
            Err(e) => {
                debug!("PAM authentication failed for user: {}: {:?}", username, e);
                false
            }
        }
    }
}
