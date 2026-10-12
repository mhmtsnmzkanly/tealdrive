//! PHC-encoded Argon2id hashes with random salt and bounded verification work.
use super::store::StoreError;
use argon2::{password_hash::phc::PasswordHash, Argon2, PasswordHasher, PasswordVerifier};
pub const MIN_PASSWORD_CHARS: usize = 12;
pub const MAX_PASSWORD_BYTES: usize = 1024;
pub fn hash(password: &str) -> Result<zeroize::Zeroizing<String>, StoreError> {
    if password.chars().count() < MIN_PASSWORD_CHARS
        || password.len() > MAX_PASSWORD_BYTES
        || password.contains('\0')
    {
        return Err(StoreError::InvalidInput);
    }
    let value = Argon2::default()
        .hash_password(password.as_bytes())
        .map_err(|_| StoreError::PasswordHash)?;
    Ok(zeroize::Zeroizing::new(value.to_string()))
}
pub fn verify(password: &str, encoded: &str) -> Result<bool, StoreError> {
    if password.len() > MAX_PASSWORD_BYTES || encoded.len() > 512 {
        return Err(StoreError::InvalidInput);
    }
    let parsed = PasswordHash::new(encoded).map_err(|_| StoreError::PasswordHash)?;
    if parsed.algorithm.as_str() != "argon2id"
        || parsed.version != Some(19)
        || parsed
            .salt
            .as_ref()
            .is_none_or(|salt| salt.as_ref().len() < 16)
        || parsed.hash.as_ref().is_none_or(|hash| hash.len() != 32)
        || parsed
            .params
            .get_decimal("m")
            .is_none_or(|m| !(19456..=262144).contains(&m))
        || parsed
            .params
            .get_decimal("t")
            .is_none_or(|t| !(2..=10).contains(&t))
        || parsed
            .params
            .get_decimal("p")
            .is_none_or(|p| !(1..=4).contains(&p))
    {
        return Err(StoreError::PasswordHash);
    }
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn salted_argon2id_verification_and_resource_bounds() {
        let first = hash("different-web-password").unwrap();
        let second = hash("different-web-password").unwrap();
        assert_ne!(*first, *second);
        assert!(first.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
        assert!(verify("different-web-password", &first).unwrap());
        assert!(!verify("wrong-web-password", &first).unwrap());
        assert!(hash("short").is_err());
        assert!(hash("🔐🔐🔐🔐").is_err());
        assert!(hash(&"x".repeat(1025)).is_err());
        assert!(verify("any", &first.replace("m=19456", "m=1073741824")).is_err());
        assert!(verify("any", &first.replace("argon2id", "argon2i")).is_err());
    }
}
