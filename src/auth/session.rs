use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub username: String,
    pub uid: u32,
    pub gid: u32,
    pub created_at: u64,
}

pub struct SessionManager {
    sessions: Arc<DashMap<Uuid, Session>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
        }
    }

    pub fn create_session(&self, username: String, uid: u32, gid: u32) -> Session {
        let id = Uuid::new_v4();
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let session = Session {
            id,
            username,
            uid,
            gid,
            created_at,
        };

        self.sessions.insert(id, session.clone());
        session
    }

    pub fn get_session(&self, id: &Uuid) -> Option<Session> {
        self.sessions.get(id).map(|s| s.clone())
    }

    pub fn remove_session(&self, id: &Uuid) {
        self.sessions.remove(id);
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}
