//! The parent keeps stdin open while the helper runs. EOF is an unprivileged
//! cancellation channel that still works after the helper switches to another UID.
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
static CANCELLED: AtomicBool = AtomicBool::new(false);

pub fn check() -> io::Result<()> {
    if CANCELLED.load(Ordering::Acquire) {
        Err(io::Error::other("helper operation cancelled"))
    } else {
        Ok(())
    }
}

/// Only the standalone helper opts into watching its inherited stdin pipe.
/// The watchdog also bounds uninterruptible application code (for example PAM).
pub fn watch_parent() {
    std::thread::spawn(|| {
        let mut descriptor = libc::pollfd {
            fd: libc::STDIN_FILENO,
            events: 0, // POLLHUP is reported without consuming the request bytes.
            revents: 0,
        };
        loop {
            let result = unsafe { libc::poll(&mut descriptor, 1, 100) };
            if result > 0
                && descriptor.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0
            {
                CANCELLED.store(true, Ordering::Release);
                let deadline = Instant::now() + Duration::from_secs(1);
                while Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(10));
                }
                // Cooperative cancellation normally returns through transaction
                // cleanup first. A stuck helper cannot outlive the parent indefinitely.
                std::process::exit(1);
            }
        }
    });
}
