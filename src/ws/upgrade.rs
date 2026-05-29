use crate::config::{SecurityConfig, UserContext};
use crate::errors::TealDriveError;
use crate::policy::feature_gate::FeatureFlags;
use crate::session::store::{Session, SessionId, SessionLookupResult, SessionStore};
use crate::ws::connection::WsConnectionContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionBoundUpgrade;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsUpgradeRequest {
    pub session_id: Option<SessionId>,
    pub origin: Option<String>,
    pub user_agent: Option<String>,
    pub ip: Option<String>,
    pub now: u64,
}

pub fn validate_origin(
    security: &SecurityConfig,
    origin: Option<&str>,
) -> Result<(), TealDriveError> {
    if security.allow_plain_ws_localhost_only {
        return Ok(());
    }
    match origin {
        Some(value) if !value.trim().is_empty() => Ok(()),
        _ => Err(TealDriveError::Validation),
    }
}

pub fn accept_session_bound_upgrade<S: SessionStore>(
    store: &S,
    security: &SecurityConfig,
    features: FeatureFlags,
    request: WsUpgradeRequest,
) -> Result<WsConnectionContext, TealDriveError> {
    validate_origin(security, request.origin.as_deref())?;

    let session_id = request.session_id.ok_or(TealDriveError::PermissionDenied)?;
    let session = match store.get_session(&session_id, request.now) {
        SessionLookupResult::Found(session) => session,
        SessionLookupResult::NotFound
        | SessionLookupResult::Expired
        | SessionLookupResult::Revoked => return Err(TealDriveError::PermissionDenied),
    };

    Ok(context_from_session(
        session,
        request.user_agent,
        request.ip,
        request.now,
        features,
    ))
}

fn context_from_session(
    session: Session,
    user_agent: Option<String>,
    ip: Option<String>,
    connected_at: u64,
    features: FeatureFlags,
) -> WsConnectionContext {
    let user = UserContext {
        username: session.username,
        uid: session.uid,
        gid: session.gid,
    };
    WsConnectionContext {
        session_id: session.session_id,
        username: user.username,
        uid: user.uid,
        gid: user.gid,
        user_agent: user_agent.or(session.user_agent),
        ip: ip.or(session.ip),
        connected_at,
        protocol_ready: false,
        selected_protocol: None,
        features,
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::config::{SessionConfig, UserContext};
    use crate::session::{InMemorySessionStore, SessionCreateInput};

    pub fn valid_context() -> WsConnectionContext {
        WsConnectionContext {
            session_id: SessionId::new(),
            username: "alice".to_owned(),
            uid: 1000,
            gid: 1000,
            user_agent: Some("test".to_owned()),
            ip: Some("127.0.0.1".to_owned()),
            connected_at: 100,
            protocol_ready: false,
            selected_protocol: None,
            features: FeatureFlags::default(),
        }
    }

    fn store_with_session(now: u64) -> (InMemorySessionStore, SessionId) {
        let mut store = InMemorySessionStore::new(SessionConfig {
            idle_timeout_seconds: 10,
            absolute_timeout_seconds: 100,
            ..SessionConfig::default()
        });
        let session = store.create_session(
            SessionCreateInput {
                username: "alice".to_owned(),
                uid: 1000,
                gid: 1000,
                user_agent: Some("agent".to_owned()),
                ip: Some("127.0.0.1".to_owned()),
            },
            now,
        );
        (store, session.session_id)
    }

    #[test]
    fn missing_session_rejected() {
        let (store, _) = store_with_session(100);
        let request = WsUpgradeRequest {
            session_id: None,
            origin: Some("https://example.test".to_owned()),
            user_agent: None,
            ip: None,
            now: 100,
        };

        assert!(matches!(
            accept_session_bound_upgrade(
                &store,
                &SecurityConfig::default(),
                FeatureFlags::default(),
                request
            ),
            Err(TealDriveError::PermissionDenied)
        ));
    }

    #[test]
    fn expired_session_rejected() {
        let (store, session_id) = store_with_session(100);
        let request = WsUpgradeRequest {
            session_id: Some(session_id),
            origin: Some("https://example.test".to_owned()),
            user_agent: None,
            ip: None,
            now: 111,
        };

        assert!(matches!(
            accept_session_bound_upgrade(
                &store,
                &SecurityConfig::default(),
                FeatureFlags::default(),
                request
            ),
            Err(TealDriveError::PermissionDenied)
        ));
    }

    #[test]
    fn revoked_session_rejected() {
        let (mut store, session_id) = store_with_session(100);
        store.revoke_session(&session_id);
        let request = WsUpgradeRequest {
            session_id: Some(session_id),
            origin: Some("https://example.test".to_owned()),
            user_agent: None,
            ip: None,
            now: 100,
        };

        assert!(matches!(
            accept_session_bound_upgrade(
                &store,
                &SecurityConfig::default(),
                FeatureFlags::default(),
                request
            ),
            Err(TealDriveError::PermissionDenied)
        ));
    }

    #[test]
    fn valid_session_accepted_into_context() {
        let (store, session_id) = store_with_session(100);
        let request = WsUpgradeRequest {
            session_id: Some(session_id),
            origin: Some("https://example.test".to_owned()),
            user_agent: Some("browser".to_owned()),
            ip: Some("10.0.0.1".to_owned()),
            now: 100,
        };
        let context = accept_session_bound_upgrade(
            &store,
            &SecurityConfig::default(),
            FeatureFlags::default(),
            request,
        )
        .expect("context");

        assert_eq!(
            context.user_context(),
            UserContext {
                username: "alice".to_owned(),
                uid: 1000,
                gid: 1000,
            }
        );
        assert!(!context.protocol_ready);
    }
}
