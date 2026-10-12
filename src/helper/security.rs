//! Authentication capabilities are issued only by the privileged PAM boundary.
use crate::auth::account::{AuthError, AuthRequest, AuthSuccess};
use crate::config::{AppConfig, UserContext};
use crate::session::store::InMemorySessionStore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::{CStr, CString};
use std::io;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

pub const TRUSTED_CONFIG: &str = crate::layout::HELPER_CONFIG;

#[derive(Clone, Serialize, Deserialize)]
pub struct IdentityTicket {
    pub identity: UserContext,
    pub caller_uid: u32,
    pub expires_at: u64,
    pub nonce: [u8; 16],
    pub mac: [u8; 32],
}
impl std::fmt::Debug for IdentityTicket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IdentityTicket([REDACTED])")
    }
}
fn payload(ticket: &IdentityTicket) -> Vec<u8> {
    rmp_serde::to_vec_named(&(
        &ticket.identity,
        ticket.caller_uid,
        ticket.expires_at,
        ticket.nonce,
    ))
    .expect("serializable identity")
}
fn hmac(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut block = [0u8; 64];
    if key.len() > 64 {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let inner_key = block.map(|b| b ^ 0x36);
    let outer_key = block.map(|b| b ^ 0x5c);
    let mut inner = Sha256::new();
    inner.update(inner_key);
    inner.update(message);
    let mut outer = Sha256::new();
    outer.update(outer_key);
    outer.update(inner.finalize());
    outer.finalize().into()
}
fn equal(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |diff, (a, b)| diff | (a ^ b)) == 0
}
pub fn trusted_read(path: &Path, secret: bool) -> io::Result<Vec<u8>> {
    let mut parent = path.parent();
    while let Some(dir) = parent {
        let m = std::fs::symlink_metadata(dir)?;
        if !m.is_dir() || m.uid() != 0 || m.mode() & 0o022 != 0 {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        parent = dir.parent();
    }
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    let m = file.metadata()?;
    if !m.is_file() || m.uid() != 0 || m.mode() & (if secret { 0o077 } else { 0o022 }) != 0 {
        return Err(io::Error::from(io::ErrorKind::PermissionDenied));
    }
    use std::io::Read;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err(io::Error::other("trusted file too large"));
    }
    Ok(bytes)
}
pub fn trusted_config() -> io::Result<AppConfig> {
    let (path, _) = crate::layout::trusted_helper_paths();
    let bytes = trusted_read(&path, false)?;
    let config: AppConfig = toml::from_str(std::str::from_utf8(&bytes).map_err(io::Error::other)?)
        .map_err(io::Error::other)?;
    config.validate().map_err(io::Error::other)?;
    Ok(config)
}
fn key() -> io::Result<Vec<u8>> {
    let (_, path) = crate::layout::trusted_helper_paths();
    let key = trusted_read(&path, true)?;
    if key.len() != 32 {
        return Err(io::Error::other("helper key must contain 32 random bytes"));
    }
    Ok(key)
}
pub fn authenticate(request: &AuthRequest) -> Result<(AuthSuccess, IdentityTicket), AuthError> {
    if unsafe { libc::geteuid() == 0 || libc::geteuid() != libc::getuid() } {
        return issue(request);
    }
    let config = trusted_config().unwrap_or_default();
    let success = crate::auth::pam::authenticate_local(request, &config.account_policy)?;
    if success.uid != unsafe { libc::getuid() } || success.primary_gid != unsafe { libc::getgid() }
    {
        return Err(AuthError::InvalidCredentials);
    }
    let ticket = IdentityTicket {
        identity: UserContext {
            username: success.username.clone(),
            uid: success.uid,
            gid: success.primary_gid,
        },
        caller_uid: success.uid,
        expires_at: InMemorySessionStore::now_epoch_seconds()
            .saturating_add(config.session.absolute_timeout_seconds),
        nonce: *uuid::Uuid::new_v4().as_bytes(),
        mac: [0; 32],
    };
    Ok((success, ticket))
}
pub fn issue(request: &AuthRequest) -> Result<(AuthSuccess, IdentityTicket), AuthError> {
    let config = trusted_config().map_err(|_| AuthError::AuthBackendUnavailable)?;
    let success = crate::auth::pam::authenticate_local(request, &config.account_policy)?;
    let key = key().map_err(|_| AuthError::AuthBackendUnavailable)?;
    let mut ticket = IdentityTicket {
        identity: UserContext {
            username: success.username.clone(),
            uid: success.uid,
            gid: success.primary_gid,
        },
        caller_uid: unsafe { libc::getuid() },
        expires_at: InMemorySessionStore::now_epoch_seconds()
            .saturating_add(config.session.absolute_timeout_seconds),
        nonce: *uuid::Uuid::new_v4().as_bytes(),
        mac: [0; 32],
    };
    ticket.mac = hmac(&key, &payload(&ticket));
    Ok((success, ticket))
}
pub fn verify(ticket: &IdentityTicket, user: &UserContext) -> io::Result<()> {
    if !trusted_config()?
        .account_policy
        .allows_login(&user.username)
        || ticket.identity != *user
        || ticket.caller_uid != unsafe { libc::getuid() }
        || ticket.expires_at <= InMemorySessionStore::now_epoch_seconds()
        || !equal(&ticket.mac, &hmac(&key()?, &payload(ticket)))
    {
        return Err(io::Error::from(io::ErrorKind::PermissionDenied));
    }
    Ok(())
}
pub fn account(username: &str) -> Result<AuthSuccess, AuthError> {
    let name = CString::new(username).map_err(|_| AuthError::InvalidCredentials)?;
    let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
    let mut result = std::ptr::null_mut();
    let mut buffer = vec![0u8; 65536];
    let code = unsafe {
        libc::getpwnam_r(
            name.as_ptr(),
            &mut pwd,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        )
    };
    if code != 0 || result.is_null() {
        return Err(AuthError::InvalidCredentials);
    }
    let string =
        |p: *const libc::c_char| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    Ok(AuthSuccess {
        username: string(pwd.pw_name),
        uid: pwd.pw_uid,
        primary_gid: pwd.pw_gid,
        home_dir: Some(string(pwd.pw_dir).into()),
        shell: Some(string(pwd.pw_shell)),
        groups: Vec::new(),
    })
}
pub fn enter_identity(user: &UserContext) -> io::Result<()> {
    if user.uid == 0 {
        return Err(io::Error::from(io::ErrorKind::PermissionDenied));
    }
    let actual = account(&user.username).map_err(io::Error::other)?;
    if actual.uid != user.uid || actual.primary_gid != user.gid {
        return Err(io::Error::from(io::ErrorKind::PermissionDenied));
    }
    unsafe {
        if libc::geteuid() == 0 {
            let name = CString::new(user.username.as_str()).map_err(io::Error::other)?;
            if libc::initgroups(name.as_ptr(), user.gid) != 0
                || libc::setresgid(user.gid, user.gid, user.gid) != 0
                || libc::setresuid(user.uid, user.uid, user.uid) != 0
            {
                return Err(io::Error::last_os_error());
            }
        } else if libc::getuid() != user.uid || libc::getgid() != user.gid {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rfc4231_hmac_vector() {
        let result = hmac(&[0x0b; 20], b"Hi There");
        assert_eq!(
            result
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }
    #[test]
    fn ticket_mac_binds_identity_and_expiry() {
        let mut t = IdentityTicket {
            identity: UserContext {
                username: "alice".into(),
                uid: 1000,
                gid: 1000,
            },
            caller_uid: 1001,
            expires_at: 99,
            nonce: [1; 16],
            mac: [0; 32],
        };
        let mac = hmac(b"test", &payload(&t));
        t.identity.uid = 1002;
        assert!(!equal(&mac, &hmac(b"test", &payload(&t))));
        t.identity.uid = 1000;
        t.expires_at = 100;
        assert!(!equal(&mac, &hmac(b"test", &payload(&t))));
    }
}
