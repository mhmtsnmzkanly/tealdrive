//! Durable pre-verification reservations: concurrent helpers cannot bypass attempt limits.
use super::store::{number, AccountStore, StoreError};
use crate::config::RateLimitConfig;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};

pub struct LoginReservation(Vec<(String, String, i64)>);
impl AccountStore {
    pub fn reserve_login(
        &mut self,
        username: &str,
        ip: Option<&str>,
        config: &RateLimitConfig,
        at: u64,
    ) -> Result<LoginReservation, StoreError> {
        if username.len() > 256
            || config.max_login_failures == 0
            || config.lockout_duration_seconds == 0
        {
            return Err(StoreError::InvalidInput);
        }
        let at = number(at)?;
        let until = at
            .checked_add(number(config.lockout_duration_seconds)?)
            .ok_or(StoreError::InvalidInput)?;
        let mut scopes = vec![
            (
                "account",
                username.to_owned(),
                i64::from(config.max_login_failures),
            ),
            (
                "global",
                "all".into(),
                i64::from(config.max_login_failures).saturating_mul(40),
            ),
        ];
        if let Some(ip) = ip {
            let ip: std::net::IpAddr = ip.parse().map_err(|_| StoreError::InvalidInput)?;
            scopes.push((
                "ip",
                ip.to_string(),
                i64::from(config.max_login_failures).saturating_mul(5),
            ));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("DELETE FROM login_limits WHERE lock_until<=?1", [at])?;
        let count: i64 = tx.query_row("SELECT count(*) FROM login_limits", [], |r| r.get(0))?;
        let mut reserved = Vec::new();
        for (scope, key, max) in scopes {
            let hash = format!(
                "{:x}",
                Sha256::digest(format!("tealdrive-login-v1:{scope}:{key}"))
            );
            let existing: Option<(i64, i64)> = tx
                .query_row(
                    "SELECT failures,lock_until FROM login_limits WHERE scope=?1 AND key_hash=?2",
                    params![scope, hash],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let (failures, deadline) = existing.unwrap_or((0, until));
            if failures >= max || existing.is_none() && count + reserved.len() as i64 >= 10000 {
                return Err(StoreError::RateLimited);
            }
            tx.execute("INSERT INTO login_limits(scope,key_hash,failures,lock_until,updated_at) VALUES(?1,?2,1,?3,?4) ON CONFLICT(scope,key_hash) DO UPDATE SET failures=failures+1,updated_at=excluded.updated_at", params![scope,hash,deadline,at])?;
            reserved.push((scope.into(), hash, deadline));
        }
        tx.commit()?;
        Ok(LoginReservation(reserved))
    }
    /// Success refunds only this attempt, never other concurrent failures. Crashes consume a slot.
    pub fn finish_login(&mut self, reservation: LoginReservation) -> Result<(), StoreError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for (scope, hash, deadline) in reservation.0 {
            tx.execute("UPDATE login_limits SET failures=max(0,failures-1) WHERE scope=?1 AND key_hash=?2 AND lock_until=?3", params![scope,hash,deadline])?;
        }
        tx.commit()?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reservations_cover_parallel_attempts_and_expire_without_secret_storage() {
        let mut store = AccountStore::memory();
        let config = RateLimitConfig {
            max_login_failures: 2,
            lockout_duration_seconds: 10,
        };
        let success = store
            .reserve_login("alice", Some("127.0.0.1"), &config, 100)
            .unwrap();
        let _failed = store
            .reserve_login("alice", Some("127.0.0.1"), &config, 100)
            .unwrap();
        assert!(matches!(
            store.reserve_login("alice", Some("127.0.0.1"), &config, 101),
            Err(StoreError::RateLimited)
        ));
        store.finish_login(success).unwrap();
        store.reserve_login("alice", None, &config, 102).unwrap();
        assert!(matches!(
            store.reserve_login("alice", None, &config, 109),
            Err(StoreError::RateLimited)
        ));
        store.reserve_login("alice", None, &config, 110).unwrap();
        let keys: Vec<String> = store
            .connection
            .prepare("SELECT key_hash FROM login_limits")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(keys
            .iter()
            .all(|key| key.len() == 64 && !key.contains("alice")));
    }
    #[test]
    fn ip_and_global_limits_apply_across_different_accounts() {
        let mut store = AccountStore::memory();
        let config = RateLimitConfig {
            max_login_failures: 1,
            lockout_duration_seconds: 300,
        };
        for i in 0..5 {
            store
                .reserve_login(&format!("a{i}"), Some("::1"), &config, 100)
                .unwrap();
        }
        assert!(matches!(
            store.reserve_login("other", Some("0:0:0:0:0:0:0:1"), &config, 100),
            Err(StoreError::RateLimited)
        ));
        for i in 5..40 {
            store
                .reserve_login(&format!("a{i}"), None, &config, 100)
                .unwrap();
        }
        assert!(matches!(
            store.reserve_login("last", None, &config, 100),
            Err(StoreError::RateLimited)
        ));
    }
}
