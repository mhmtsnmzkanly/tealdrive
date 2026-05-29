use crate::config::UserContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionContext {
    pub user: UserContext,
    pub protocol_ready: bool,
}
