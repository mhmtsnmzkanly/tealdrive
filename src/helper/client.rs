use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::errors::TealDriveError;
use crate::helper::ipc::{encode_helper_request, read_helper_response};
use crate::helper::types::{HelperRequest, HelperResponse};

#[derive(Debug, Clone)]
pub struct HelperProcessClient {
    helper_path: PathBuf,
    timeout_millis: Arc<AtomicU64>,
    env_vars: Vec<(String, String)>,
}

impl PartialEq for HelperProcessClient {
    fn eq(&self, other: &Self) -> bool {
        self.helper_path == other.helper_path
            && self.timeout_millis.load(Ordering::Relaxed)
                == other.timeout_millis.load(Ordering::Relaxed)
            && self.env_vars == other.env_vars
    }
}

impl Eq for HelperProcessClient {}

impl HelperProcessClient {
    pub fn new(helper_path: impl Into<PathBuf>, timeout: Duration) -> Self {
        Self {
            helper_path: helper_path.into(),
            timeout_millis: Arc::new(AtomicU64::new(timeout.as_millis() as u64)),
            env_vars: Vec::new(),
        }
    }

    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_millis.load(Ordering::Relaxed))
    }

    pub fn set_timeout(&self, timeout: Duration) {
        self.timeout_millis
            .store(timeout.as_millis() as u64, Ordering::Relaxed);
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.push((key.into(), value.into()));
        self
    }

    pub fn execute(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        let request_bytes = encode_helper_request(request)?;
        let mut cmd = Command::new(&self.helper_path);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, val) in &self.env_vars {
            cmd.env(key, val);
        }
        let mut child = cmd.spawn()
            .map_err(|_| TealDriveError::HelperSpawnFailed)?;

        {
            let stdin = child
                .stdin
                .as_mut()
                .ok_or(TealDriveError::HelperStdinFailed)?;
            stdin
                .write_all(&request_bytes)
                .map_err(|_| TealDriveError::HelperStdinFailed)?;
        }
        drop(child.stdin.take());

        let timeout = self.timeout();
        let start = Instant::now();
        loop {
            if let Some(status) = child
                .try_wait()
                .map_err(|_| TealDriveError::HelperNonZeroExit)?
            {
                if !status.success() {
                    return Err(TealDriveError::HelperNonZeroExit);
                }
                let mut stdout = child
                    .stdout
                    .take()
                    .ok_or(TealDriveError::HelperStdoutFailed)?;
                return read_helper_response(&mut stdout)
                    .map_err(|_| TealDriveError::HelperMalformedResponse);
            }

            if start.elapsed() >= timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(TealDriveError::HelperTimeout);
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}

pub fn default_helper_binary_path() -> PathBuf {
    if let Ok(path) = std::env::var("TEALDRIVE_HELPER_PATH") {
        return PathBuf::from(path);
    }

    let mut path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("tealdrive-server"));
    path.set_file_name(helper_binary_name());
    path
}

fn helper_binary_name() -> String {
    if cfg!(windows) {
        "tealdrive-helper.exe".to_owned()
    } else {
        "tealdrive-helper".to_owned()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelperClient;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_existing_helper_maps_to_spawn_failed() {
        let client = HelperProcessClient::new(
            "/definitely/not/a/tealdrive-helper",
            Duration::from_millis(50),
        );
        let request = crate::helper::ipc::tests_support::sample_request(
            crate::helper::types::HelperCommand::ListDirectory,
        );

        assert_eq!(
            client.execute(&request),
            Err(TealDriveError::HelperSpawnFailed)
        );
    }

    #[test]
    fn atomic_timeout_update_works() {
        let client = HelperProcessClient::new(
            "/bin/true",
            Duration::from_secs(10),
        );
        assert_eq!(client.timeout(), Duration::from_secs(10));

        let client_clone = client.clone();
        client.set_timeout(Duration::from_secs(25));

        assert_eq!(client.timeout(), Duration::from_secs(25));
        assert_eq!(client_clone.timeout(), Duration::from_secs(25));
    }
}
