//! A connection/transfer owns a revocable lease, checked independently of I/O.
use crate::session::store::{InMemorySessionStore, SessionId, SessionLookupResult, SessionStore};
use std::cell::RefCell;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::{Mutex, Notify};
#[derive(Clone, Default)]
pub struct Cancellation(Arc<CancelState>);
#[derive(Default)]
struct CancelState {
    grant: std::sync::Mutex<Option<crate::accounts::grants::SessionGrant>>,
    cancelled: AtomicBool,
    notify: Notify,
}
impl Cancellation {
    pub fn grant(&self) -> Option<crate::accounts::grants::SessionGrant> {
        self.0.grant.lock().ok().and_then(|grant| grant.clone())
    }

    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
        self.0.notify.notify_waiters();
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }
    pub async fn cancelled(&self) {
        loop {
            let notified = self.0.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}
thread_local! { static CANCEL:RefCell<Option<Cancellation>>=const { RefCell::new(None) }; }
pub fn current_cancellation() -> Option<Cancellation> {
    CANCEL.with(|s| s.borrow().clone())
}
pub fn with_cancellation<T>(token: Cancellation, f: impl FnOnce() -> T) -> T {
    struct Reset(Option<Cancellation>);
    impl Drop for Reset {
        fn drop(&mut self) {
            CANCEL.with(|s| *s.borrow_mut() = self.0.take());
        }
    }
    let _reset = Reset(CANCEL.with(|s| s.replace(Some(token))));
    f()
}
pub fn validation_slots() -> Arc<tokio::sync::Semaphore> {
    static SLOTS: std::sync::LazyLock<Arc<tokio::sync::Semaphore>> =
        std::sync::LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(8)));
    SLOTS.clone()
}
pub struct SessionLease {
    pub cancellation: Cancellation,
    task: tokio::task::JoinHandle<()>,
    validation_task: Option<tokio::task::JoinHandle<()>>,
}
impl SessionLease {
    /// Bind authorization before returning; chunk workers share this immutable lease context.
    pub async fn with_shutdown_for_session(
        store: Arc<Mutex<InMemorySessionStore>>,
        id: SessionId,
        shutdown: Cancellation,
        authenticator: Arc<dyn crate::auth::Authenticator>,
    ) -> Self {
        let session = store
            .lock()
            .await
            .get_session(&id, InMemorySessionStore::now_epoch_seconds());
        let mut lease = Self::with_shutdown(store.clone(), id, shutdown);
        match session {
            SessionLookupResult::Found(session) => {
                let grant = session.grant.as_deref().cloned();
                *lease.cancellation.0.grant.lock().expect("new lease mutex") = grant.clone();
                if let Some(grant) = grant {
                    let cancellation = lease.cancellation.clone();
                    lease.validation_task = Some(tokio::spawn(async move {
                        loop {
                            let slots = validation_slots();
                            let permit = tokio::select! {
                                _ = cancellation.cancelled() => return,
                                permit = slots.acquire_owned() => match permit { Ok(permit) => permit, Err(_) => {cancellation.cancel();return;} },
                            };
                            let auth = authenticator.clone();
                            let proof = grant.clone();
                            let token = cancellation.clone();
                            let result = tokio::task::spawn_blocking(move || {
                                let _permit = permit;
                                with_cancellation(token, || {
                                    auth.account_command(
                                        crate::helper::account_gateway::AccountCommand::Validate(
                                            proof,
                                        ),
                                    )
                                })
                            })
                            .await;
                            let valid = matches!(result, Ok(Ok(crate::helper::account_gateway::AccountReply::Validated(ref account)))
                                if account.username == session.username && account.uid == session.uid && account.gid == session.gid
                                    && account.id == grant.account_id && account.generation == grant.generation);
                            if !valid {
                                store.lock().await.revoke_session(&id);
                                cancellation.cancel();
                                return;
                            }
                            tokio::select! {
                                _ = cancellation.cancelled() => return,
                                _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {},
                            }
                        }
                    }));
                }
            }
            _ => lease.cancellation.cancel(),
        }
        lease
    }

    pub fn new(store: Arc<Mutex<InMemorySessionStore>>, id: SessionId) -> Self {
        Self::with_shutdown(store, id, Cancellation::default())
    }
    pub fn with_shutdown(
        store: Arc<Mutex<InMemorySessionStore>>,
        id: SessionId,
        shutdown: Cancellation,
    ) -> Self {
        let cancellation = Cancellation::default();
        let token = cancellation.clone();
        let task = tokio::spawn(async move {
            loop {
                let valid = matches!(
                    store
                        .lock()
                        .await
                        .get_session(&id, InMemorySessionStore::now_epoch_seconds()),
                    SessionLookupResult::Found(_)
                );
                if !valid {
                    token.cancel();
                    break;
                }
                tokio::select! {
                    _ = shutdown.cancelled() => { token.cancel(); break; },
                    _ = tokio::time::sleep(std::time::Duration::from_millis(250)) => {},
                }
            }
        });
        Self {
            cancellation,
            task,
            validation_task: None,
        }
    }
}
impl Drop for SessionLease {
    fn drop(&mut self) {
        self.cancellation.cancel();
        self.task.abort();
        if let Some(task) = &self.validation_task {
            task.abort();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn revocation_cancels_an_existing_lease() {
        let store = Arc::new(Mutex::new(InMemorySessionStore::new(Default::default())));
        let s = store.lock().await.create_session(
            crate::session::store::SessionCreateInput {
                username: "alice".into(),
                uid: 1000,
                gid: 1000,
                user_agent: None,
                ip: None,
            },
            InMemorySessionStore::now_epoch_seconds(),
        );
        let lease = SessionLease::new(store.clone(), s.session_id);
        store.lock().await.revoke_session(&s.session_id);
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            lease.cancellation.cancelled(),
        )
        .await
        .unwrap();
    }
    struct Validator {
        account: crate::accounts::store::Account,
        revoked: Arc<AtomicBool>,
    }
    impl crate::auth::Authenticator for Validator {
        fn authenticate(
            &self,
            _: &crate::auth::AuthRequest,
        ) -> Result<crate::auth::AuthSuccess, crate::auth::AuthError> {
            Err(crate::auth::AuthError::AuthBackendUnavailable)
        }
        fn account_command(
            &self,
            _: crate::helper::account_gateway::AccountCommand,
        ) -> Result<
            crate::helper::account_gateway::AccountReply,
            crate::helper::account_gateway::GatewayFailure,
        > {
            if self.revoked.load(Ordering::Acquire) {
                Err(crate::helper::account_gateway::GatewayFailure::Forbidden)
            } else {
                Ok(crate::helper::account_gateway::AccountReply::Validated(
                    self.account.clone(),
                ))
            }
        }
    }
    #[tokio::test]
    async fn root_revocation_cancels_active_lease_and_secret_context_is_scoped() {
        use crate::accounts::{
            grants::{CallerBinding, SessionGrant},
            store::{AccountKind, AccountStore, Authority, NewAccount, Role},
        };
        let mut database = AccountStore::memory();
        let account = database
            .create(
                &Authority::Root,
                NewAccount {
                    username: "alice".into(),
                    uid: 1000,
                    gid: 1000,
                    home: "/home/alice".into(),
                    kind: AccountKind::Linux,
                    role: Role::User,
                    owner_admin_id: None,
                    quota_bytes: None,
                },
            )
            .unwrap();
        let grant = SessionGrant {
            nonce: "secret-lease-nonce".into(),
            account_id: account.id.clone(),
            generation: account.generation,
            caller: CallerBinding {
                uid: 995,
                pid: 1,
                start_time: 1,
                boot_id: "00000000-0000-4000-8000-000000000001".into(),
            },
            expires_at: InMemorySessionStore::now_epoch_seconds() + 60,
        };
        let store = Arc::new(Mutex::new(InMemorySessionStore::new(Default::default())));
        let session = store.lock().await.create_session(
            crate::session::store::SessionCreateInput {
                username: "alice".into(),
                uid: 1000,
                gid: 1000,
                user_agent: None,
                ip: None,
            },
            InMemorySessionStore::now_epoch_seconds(),
        );
        assert!(store
            .lock()
            .await
            .attach_grant(&session.session_id, grant.clone()));
        let snapshot = store.lock().await.get_session(
            &session.session_id,
            InMemorySessionStore::now_epoch_seconds(),
        );
        let SessionLookupResult::Found(snapshot) = snapshot else {
            panic!("session missing")
        };
        assert!(!serde_json::to_string(&snapshot)
            .unwrap()
            .contains("secret-lease-nonce"));
        let revoked = Arc::new(AtomicBool::new(false));
        let lease = SessionLease::with_shutdown_for_session(
            store.clone(),
            session.session_id,
            Cancellation::default(),
            Arc::new(Validator {
                account,
                revoked: revoked.clone(),
            }),
        )
        .await;
        assert!(current_cancellation().is_none());
        with_cancellation(lease.cancellation.clone(), || {
            assert_eq!(current_cancellation().unwrap().grant(), Some(grant.clone()));
            with_cancellation(Cancellation::default(), || {
                assert!(current_cancellation().unwrap().grant().is_none())
            });
            assert_eq!(current_cancellation().unwrap().grant(), Some(grant));
        });
        assert!(current_cancellation().is_none());
        revoked.store(true, Ordering::Release);
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            lease.cancellation.cancelled(),
        )
        .await
        .unwrap();
        assert!(matches!(
            store.lock().await.get_session(
                &session.session_id,
                InMemorySessionStore::now_epoch_seconds()
            ),
            SessionLookupResult::Revoked
        ));
    }
}
