//! Authentication capabilities are issued only by the privileged PAM boundary.
use crate::auth::account::{AuthError, AuthRequest, AuthSuccess};
use crate::config::{AppConfig, UserContext};
use crate::session::store::InMemorySessionStore;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::ffi::{CStr, CString};
use std::io;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

pub const TRUSTED_CONFIG: &str = crate::layout::HELPER_CONFIG;

#[derive(Clone, Serialize, Deserialize)]
pub struct IdentityTicket {
    pub identity: UserContext,
    pub caller_uid: u32,
    /// Kernel-derived server process identity; old unbound tickets are rejected.
    #[serde(default)]
    pub caller: Option<crate::accounts::grants::CallerBinding>,
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
        "tealdrive.identity-ticket.v2",
        &ticket.identity,
        ticket.caller_uid,
        &ticket.caller,
        ticket.expires_at,
        ticket.nonce,
    ))
    .expect("serializable identity")
}
fn hmac(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(message);
    mac.finalize().into_bytes().into()
}
fn verify_mac(key: &[u8], message: &[u8], expected: &[u8; 32]) -> bool {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(message);
    mac.verify_slice(expected).is_ok()
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
fn key() -> io::Result<zeroize::Zeroizing<Vec<u8>>> {
    let (_, path) = crate::layout::trusted_helper_paths();
    let key = zeroize::Zeroizing::new(trusted_read(&path, true)?);
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
    if config.web_accounts.enabled {
        return Err(AuthError::AuthBackendUnavailable);
    }
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
        caller: None,
        expires_at: InMemorySessionStore::now_epoch_seconds()
            .saturating_add(config.session.absolute_timeout_seconds),
        nonce: *uuid::Uuid::new_v4().as_bytes(),
        mac: [0; 32],
    };
    Ok((success, ticket))
}
pub fn issue(request: &AuthRequest) -> Result<(AuthSuccess, IdentityTicket), AuthError> {
    let config = trusted_config().map_err(|_| AuthError::AuthBackendUnavailable)?;
    if config.web_accounts.enabled {
        return Err(AuthError::AuthBackendUnavailable);
    }
    let success = crate::auth::pam::authenticate_local(request, &config.account_policy)?;
    let key = key().map_err(|_| AuthError::AuthBackendUnavailable)?;
    let mut ticket = IdentityTicket {
        identity: UserContext {
            username: success.username.clone(),
            uid: success.uid,
            gid: success.primary_gid,
        },
        caller_uid: unsafe { libc::getuid() },
        caller: Some(
            crate::accounts::grants::CallerBinding::capture()
                .map_err(|_| AuthError::AuthBackendUnavailable)?,
        ),
        expires_at: InMemorySessionStore::now_epoch_seconds()
            .saturating_add(config.session.absolute_timeout_seconds),
        nonce: *uuid::Uuid::new_v4().as_bytes(),
        mac: [0; 32],
    };
    ticket.mac = hmac(&key, &payload(&ticket));
    Ok((success, ticket))
}
pub fn verify(ticket: &IdentityTicket, user: &UserContext) -> io::Result<()> {
    let caller = crate::accounts::grants::CallerBinding::capture().map_err(io::Error::other)?;
    let config = trusted_config()?;
    if config.web_accounts.enabled
        || !config.account_policy.allows_login(&user.username)
        || ticket.identity != *user
        || ticket.caller.as_ref() != Some(&caller)
        || ticket.caller_uid != unsafe { libc::getuid() }
        || ticket.expires_at <= InMemorySessionStore::now_epoch_seconds()
        || !verify_mac(&key()?, &payload(ticket), &ticket.mac)
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
/// Resolve the OS principal by its protected numeric identity, never a client alias.
pub fn account_by_uid(uid: u32) -> Result<AuthSuccess, AuthError> {
    let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
    let mut result = std::ptr::null_mut();
    let mut buffer = vec![0u8; 65536];
    let code = unsafe {
        libc::getpwuid_r(
            uid,
            &mut pwd,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        )
    };
    if code != 0 || result.is_null() || pwd.pw_name.is_null() {
        return Err(AuthError::InvalidCredentials);
    }
    let name = unsafe { CStr::from_ptr(pwd.pw_name) }
        .to_str()
        .map_err(|_| AuthError::InvalidCredentials)?;
    let actual = account(name)?;
    if actual.uid != uid {
        return Err(AuthError::InvalidCredentials);
    }
    Ok(actual)
}
pub fn enter_identity(user: &UserContext) -> io::Result<()> {
    if user.uid == 0 {
        return Err(io::Error::from(io::ErrorKind::PermissionDenied));
    }
    let actual = if unsafe { libc::geteuid() } == 0
        && trusted_config().is_ok_and(|config| config.web_accounts.enabled)
    {
        // Logical virtual names were bound to this OS UID by the root gateway.
        account_by_uid(user.uid)
    } else {
        account(&user.username)
    }
    .map_err(io::Error::other)?;
    if actual.uid != user.uid || actual.primary_gid != user.gid {
        return Err(io::Error::from(io::ErrorKind::PermissionDenied));
    }
    unsafe {
        if libc::geteuid() == 0 {
            let name = CString::new(actual.username.as_str()).map_err(io::Error::other)?;
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
            caller: Some(crate::accounts::grants::CallerBinding {
                uid: 1001,
                pid: 42,
                start_time: 100,
                boot_id: "00000000-0000-4000-8000-000000000001".into(),
            }),
            expires_at: 99,
            nonce: [1; 16],
            mac: [0; 32],
        };
        let mac = hmac(b"test", &payload(&t));
        t.identity.uid = 1002;
        assert!(!verify_mac(b"test", &payload(&t), &mac));
        t.identity.uid = 1000;
        t.expires_at = 100;
        assert!(!verify_mac(b"test", &payload(&t), &mac));
        t.expires_at = 99;
        for field in 0..4 {
            let mut changed = t.clone();
            let caller = changed.caller.as_mut().unwrap();
            match field {
                0 => caller.uid += 1,
                1 => caller.pid += 1,
                2 => caller.start_time += 1,
                _ => caller.boot_id = "00000000-0000-4000-8000-000000000002".into(),
            }
            assert!(!verify_mac(b"test", &payload(&changed), &mac));
        }
        t.caller = None;
        assert!(!verify_mac(b"test", &payload(&t), &mac));
    }
}
