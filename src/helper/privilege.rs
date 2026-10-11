//! Privilege transitions happen once in the isolated helper process.
use serde::{Deserialize, Serialize};
use thiserror::Error;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivilegeDropMode { Disabled, Setresuid, FutureCapabilityMode }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivilegeTarget { pub uid: u32, pub gid: u32 }
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PrivilegeDropError {
    #[error("unsupported privilege mode")] NotImplemented,
    #[error("refusing uid 0")] RefuseRoot,
    #[error("identity transition failed")] IdentityDenied,
}
pub fn apply_privilege_mode(mode: PrivilegeDropMode, target: PrivilegeTarget) -> Result<(), PrivilegeDropError> {
    if target.uid==0 { return Err(PrivilegeDropError::RefuseRoot); }
    if mode==PrivilegeDropMode::FutureCapabilityMode { return Err(PrivilegeDropError::NotImplemented); }
    if mode==PrivilegeDropMode::Disabled {
        return if target.uid==unsafe { libc::geteuid() } && target.gid==unsafe { libc::getegid() } { Ok(()) } else { Err(PrivilegeDropError::IdentityDenied) };
    }
    let mut pwd: libc::passwd=unsafe { std::mem::zeroed() };
    let mut result=std::ptr::null_mut(); let mut bytes=vec![0u8;65536];
    if unsafe { libc::getpwuid_r(target.uid,&mut pwd,bytes.as_mut_ptr().cast(),bytes.len(),&mut result) }!=0 || result.is_null() {
        return Err(PrivilegeDropError::IdentityDenied);
    }
    let username=unsafe { std::ffi::CStr::from_ptr(pwd.pw_name) }.to_string_lossy().into_owned();
    crate::helper::security::enter_identity(&crate::config::UserContext { username,uid:target.uid,gid:target.gid }).map_err(|_|PrivilegeDropError::IdentityDenied)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelperPrivilegeMode { DevelopmentNoPrivilegeDrop, SetuidRoot, FileCapabilities, SystemdMediated }
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn refuses_root_in_every_mode() {
        for mode in [PrivilegeDropMode::Disabled,PrivilegeDropMode::Setresuid] {
            assert_eq!(apply_privilege_mode(mode,PrivilegeTarget{uid:0,gid:0}),Err(PrivilegeDropError::RefuseRoot));
        }
    }
    #[test] fn disabled_cannot_impersonate() {
        assert_eq!(apply_privilege_mode(PrivilegeDropMode::Disabled,PrivilegeTarget{uid:u32::MAX-1,gid:1000}),Err(PrivilegeDropError::IdentityDenied));
    }
}
