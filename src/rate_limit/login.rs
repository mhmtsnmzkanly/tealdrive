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
            if now >= last_failed_at
                && now.saturating_sub(last_failed_at) > lockout_duration_seconds
            {
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

    pub const MAX_RATE_LIMIT_ENTRIES: usize = 10_000;

    pub fn cleanup(&mut self, now: u64) -> usize {
        let before = self.failures.len();
        let lockout = self.lockout_duration_seconds;
        self.failures
            .retain(|_, (_, last_failed_at)| now.saturating_sub(*last_failed_at) <= lockout);
        before.saturating_sub(self.failures.len())
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
        if self.failures.len() >= Self::MAX_RATE_LIMIT_ENTRIES {
            self.cleanup(now);
            if self.failures.len() >= Self::MAX_RATE_LIMIT_ENTRIES
                && !self.failures.contains_key(&key)
            {
                // Never evict an active lockout: flooding fresh keys must not reset a victim's lock.
                let max_failures = self.max_failures;
                let Some(oldest_key) = self
                    .failures
                    .iter()
                    .filter(|(_, (count, _))| *count < max_failures)
                    .min_by_key(|(_, (_, last))| *last)
                    .map(|(k, _)| k.clone())
                else {
                    return LoginRateLimitDecision::Limited;
                };
                self.failures.remove(&oldest_key);
            }
        }
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

    #[test]
    fn full_table_never_evicts_active_lockouts() {
        let key = |name: &str| LoginRateLimitKey {
            username: name.to_owned(),
            ip: Some("192.0.2.1".to_owned()),
        };
        let mut limiter = InMemoryLoginRateLimiter::with_lockout(2, 300);
        limiter.record_failure_at(key("victim"), 300, 10);
        limiter.record_failure_at(key("victim"), 300, 10);
        for index in 0..InMemoryLoginRateLimiter::MAX_RATE_LIMIT_ENTRIES + 10 {
            limiter.record_failure_at(key(&format!("flood-{index}")), 300, 20);
        }
        assert_eq!(
            limiter.check_at(&key("victim"), 2, 300, 30),
            LoginRateLimitDecision::Limited
        );
        assert!(limiter.failures.len() <= InMemoryLoginRateLimiter::MAX_RATE_LIMIT_ENTRIES);
    }

    #[test]
    fn cleanup_evicts_expired_entries() {
        let mut limiter = InMemoryLoginRateLimiter::with_lockout(2, 60);
        let key1 = LoginRateLimitKey {
            username: "u1".into(),
            ip: Some("127.0.0.1".into()),
        };
        let key2 = LoginRateLimitKey {
            username: "u2".into(),
            ip: Some("127.0.0.1".into()),
        };
        limiter.record_failure_at(key1, 60, 100);
        limiter.record_failure_at(key2, 60, 150);

        // At t=180, key1 is expired (180 - 100 = 80 > 60), key2 is active (180 - 150 = 30 <= 60)
        let removed = limiter.cleanup(180);
        assert_eq!(removed, 1);
        assert_eq!(limiter.failures.len(), 1);
    }
}
