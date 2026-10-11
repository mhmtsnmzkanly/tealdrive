use crate::audit::event::AuditEvent;
use crate::config::UserContext;
use crate::protocol::frame::RequestId;
use crate::session::store::SessionId;
use sha2::{Digest, Sha256};

#[derive(Clone, Default)]
pub struct AuditContext {
    pub request_id: Option<RequestId>,
    pub user: Option<UserContext>,
    pub session_id: Option<SessionId>,
    pub ip: Option<String>,
    pub peer_ip: Option<String>,
    pub user_agent: Option<String>,
}
tokio::task_local! { pub static REQUEST: AuditContext; }
pub fn current() -> AuditContext {
    REQUEST.try_with(Clone::clone).unwrap_or_default()
}
pub fn session_hash(id: SessionId) -> String {
    format!("{:x}", Sha256::digest(id.0.as_bytes()))
}
impl AuditContext {
    pub fn enrich(&self, event: &mut AuditEvent) {
        if let Some(id) = self.request_id {
            event.request_id = id;
        }
        if let Some(id) = self.session_id {
            event.session_id_hash = session_hash(id);
        }
        event.ip = self.ip.clone();
        event.peer_ip = self.peer_ip.clone();
        event.user_agent = self
            .user_agent
            .as_ref()
            .map(|s| s.chars().take(512).collect());
        if event.username == "unknown" {
            if let Some(user) = &self.user {
                event.username = user.username.clone();
                event.uid = user.uid;
                event.gid = user.gid;
            }
        }
    }
}
