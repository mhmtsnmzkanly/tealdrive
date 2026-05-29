use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::AccountPolicyConfig;

pub trait Authenticator {
    fn authenticate(&self, request: &AuthRequest) -> Result<AuthSuccess, AuthError>;
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
    pub ip: Option<String>,
}

impl AuthRequest {
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
            ip: None,
        }
    }
}

impl fmt::Debug for AuthRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthRequest")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .field("ip", &self.ip)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthSuccess {
    pub username: String,
    pub uid: u32,
    pub primary_gid: u32,
    pub home_dir: Option<PathBuf>,
    pub shell: Option<String>,
    pub groups: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthFailure {
    InvalidCredentials,
    AccountLocked,
    AccountExpired,
    DisabledShell,
    RootLoginDisabled,
    SystemAccountDisabled,
    AuthBackendUnavailable,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AuthError {
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("account locked")]
    AccountLocked,
    #[error("account expired")]
    AccountExpired,
    #[error("disabled shell")]
    DisabledShell,
    #[error("root login disabled")]
    RootLoginDisabled,
    #[error("system account disabled")]
    SystemAccountDisabled,
    #[error("auth backend unavailable")]
    AuthBackendUnavailable,
}

impl AuthError {
    pub fn failure_kind(&self) -> AuthFailure {
        match self {
            Self::InvalidCredentials => AuthFailure::InvalidCredentials,
            Self::AccountLocked => AuthFailure::AccountLocked,
            Self::AccountExpired => AuthFailure::AccountExpired,
            Self::DisabledShell => AuthFailure::DisabledShell,
            Self::RootLoginDisabled => AuthFailure::RootLoginDisabled,
            Self::SystemAccountDisabled => AuthFailure::SystemAccountDisabled,
            Self::AuthBackendUnavailable => AuthFailure::AuthBackendUnavailable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountStatus {
    Active,
    Locked,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountPolicyResult {
    Allowed,
    Rejected(AuthFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockUser {
    pub username: String,
    pub password: String,
    pub uid: u32,
    pub primary_gid: u32,
    pub home_dir: Option<PathBuf>,
    pub shell: Option<String>,
    pub groups: Vec<String>,
    pub status: AccountStatus,
}

impl MockUser {
    pub fn regular(username: impl Into<String>, password: impl Into<String>) -> Self {
        let username = username.into();
        Self {
            username: username.clone(),
            password: password.into(),
            uid: 1000,
            primary_gid: 1000,
            home_dir: Some(PathBuf::from(format!("/home/{username}"))),
            shell: Some("/bin/bash".to_owned()),
            groups: vec![username],
            status: AccountStatus::Active,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MockAuthenticator {
    users: HashMap<String, MockUser>,
    policy: AccountPolicyConfig,
}

impl MockAuthenticator {
    pub fn new(policy: AccountPolicyConfig) -> Self {
        Self {
            users: HashMap::new(),
            policy,
        }
    }

    pub fn with_user(mut self, user: MockUser) -> Self {
        self.users.insert(user.username.clone(), user);
        self
    }
}

impl Authenticator for MockAuthenticator {
    fn authenticate(&self, request: &AuthRequest) -> Result<AuthSuccess, AuthError> {
        let user = self
            .users
            .get(&request.username)
            .ok_or(AuthError::InvalidCredentials)?;

        if user.password != request.password {
            return Err(AuthError::InvalidCredentials);
        }

        match evaluate_account_policy(user, &self.policy) {
            AccountPolicyResult::Allowed => Ok(AuthSuccess {
                username: user.username.clone(),
                uid: user.uid,
                primary_gid: user.primary_gid,
                home_dir: user.home_dir.clone(),
                shell: user.shell.clone(),
                groups: user.groups.clone(),
            }),
            AccountPolicyResult::Rejected(failure) => Err(auth_failure_to_error(failure)),
        }
    }
}

pub fn evaluate_account_policy(
    user: &MockUser,
    policy: &AccountPolicyConfig,
) -> AccountPolicyResult {
    if user.status == AccountStatus::Locked {
        return AccountPolicyResult::Rejected(AuthFailure::AccountLocked);
    }
    if user.status == AccountStatus::Expired {
        return AccountPolicyResult::Rejected(AuthFailure::AccountExpired);
    }
    if policy.reject_root && user.uid == 0 {
        return AccountPolicyResult::Rejected(AuthFailure::RootLoginDisabled);
    }
    if policy.reject_system_accounts && user.uid < policy.minimum_regular_uid {
        return AccountPolicyResult::Rejected(AuthFailure::SystemAccountDisabled);
    }
    if policy.reject_disabled_shells && is_disabled_shell(user.shell.as_deref()) {
        return AccountPolicyResult::Rejected(AuthFailure::DisabledShell);
    }

    AccountPolicyResult::Allowed
}

fn is_disabled_shell(shell: Option<&str>) -> bool {
    let disabled: HashSet<&str> = ["/usr/sbin/nologin", "/sbin/nologin", "/bin/false", "false"]
        .into_iter()
        .collect();
    shell.is_some_and(|value| disabled.contains(value))
}

fn auth_failure_to_error(failure: AuthFailure) -> AuthError {
    match failure {
        AuthFailure::InvalidCredentials => AuthError::InvalidCredentials,
        AuthFailure::AccountLocked => AuthError::AccountLocked,
        AuthFailure::AccountExpired => AuthError::AccountExpired,
        AuthFailure::DisabledShell => AuthError::DisabledShell,
        AuthFailure::RootLoginDisabled => AuthError::RootLoginDisabled,
        AuthFailure::SystemAccountDisabled => AuthError::SystemAccountDisabled,
        AuthFailure::AuthBackendUnavailable => AuthError::AuthBackendUnavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auth_with(user: MockUser) -> MockAuthenticator {
        MockAuthenticator::new(AccountPolicyConfig::default()).with_user(user)
    }

    #[test]
    fn mock_authenticator_successful_login() {
        let auth = auth_with(MockUser::regular("alice", "secret"));
        let success = auth
            .authenticate(&AuthRequest::new("alice", "secret"))
            .expect("login should pass");

        assert_eq!(success.username, "alice");
        assert_eq!(success.uid, 1000);
    }

    #[test]
    fn mock_authenticator_invalid_password_failure() {
        let auth = auth_with(MockUser::regular("alice", "secret"));
        let err = auth
            .authenticate(&AuthRequest::new("alice", "wrong"))
            .expect_err("login should fail");

        assert_eq!(err, AuthError::InvalidCredentials);
    }

    #[test]
    fn locked_account_failure() {
        let mut user = MockUser::regular("alice", "secret");
        user.status = AccountStatus::Locked;
        let auth = auth_with(user);

        assert_eq!(
            auth.authenticate(&AuthRequest::new("alice", "secret")),
            Err(AuthError::AccountLocked)
        );
    }

    #[test]
    fn expired_account_failure() {
        let mut user = MockUser::regular("alice", "secret");
        user.status = AccountStatus::Expired;
        let auth = auth_with(user);

        assert_eq!(
            auth.authenticate(&AuthRequest::new("alice", "secret")),
            Err(AuthError::AccountExpired)
        );
    }

    #[test]
    fn disabled_shell_failure() {
        let mut user = MockUser::regular("alice", "secret");
        user.shell = Some("/usr/sbin/nologin".to_owned());
        let auth = auth_with(user);

        assert_eq!(
            auth.authenticate(&AuthRequest::new("alice", "secret")),
            Err(AuthError::DisabledShell)
        );
    }

    #[test]
    fn root_login_rejected_by_policy() {
        let mut user = MockUser::regular("root", "secret");
        user.uid = 0;
        user.primary_gid = 0;
        let auth = auth_with(user);

        assert_eq!(
            auth.authenticate(&AuthRequest::new("root", "secret")),
            Err(AuthError::RootLoginDisabled)
        );
    }

    #[test]
    fn system_account_rejected_by_policy() {
        let mut user = MockUser::regular("daemon", "secret");
        user.uid = 999;
        let auth = auth_with(user);

        assert_eq!(
            auth.authenticate(&AuthRequest::new("daemon", "secret")),
            Err(AuthError::SystemAccountDisabled)
        );
    }
}
