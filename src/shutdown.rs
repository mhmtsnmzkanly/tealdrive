//! Shared admission, draining and cancellation for HTTP, WebSockets and helper jobs.
use crate::session::lease::Cancellation;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Notify;

pub const DRAIN_TIMEOUT: Duration = Duration::from_secs(30);
pub const CANCEL_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Default)]
pub struct Shutdown(Arc<Inner>);
#[derive(Default)]
struct Inner {
    state: Mutex<State>,
    changed: Notify,
    draining: Cancellation,
    forced: Cancellation,
}
#[derive(Default)]
struct State {
    draining: bool,
    active: usize,
}
pub struct Activity(Shutdown);
impl Drop for Activity {
    fn drop(&mut self) {
        self.0 .0.state.lock().unwrap().active -= 1;
        self.0 .0.changed.notify_waiters();
    }
}
impl Shutdown {
    /// Admission and shutdown are serialized so no untracked work can slip in.
    pub fn enter(&self) -> Option<Activity> {
        let mut state = self.0.state.lock().unwrap();
        if state.draining {
            return None;
        }
        state.active += 1;
        Some(Activity(self.clone()))
    }
    /// Already-admitted HTTP streams may finish during draining.
    pub fn enter_existing(&self) -> Activity {
        self.0.state.lock().unwrap().active += 1;
        Activity(self.clone())
    }
    pub fn begin(&self) {
        self.0.state.lock().unwrap().draining = true;
        self.0.draining.cancel();
    }
    pub fn is_draining(&self) -> bool {
        self.0.draining.is_cancelled()
    }
    pub async fn draining(&self) {
        self.0.draining.cancelled().await;
    }
    pub fn cancellation(&self) -> Cancellation {
        self.0.forced.clone()
    }
    pub async fn idle(&self) {
        loop {
            let changed = self.0.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.0.state.lock().unwrap().active == 0 {
                return;
            }
            changed.await;
        }
    }
    /// Returns false only when work failed to stop even after cancellation.
    pub async fn finish(&self, drain: Duration, cancel: Duration) -> bool {
        self.begin();
        if tokio::time::timeout(drain, self.idle()).await.is_ok() {
            return true;
        }
        eprintln!("Shutdown grace period expired; cancelling remaining work.");
        self.0.forced.cancel();
        tokio::time::timeout(cancel, self.idle()).await.is_ok()
    }
}

pub async fn signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("install SIGTERM handler");
        tokio::select! {
            result = tokio::signal::ctrl_c() => result.expect("receive Ctrl+C"),
            _ = terminate.recv() => {},
        }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await.expect("receive Ctrl+C");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn admission_closes_but_existing_work_can_finish() {
        let shutdown = Shutdown::default();
        let work = shutdown.enter().unwrap();
        shutdown.begin();
        assert!(shutdown.enter().is_none());
        assert!(!shutdown.cancellation().is_cancelled());
        assert!(
            tokio::time::timeout(Duration::from_millis(10), shutdown.idle())
                .await
                .is_err()
        );
        drop(work);
        assert!(
            shutdown
                .finish(Duration::from_millis(10), Duration::from_millis(10))
                .await
        );
        assert!(!shutdown.cancellation().is_cancelled());
    }
    #[tokio::test]
    async fn grace_expiry_cancels_and_waits_for_cleanup() {
        let shutdown = Shutdown::default();
        let work = shutdown.enter().unwrap();
        let token = shutdown.cancellation();
        let cleanup = tokio::spawn(async move {
            token.cancelled().await;
            drop(work);
        });
        assert!(
            shutdown
                .finish(Duration::from_millis(10), Duration::from_secs(1))
                .await
        );
        cleanup.await.unwrap();
        assert!(shutdown.cancellation().is_cancelled());
    }
    #[tokio::test]
    async fn uncooperative_work_has_a_bounded_wait() {
        let shutdown = Shutdown::default();
        let _work = shutdown.enter().unwrap();
        assert!(
            !shutdown
                .finish(Duration::from_millis(10), Duration::from_millis(10))
                .await
        );
    }
}
