//! Standard privilege separation: Root validates; a fresh process drops IDs and touches files.
use super::{
    ipc,
    types::{HelperRequest, HelperResponse},
};
use crate::{config::AppConfig, errors::TealDriveError};
use std::{
    io::Write,
    os::unix::fs::MetadataExt,
    process::{Command, Stdio},
};
#[derive(serde::Serialize, serde::Deserialize)]
pub struct WorkerInvocation {
    pub request: Box<HelperRequest>,
    pub config: Box<AppConfig>,
    pub available: Option<u64>,
    pub parent_pid: u32,
    pub caller_uid: u32,
    pub expires_at: u64,
    pub nonce: [u8; 16],
    pub mac: [u8; 32],
}
impl WorkerInvocation {
    fn payload(&self) -> Result<Vec<u8>, TealDriveError> {
        let mut result = b"tealdrive-internal-worker-v1\0".to_vec();
        result.extend(
            rmp_serde::to_vec_named(&(
                &self.request,
                &self.config,
                self.available,
                self.parent_pid,
                self.caller_uid,
                self.expires_at,
                self.nonce,
            ))
            .map_err(|_| TealDriveError::HelperDecodeFailed)?,
        );
        Ok(result)
    }
}
fn trusted_parent() -> std::io::Result<()> {
    if unsafe { libc::geteuid() } != 0 || unsafe { libc::getuid() } == 0 {
        return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
    }
    let parent = unsafe { libc::getppid() };
    let status = std::fs::read_to_string(format!("/proc/{parent}/status"))?;
    if status.len() > 65536 {
        return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
    }
    let ids = status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .ok_or(std::io::ErrorKind::PermissionDenied)?
        .split_whitespace()
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(std::io::Error::other)?;
    let me = std::fs::metadata("/proc/self/exe")?;
    // A private Root-created pipe avoids requiring CAP_SYS_PTRACE to inspect parent exe.
    let input = std::fs::metadata("/proc/self/fd/0")?;
    if ids.len() != 4
        || ids[0] != unsafe { libc::getuid() }
        || ids[1] != 0
        || me.uid() != 0
        || me.mode() & 0o022 != 0
        || input.uid() != 0
        || input.mode() & libc::S_IFMT != libc::S_IFIFO
    {
        return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
    }
    Ok(())
}
pub fn run(invocation: WorkerInvocation) -> Result<HelperResponse, TealDriveError> {
    // A panel process cannot invoke this private path, even with copied JSON/config.
    trusted_parent().map_err(|_| TealDriveError::PermissionDenied)?;
    if invocation.parent_pid != unsafe { libc::getppid() } as u32
        || invocation.caller_uid != unsafe { libc::getuid() }
        || invocation.expires_at <= crate::session::store::InMemorySessionStore::now_epoch_seconds()
        || !super::security::verify_worker(&invocation.payload()?, &invocation.mac)
            .map_err(|_| TealDriveError::PermissionDenied)?
    {
        return Err(TealDriveError::PermissionDenied);
    }
    let WorkerInvocation {
        request,
        config,
        available,
        ..
    } = invocation;
    config.validate()?;
    super::access::authorize(&request, &config)?;
    super::policy::authorize(&request, &config)?;
    execute(&request, &config, available)
}
pub(super) fn execute(
    request: &HelperRequest,
    config: &AppConfig,
    available: Option<u64>,
) -> Result<HelperResponse, TealDriveError> {
    let user = crate::config::UserContext {
        username: request.username.clone(),
        uid: request.uid,
        gid: request.gid,
    };
    if super::security::enter_identity(&user).is_err() {
        return Ok(HelperResponse::permission_denied(
            request,
            "Identity verification failed.",
        ));
    }
    let root = config
        .roots
        .iter()
        .find(|r| r.root_id == request.root_id)
        .ok_or(TealDriveError::PermissionDenied)?;
    if super::sandbox::confine(root).is_err() {
        return Ok(HelperResponse::permission_denied(
            request,
            "Kernel filesystem confinement is unavailable.",
        ));
    }
    Ok(super::quota::with_budget(available, || {
        super::access::with_context(request, config, || {
            super::observation::execute(request, config)
        })
    }))
}
pub(super) fn spawn(
    request: &HelperRequest,
    config: &AppConfig,
    available: Option<u64>,
) -> Result<HelperResponse, TealDriveError> {
    let child = Command::new("/proc/self/exe")
        .arg("--watch-stdin")
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| TealDriveError::Internal)?;
    struct Reap(std::process::Child);
    impl Drop for Reap {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = Reap(child);
    let input = child.0.stdin.take().ok_or(TealDriveError::Internal)?;
    let output = child.0.stdout.take().ok_or(TealDriveError::Internal)?;
    let mut input = input;
    let mut output = output;
    let mut invocation = WorkerInvocation {
        request: Box::new(request.clone()),
        config: Box::new(config.clone()),
        available,
        parent_pid: std::process::id(),
        caller_uid: unsafe { libc::getuid() },
        expires_at: crate::session::store::InMemorySessionStore::now_epoch_seconds()
            .saturating_add(30),
        nonce: *uuid::Uuid::new_v4().as_bytes(),
        mac: [0; 32],
    };
    invocation.mac = super::security::sign_worker(&invocation.payload()?)
        .map_err(|_| TealDriveError::PermissionDenied)?;
    input
        .write_all(&ipc::encode_invocation(&ipc::HelperInvocation::Worker(
            invocation,
        ))?)
        .map_err(|_| TealDriveError::Internal)?;
    // Keep stdin open until worker exits: EOF is its existing cancellation channel.
    let response = ipc::read_helper_response(&mut output)?;
    if !child
        .0
        .wait()
        .map_err(|_| TealDriveError::Internal)?
        .success()
    {
        return Err(TealDriveError::Internal);
    }
    drop(input);
    Ok(response)
}
#[cfg(test)]
mod tests {
    #[test]
    fn ordinary_process_cannot_claim_root_helper_parent() {
        assert!(super::trusted_parent().is_err());
    }
}
