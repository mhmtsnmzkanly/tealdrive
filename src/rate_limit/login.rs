use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoginRateLimitKey {
    pub username: String,
    pub ip: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginRateLimitDecision {
    Allowed,
    Limited,
}

#[derive(Debug, Clone)]
pub struct InMemoryLoginRateLimiter {
    max_failures: u32,
    failures: HashMap<LoginRateLimitKey, u32>,
}

impl InMemoryLoginRateLimiter {
    pub fn new(max_failures: u32) -> Self {
        Self {
            max_failures,
            failures: HashMap::new(),
        }
    }

    pub fn check(&self, key: &LoginRateLimitKey) -> LoginRateLimitDecision {
        if self.failures.get(key).copied().unwrap_or(0) >= self.max_failures {
            LoginRateLimitDecision::Limited
        } else {
            LoginRateLimitDecision::Allowed
        }
    }

    pub fn record_failure(&mut self, key: LoginRateLimitKey) -> LoginRateLimitDecision {
        let failures = self.failures.entry(key.clone()).or_insert(0);
        *failures += 1;
        self.check(&key)
    }

    pub fn record_success(&mut self, key: &LoginRateLimitKey) {
        self.failures.remove(key);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoginRateLimit;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_failed_attempts_eventually_limited() {
        let key = LoginRateLimitKey {
            username: "alice".to_owned(),
            ip: Some("127.0.0.1".to_owned()),
        };
        let mut limiter = InMemoryLoginRateLimiter::new(2);

        assert_eq!(limiter.check(&key), LoginRateLimitDecision::Allowed);
        assert_eq!(
            limiter.record_failure(key.clone()),
            LoginRateLimitDecision::Allowed
        );
        assert_eq!(
            limiter.record_failure(key.clone()),
            LoginRateLimitDecision::Limited
        );
        assert_eq!(limiter.check(&key), LoginRateLimitDecision::Limited);
    }
}
