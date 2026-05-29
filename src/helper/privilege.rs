use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivilegeDropMode {
    Disabled,
    Setresuid,
    FutureCapabilityMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivilegeTarget {
    pub uid: u32,
    pub gid: u32,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PrivilegeDropError {
    #[error("privilege drop not implemented")]
    NotImplemented,
    #[error("refusing uid 0")]
    RefuseRoot,
}

pub fn apply_privilege_mode(
    mode: PrivilegeDropMode,
    target: PrivilegeTarget,
) -> Result<(), PrivilegeDropError> {
    if target.uid == 0 {
        return Err(PrivilegeDropError::RefuseRoot);
    }
    match mode {
        PrivilegeDropMode::Disabled => Ok(()),
        PrivilegeDropMode::Setresuid | PrivilegeDropMode::FutureCapabilityMode => {
            Err(PrivilegeDropError::NotImplemented)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelperPrivilegeMode {
    DevelopmentNoPrivilegeDrop,
    SetuidRoot,
    FileCapabilities,
    SystemdMediated,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_mode_returns_ok() {
        assert_eq!(
            apply_privilege_mode(
                PrivilegeDropMode::Disabled,
                PrivilegeTarget {
                    uid: 1000,
                    gid: 1000
                }
            ),
            Ok(())
        );
    }

    #[test]
    fn setresuid_mode_returns_not_implemented_safely() {
        assert_eq!(
            apply_privilege_mode(
                PrivilegeDropMode::Setresuid,
                PrivilegeTarget {
                    uid: 1000,
                    gid: 1000
                }
            ),
            Err(PrivilegeDropError::NotImplemented)
        );
    }
}
