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
    cancelled: AtomicBool,
    notify: Notify,
}
impl Cancellation {
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
pub struct SessionLease {
    pub cancellation: Cancellation,
    task: tokio::task::JoinHandle<()>,
}
impl SessionLease {
    pub fn new(store: Arc<Mutex<InMemorySessionStore>>, id: SessionId) -> Self {
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
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
        });
        Self { cancellation, task }
    }
}
impl Drop for SessionLease {
    fn drop(&mut self) {
        self.cancellation.cancel();
        self.task.abort();
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
}
