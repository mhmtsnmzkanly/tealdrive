use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CsrfToken(pub String);

impl CsrfToken {
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn validate(&self, provided: &CsrfToken) -> bool {
        self == provided
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_generation_produces_value() {
        let token = CsrfToken::generate();

        assert!(!token.0.is_empty());
    }

    #[test]
    fn valid_token_passes() {
        let token = CsrfToken::generate();

        assert!(token.validate(&token));
    }

    #[test]
    fn invalid_token_fails() {
        let token = CsrfToken::generate();
        let other = CsrfToken::generate();

        assert!(!token.validate(&other));
    }
}
