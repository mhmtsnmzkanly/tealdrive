use crate::auth::session::SessionManager;
use std::sync::Arc;

pub struct AppState {
    pub sessions: SessionManager,
    // Add more state as needed (e.g., config, database handle)
}

pub type SharedState = Arc<AppState>;

impl AppState {
    pub fn new() -> Self {
        Self {
            sessions: SessionManager::new(),
        }
    }
}
