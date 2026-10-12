use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::errors::TealDriveError;
use crate::helper::ipc::{encode_invocation, HelperInvocation};
use crate::helper::types::{HelperRequest, HelperResponse};

#[derive(Debug, Clone)]
pub struct HelperProcessClient {
    helper_path: PathBuf,
    timeout_millis: Arc<AtomicU64>,
    env_vars: Vec<(String, String)>,
    tickets: Arc<
        std::sync::Mutex<
            std::collections::HashMap<String, crate::helper::security::IdentityTicket>,
        >,
    >,
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
            tickets: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
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

    pub fn authenticate(
        &self,
        request: &crate::auth::account::AuthRequest,
    ) -> Result<crate::auth::account::AuthSuccess, crate::auth::account::AuthError> {
        use crate::auth::account::AuthError;
        let bytes = encode_invocation(&HelperInvocation::Authenticate(request.clone()))
            .map_err(|_| AuthError::AuthBackendUnavailable)?;
        let output = self
            .communicate(bytes)
            .map_err(|_| AuthError::AuthBackendUnavailable)?;
        let response: crate::helper::ipc::HelperAuthentication =
            rmp_serde::from_slice(&output).map_err(|_| AuthError::AuthBackendUnavailable)?;
        match response.result {
            Ok((success, ticket)) => {
                self.tickets
                    .lock()
                    .map_err(|_| AuthError::AuthBackendUnavailable)?
                    .insert(success.username.clone(), ticket);
                Ok(success)
            }
            Err(crate::auth::account::AuthFailure::AccountNotAllowed) => {
                Err(AuthError::AccountNotAllowed)
            }
            Err(crate::auth::account::AuthFailure::AuthBackendUnavailable) => {
                Err(AuthError::AuthBackendUnavailable)
            }
            Err(_) => Err(AuthError::InvalidCredentials),
        }
    }
    /// Root gateway commands; the helper selects its protected policy and DB itself.
    pub fn account_command(
        &self,
        command: crate::helper::account_gateway::AccountCommand,
    ) -> Result<
        crate::helper::account_gateway::AccountReply,
        crate::helper::account_gateway::GatewayFailure,
    > {
        use crate::helper::account_gateway::GatewayFailure;
        let bytes = encode_invocation(&HelperInvocation::Account(command))
            .map_err(|_| GatewayFailure::Unavailable)?;
        let output = zeroize::Zeroizing::new(
            self.communicate(bytes)
                .map_err(|_| GatewayFailure::Unavailable)?,
        );
        rmp_serde::from_slice(&output).map_err(|_| GatewayFailure::Unavailable)?
    }
    pub fn execute(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        let mut ticket = self
            .tickets
            .lock()
            .map_err(|_| TealDriveError::Internal)?
            .get(&request.username)
            .cloned();
        let policy = crate::helper::policy::current();
        let revision = policy.as_ref().map(|p| p.revision);
        let grant =
            crate::session::lease::current_cancellation().and_then(|context| context.grant());
        // A grant operation must never downgrade to a cached PAM ticket.
        if grant.is_some() {
            ticket = None;
        }
        let bytes = encode_invocation(&HelperInvocation::Operation {
            grant,
            request: Box::new(request.clone()),
            ticket,
            policy: policy.map(Box::new),
        })?;
        let output = self.communicate(bytes).map_err(|error| {
            // Safe enum only: never include the request, ticket, grant or password.
            eprintln!("Helper operation failed: {error:?}");
            error
        })?;
        let response: HelperResponse =
            rmp_serde::from_slice(&output).map_err(|_| TealDriveError::HelperMalformedResponse)?;
        crate::audit::operation::observe(&response);
        if response.success && response.policy_revision != revision {
            return Err(TealDriveError::HelperMalformedResponse);
        }
        Ok(response)
    }
    /// Read and write both pipes while the child runs. All exits kill/reap the child
    /// and join pipe workers; timeout includes input writes, authentication and output.
    fn communicate(&self, bytes: Vec<u8>) -> Result<Vec<u8>, TealDriveError> {
        let bytes = zeroize::Zeroizing::new(bytes);
        let start = Instant::now();
        let timeout = self.timeout();
        let cancellation = crate::session::lease::current_cancellation();
        if cancellation.as_ref().is_some_and(|c| c.is_cancelled()) {
            return Err(TealDriveError::PermissionDenied);
        }
        let mut command = Command::new(&self.helper_path);
        command
            .arg("--watch-stdin")
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for (key, value) in &self.env_vars {
            command.env(key, value);
        }
        let mut child = command
            .spawn()
            .map_err(|_| TealDriveError::HelperSpawnFailed)?;
        let stdin = child
            .stdin
            .take()
            .ok_or(TealDriveError::HelperStdinFailed)?;
        let stdout = child
            .stdout
            .take()
            .ok_or(TealDriveError::HelperStdoutFailed)?;
        thread::scope(|scope| {
            let (write_tx, write_rx) = std::sync::mpsc::channel();
            let (read_tx, read_rx) = std::sync::mpsc::channel();
            scope.spawn(move || {
                let mut stdin = stdin;
                let result = stdin
                    .write_all(&bytes)
                    .map_err(|_| TealDriveError::HelperStdinFailed);
                // Keep the pipe open until completion; closing it cancels the helper.
                let _ = write_tx.send(result.map(|()| stdin));
            });
            scope.spawn(move || {
                let mut stdout = stdout;
                let _ = read_tx.send(crate::helper::ipc::read_wire_payload(&mut stdout));
            });
            let mut input = None;
            let result = (|| {
                let mut output = None;
                let mut written = false;
                loop {
                    if !written {
                        if let Ok(result) = write_rx.try_recv() {
                            input = Some(result?);
                            written = true;
                        }
                    }
                    if output.is_none() {
                        if let Ok(result) = read_rx.try_recv() {
                            output = Some(result?);
                        }
                    }
                    if let Some(status) = child
                        .try_wait()
                        .map_err(|_| TealDriveError::HelperNonZeroExit)?
                    {
                        if !status.success() {
                            return Err(TealDriveError::HelperNonZeroExit);
                        }
                        if written && output.is_some() {
                            return Ok(output.take().unwrap());
                        }
                    }
                    if cancellation.as_ref().is_some_and(|c| c.is_cancelled()) {
                        return Err(TealDriveError::PermissionDenied);
                    }
                    if start.elapsed() >= timeout {
                        return Err(TealDriveError::HelperTimeout);
                    }
                    thread::sleep(Duration::from_millis(2));
                }
            })();
            // EOF cancellation works even when kill(2) is denied after UID switching.
            drop(input);
            drop(write_rx);
            if result.is_err() {
                let cleanup_deadline = Instant::now() + Duration::from_millis(1200);
                while Instant::now() < cleanup_deadline {
                    if matches!(child.try_wait(), Ok(Some(_))) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(2));
                }
                let _ = child.kill();
            }
            let _ = child.wait();
            result
        })
    }
}

pub fn default_helper_binary_path() -> PathBuf {
    if let Ok(path) = std::env::var("TEALDRIVE_HELPER_PATH") {
        return PathBuf::from(path);
    }

    let central = PathBuf::from(crate::layout::HOME)
        .join("bin")
        .join(helper_binary_name());
    if central.exists() {
        return central;
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
        let client = HelperProcessClient::new("/bin/true", Duration::from_secs(10));
        assert_eq!(client.timeout(), Duration::from_secs(10));

        let client_clone = client.clone();
        client.set_timeout(Duration::from_secs(25));

        assert_eq!(client.timeout(), Duration::from_secs(25));
        assert_eq!(client_clone.timeout(), Duration::from_secs(25));
    }
}

#[cfg(test)]
mod pipe_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    fn script(code: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("tdrv-pipe-test-{}", uuid::Uuid::new_v4()));
        std::fs::write(&path, format!("#!/usr/bin/python3\n{code}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        path
    }
    #[test]
    fn drains_large_output_while_writing_large_input() {
        let path=script("import sys,struct\nsys.stdout.buffer.write(struct.pack('>I',1048576)+b'x'*1048576)\nsys.stdout.buffer.flush()\nsys.stdin.buffer.read(2*1024*1024)");
        let client = HelperProcessClient::new(&path, Duration::from_secs(3));
        assert_eq!(
            client.communicate(vec![1; 2 * 1024 * 1024]).unwrap().len(),
            1024 * 1024
        );
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn timeout_also_covers_blocked_input_pipe() {
        let path = script("import time\ntime.sleep(60)");
        let client = HelperProcessClient::new(&path, Duration::from_millis(100));
        assert_eq!(
            client.communicate(vec![1; 1024 * 1024]),
            Err(TealDriveError::HelperTimeout)
        );
        std::fs::remove_file(path).unwrap();
    }
}
