use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::SessionConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub session_id: SessionId,
    pub username: String,
    pub uid: u32,
    pub gid: u32,
    pub created_at: u64,
    pub last_seen_at: u64,
    pub expires_at: u64,
    pub idle_expires_at: u64,
    pub user_agent: Option<String>,
    pub ip: Option<String>,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionCreateInput {
    pub username: String,
    pub uid: u32,
    pub gid: u32,
    pub user_agent: Option<String>,
    pub ip: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionLookupResult {
    Found(Session),
    NotFound,
    Expired,
    Revoked,
}

pub trait SessionStore {
    fn create_session(&mut self, input: SessionCreateInput, now: u64) -> Session;
    fn get_session(&self, session_id: &SessionId, now: u64) -> SessionLookupResult;
    fn touch_session(&mut self, session_id: &SessionId, now: u64) -> SessionLookupResult;
    fn revoke_session(&mut self, session_id: &SessionId) -> bool;
    fn revoke_all_for_user(&mut self, username: &str) -> usize;
    fn cleanup_expired(&mut self, now: u64) -> usize;
}

#[derive(Debug, Clone)]
pub struct InMemorySessionStore {
    sessions: HashMap<SessionId, Session>,
    config: SessionConfig,
}

impl InMemorySessionStore {
    pub fn new(config: SessionConfig) -> Self {
        Self {
            sessions: HashMap::new(),
            config,
        }
    }

    pub fn now_epoch_seconds() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs())
    }

    fn classify(session: &Session, now: u64) -> SessionLookupResult {
        if session.revoked {
            SessionLookupResult::Revoked
        } else if now >= session.expires_at || now >= session.idle_expires_at {
            SessionLookupResult::Expired
        } else {
            SessionLookupResult::Found(session.clone())
        }
    }
}

impl SessionStore for InMemorySessionStore {
    fn create_session(&mut self, input: SessionCreateInput, now: u64) -> Session {
        let session = Session {
            session_id: SessionId::new(),
            username: input.username,
            uid: input.uid,
            gid: input.gid,
            created_at: now,
            last_seen_at: now,
            expires_at: now + self.config.absolute_timeout_seconds,
            idle_expires_at: now + self.config.idle_timeout_seconds,
            user_agent: input.user_agent,
            ip: input.ip,
            revoked: false,
        };
        self.sessions.insert(session.session_id, session.clone());
        session
    }

    fn get_session(&self, session_id: &SessionId, now: u64) -> SessionLookupResult {
        self.sessions
            .get(session_id)
            .map_or(SessionLookupResult::NotFound, |session| {
                Self::classify(session, now)
            })
    }

    fn touch_session(&mut self, session_id: &SessionId, now: u64) -> SessionLookupResult {
        let Some(session) = self.sessions.get_mut(session_id) else {
            return SessionLookupResult::NotFound;
        };
        match Self::classify(session, now) {
            SessionLookupResult::Found(_) => {
                session.last_seen_at = now;
                session.idle_expires_at = now + self.config.idle_timeout_seconds;
                SessionLookupResult::Found(session.clone())
            }
            other => other,
        }
    }

    fn revoke_session(&mut self, session_id: &SessionId) -> bool {
        let Some(session) = self.sessions.get_mut(session_id) else {
            return false;
        };
        session.revoked = true;
        true
    }

    fn revoke_all_for_user(&mut self, username: &str) -> usize {
        let mut revoked = 0;
        for session in self.sessions.values_mut() {
            if session.username == username && !session.revoked {
                session.revoked = true;
                revoked += 1;
            }
        }
        revoked
    }

    fn cleanup_expired(&mut self, now: u64) -> usize {
        let before = self.sessions.len();
        self.sessions.retain(|_, session| {
            !matches!(
                Self::classify(session, now),
                SessionLookupResult::Expired | SessionLookupResult::Revoked
            )
        });
        before - self.sessions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> InMemorySessionStore {
        let config = SessionConfig {
            idle_timeout_seconds: 10,
            absolute_timeout_seconds: 100,
            ..SessionConfig::default()
        };
        InMemorySessionStore::new(config)
    }

    fn input(username: &str) -> SessionCreateInput {
        SessionCreateInput {
            username: username.to_owned(),
            uid: 1000,
            gid: 1000,
            user_agent: Some("test-agent".to_owned()),
            ip: Some("127.0.0.1".to_owned()),
        }
    }

    #[test]
    fn create_and_lookup_session() {
        let mut store = store();
        let session = store.create_session(input("alice"), 100);

        assert!(matches!(
            store.get_session(&session.session_id, 100),
            SessionLookupResult::Found(found) if found.username == "alice"
        ));
    }

    #[test]
    fn touch_session_updates_last_seen() {
        let mut store = store();
        let session = store.create_session(input("alice"), 100);

        let touched = store.touch_session(&session.session_id, 105);

        assert!(matches!(
            touched,
            SessionLookupResult::Found(found)
                if found.last_seen_at == 105 && found.idle_expires_at == 115
        ));
    }

    #[test]
    fn idle_expiry_works() {
        let mut store = store();
        let session = store.create_session(input("alice"), 100);

        assert_eq!(
            store.get_session(&session.session_id, 111),
            SessionLookupResult::Expired
        );
    }

    #[test]
    fn absolute_expiry_works() {
        let mut store = store();
        let session = store.create_session(input("alice"), 100);

        assert_eq!(
            store.get_session(&session.session_id, 200),
            SessionLookupResult::Expired
        );
    }

    #[test]
    fn revoke_session_works() {
        let mut store = store();
        let session = store.create_session(input("alice"), 100);

        assert!(store.revoke_session(&session.session_id));
        assert_eq!(
            store.get_session(&session.session_id, 100),
            SessionLookupResult::Revoked
        );
    }

    #[test]
    fn revoke_all_sessions_for_user() {
        let mut store = store();
        let first = store.create_session(input("alice"), 100);
        let second = store.create_session(input("alice"), 100);
        let bob = store.create_session(input("bob"), 100);

        assert_eq!(store.revoke_all_for_user("alice"), 2);
        assert_eq!(
            store.get_session(&first.session_id, 100),
            SessionLookupResult::Revoked
        );
        assert_eq!(
            store.get_session(&second.session_id, 100),
            SessionLookupResult::Revoked
        );
        assert!(matches!(
            store.get_session(&bob.session_id, 100),
            SessionLookupResult::Found(_)
        ));
    }

    #[test]
    fn cleanup_expired_sessions() {
        let mut store = store();
        let expired = store.create_session(input("alice"), 100);
        let active = store.create_session(input("bob"), 105);
        store.revoke_session(&expired.session_id);

        assert_eq!(store.cleanup_expired(106), 1);
        assert_eq!(
            store.get_session(&expired.session_id, 106),
            SessionLookupResult::NotFound
        );
        assert!(matches!(
            store.get_session(&active.session_id, 106),
            SessionLookupResult::Found(_)
        ));
    }
}
