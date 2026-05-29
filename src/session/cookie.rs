use crate::config::{SameSitePolicy, SessionConfig};
use crate::session::store::SessionId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionCookieConfig {
    pub name: String,
    pub http_only: bool,
    pub secure: bool,
    pub same_site: SameSitePolicy,
    pub path: String,
    pub max_age_seconds: u64,
}

impl SessionCookieConfig {
    pub fn from_session_config(config: &SessionConfig) -> Self {
        Self {
            name: config.cookie_name.clone(),
            http_only: true,
            secure: config.secure_cookies,
            same_site: config.same_site,
            path: "/".to_owned(),
            max_age_seconds: config.absolute_timeout_seconds,
        }
    }

    pub fn build_set_cookie_value(&self, session_id: &SessionId) -> String {
        let same_site = match self.same_site {
            SameSitePolicy::Strict => "Strict",
            SameSitePolicy::Lax => "Lax",
        };
        let mut value = format!(
            "{}={}; Path={}; Max-Age={}; SameSite={}",
            self.name, session_id.0, self.path, self.max_age_seconds, same_site
        );
        if self.http_only {
            value.push_str("; HttpOnly");
        }
        if self.secure {
            value.push_str("; Secure");
        }
        value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecureCookieConfig;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookie_config_is_secure_by_default() {
        let config = SessionCookieConfig::from_session_config(&SessionConfig::default());

        assert!(config.http_only);
        assert!(config.secure);
        assert_eq!(config.same_site, SameSitePolicy::Strict);
        assert_eq!(config.path, "/");
    }
}
