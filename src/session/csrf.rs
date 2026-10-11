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

pub fn validate_header(
    session: &crate::session::store::Session,
    headers: &axum::http::HeaderMap,
) -> bool {
    let Some(provided) = headers.get("x-csrf-token").and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let expected = session.csrf_token.0.as_bytes();
    let actual = provided.as_bytes();
    expected.len() == actual.len()
        && expected
            .iter()
            .zip(actual)
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            == 0
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
