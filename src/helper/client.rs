use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::errors::TealDriveError;
use crate::helper::ipc::{encode_helper_request, read_helper_response};
use crate::helper::types::{HelperRequest, HelperResponse};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperProcessClient {
    helper_path: PathBuf,
    timeout: Duration,
}

impl HelperProcessClient {
    pub fn new(helper_path: impl Into<PathBuf>, timeout: Duration) -> Self {
        Self {
            helper_path: helper_path.into(),
            timeout,
        }
    }

    pub fn execute(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        let request_bytes = encode_helper_request(request)?;
        let mut child = Command::new(&self.helper_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
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

            if start.elapsed() >= self.timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(TealDriveError::HelperTimeout);
            }
            thread::sleep(Duration::from_millis(5));
        }
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
}
