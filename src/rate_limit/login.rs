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
    lockout_duration_seconds: u64,
    failures: HashMap<LoginRateLimitKey, (u32, u64)>,
}

impl InMemoryLoginRateLimiter {
    pub fn new(max_failures: u32) -> Self {
        Self::with_lockout(max_failures, 300)
    }

    pub fn with_lockout(max_failures: u32, lockout_duration_seconds: u64) -> Self {
        Self {
            max_failures,
            lockout_duration_seconds,
            failures: HashMap::new(),
        }
    }

    pub fn set_limits(&mut self, max_failures: u32, lockout_duration_seconds: u64) {
        self.max_failures = max_failures;
        self.lockout_duration_seconds = lockout_duration_seconds;
    }

    pub fn check(&self, key: &LoginRateLimitKey) -> LoginRateLimitDecision {
        self.check_at(
            key,
            self.max_failures,
            self.lockout_duration_seconds,
            crate::session::store::InMemorySessionStore::now_epoch_seconds(),
        )
    }

    pub fn check_at(
        &self,
        key: &LoginRateLimitKey,
        max_failures: u32,
        lockout_duration_seconds: u64,
        now: u64,
    ) -> LoginRateLimitDecision {
        if let Some(&(count, last_failed_at)) = self.failures.get(key) {
            if now >= last_failed_at && now.saturating_sub(last_failed_at) > lockout_duration_seconds {
                LoginRateLimitDecision::Allowed
            } else if count >= max_failures {
                LoginRateLimitDecision::Limited
            } else {
                LoginRateLimitDecision::Allowed
            }
        } else {
            LoginRateLimitDecision::Allowed
        }
    }

    pub fn record_failure(&mut self, key: LoginRateLimitKey) -> LoginRateLimitDecision {
        self.record_failure_at(
            key,
            self.lockout_duration_seconds,
            crate::session::store::InMemorySessionStore::now_epoch_seconds(),
        )
    }

    pub fn record_failure_at(
        &mut self,
        key: LoginRateLimitKey,
        lockout_duration_seconds: u64,
        now: u64,
    ) -> LoginRateLimitDecision {
        let entry = self.failures.entry(key.clone()).or_insert((0, now));
        if now >= entry.1 && now.saturating_sub(entry.1) > lockout_duration_seconds {
            *entry = (1, now);
        } else {
            entry.0 += 1;
            entry.1 = now;
        }
        self.check_at(&key, self.max_failures, lockout_duration_seconds, now)
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

    #[test]
    fn lockout_expires_after_duration() {
        let key = LoginRateLimitKey {
            username: "bob".to_owned(),
            ip: Some("127.0.0.1".to_owned()),
        };
        let mut limiter = InMemoryLoginRateLimiter::with_lockout(2, 60);

        limiter.record_failure_at(key.clone(), 60, 100);
        limiter.record_failure_at(key.clone(), 60, 105);

        // Within lockout duration
        assert_eq!(
            limiter.check_at(&key, 2, 60, 150),
            LoginRateLimitDecision::Limited
        );

        // After lockout duration has passed (> 60s from 105)
        assert_eq!(
            limiter.check_at(&key, 2, 60, 170),
            LoginRateLimitDecision::Allowed
        );
    }
}
