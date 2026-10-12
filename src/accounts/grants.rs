//! Root-only persistent session and enrollment proofs, separate from browser cookies.
use super::store::{
    account, number, Account, AccountKind, AccountStatus, AccountStore, StoreError,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallerBinding {
    pub uid: u32,
    pub pid: u32,
    pub start_time: u64,
    pub boot_id: String,
}
impl CallerBinding {
    pub fn validate(&self) -> Result<(), StoreError> {
        if self.pid == 0 || self.start_time == 0 || uuid::Uuid::parse_str(&self.boot_id).is_err() {
            return Err(StoreError::InvalidInput);
        }
        number(self.start_time)?;
        Ok(())
    }
    /// Parent of this per-operation helper; UID comes from the kernel, not request JSON.
    pub fn capture() -> Result<Self, StoreError> {
        let uid = unsafe { libc::getuid() };
        let pid = unsafe { libc::getppid() } as u32;
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
        if stat.len() > 8192 {
            return Err(StoreError::InvalidInput);
        }
        let status = std::fs::read_to_string(format!("/proc/{pid}/status"))?;
        if status.len() > 65536 {
            return Err(StoreError::InvalidInput);
        }
        let parent_uid: u32 = status
            .lines()
            .find_map(|line| line.strip_prefix("Uid:"))
            .and_then(|line| line.split_whitespace().next())
            .ok_or(StoreError::InvalidInput)?
            .parse()
            .map_err(|_| StoreError::InvalidInput)?;
        if parent_uid != uid {
            return Err(StoreError::Forbidden);
        }
        let fields = stat.rsplit_once(") ").ok_or(StoreError::InvalidInput)?.1;
        let start_time = fields
            .split_whitespace()
            .nth(19)
            .ok_or(StoreError::InvalidInput)?
            .parse()
            .map_err(|_| StoreError::InvalidInput)?;
        let boot_id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?
            .trim()
            .to_owned();
        let caller = Self {
            uid,
            pid,
            start_time,
            boot_id,
        };
        caller.validate()?;
        Ok(caller)
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionGrant {
    pub nonce: String,
    pub account_id: String,
    pub generation: u64,
    pub caller: CallerBinding,
    pub expires_at: u64,
}
impl std::fmt::Debug for SessionGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionGrant([REDACTED])")
    }
}
#[derive(Clone, Serialize, Deserialize, zeroize::ZeroizeOnDrop)]
pub struct EnrollmentProof {
    pub id: String,
    #[zeroize(skip)]
    pub expires_at: u64,
}
impl std::fmt::Debug for EnrollmentProof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EnrollmentProof([REDACTED])")
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrollmentIdentity {
    pub username: String,
    pub uid: u32,
    pub gid: u32,
    pub home: PathBuf,
}
impl EnrollmentIdentity {
    fn validate(&self) -> Result<(), StoreError> {
        if self.uid == 0
            || self.gid == 0
            || self.username.is_empty()
            || self.username.len() > 256
            || self.username.contains(['/', '\\', '\0'])
            || !self.home.is_absolute()
            || self.home == std::path::Path::new("/")
            || self
                .home
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(StoreError::InvalidInput);
        }
        Ok(())
    }
}
fn token_hash(token: &str) -> Result<String, StoreError> {
    let id = uuid::Uuid::parse_str(token).map_err(|_| StoreError::InvalidCredentials)?;
    if id.to_string() != token {
        return Err(StoreError::InvalidCredentials);
    }
    Ok(format!("{:x}", Sha256::digest(token.as_bytes())))
}
fn epoch(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::InvalidInput)
}
impl AccountStore {
    pub fn issue_grant(
        &mut self,
        id: &str,
        generation: u64,
        caller: CallerBinding,
        at: u64,
        lifetime: u64,
    ) -> Result<SessionGrant, StoreError> {
        caller.validate()?;
        if lifetime == 0 || lifetime > 43200 {
            return Err(StoreError::InvalidInput);
        }
        let expiry = at.checked_add(lifetime).ok_or(StoreError::InvalidInput)?;
        number(expiry)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let target = account(&tx, id)?;
        if target.generation != generation
            || target.status != AccountStatus::Active
            || !target.login_enabled
        {
            return Err(StoreError::Forbidden);
        }
        tx.execute("DELETE FROM session_grants WHERE expires_at<=?1 OR revoked=1 OR account_generation<>(SELECT generation FROM accounts WHERE id=account_id)",[number(at)?])?;
        let total: i64 = tx.query_row("SELECT count(*) FROM session_grants", [], |r| r.get(0))?;
        let own: i64 = tx.query_row(
            "SELECT count(*) FROM session_grants WHERE account_id=?1",
            [id],
            |r| r.get(0),
        )?;
        if total >= 8192 || own >= 64 {
            return Err(StoreError::Conflict);
        }
        let nonce = uuid::Uuid::new_v4().to_string();
        tx.execute("INSERT INTO session_grants(nonce,account_id,account_generation,caller_uid,caller_pid,caller_start,caller_boot,expires_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![nonce,id,number(generation)?,caller.uid,caller.pid,number(caller.start_time)?,caller.boot_id,number(expiry)?])?;
        tx.commit()?;
        Ok(SessionGrant {
            nonce,
            account_id: id.to_owned(),
            generation,
            caller,
            expires_at: expiry,
        })
    }
    pub fn verify_grant(
        &self,
        grant: &SessionGrant,
        caller: &CallerBinding,
        at: u64,
    ) -> Result<Account, StoreError> {
        if &grant.caller != caller || grant.expires_at <= at {
            return Err(StoreError::Forbidden);
        }
        let valid:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM session_grants WHERE nonce=?1 AND account_id=?2 AND account_generation=?3 AND caller_uid=?4 AND caller_pid=?5 AND caller_start=?6 AND caller_boot=?7 AND expires_at=?8 AND expires_at>?9 AND revoked=0)",params![grant.nonce,grant.account_id,number(grant.generation)?,caller.uid,caller.pid,number(caller.start_time)?,caller.boot_id,number(grant.expires_at)?,number(at)?],|r|r.get(0))?;
        if !valid {
            return Err(StoreError::Forbidden);
        }
        let target = self.get(&grant.account_id)?;
        self.verify_generation(&target.id, grant.generation, target.uid, target.gid)
    }
    pub fn revoke_grant(
        &mut self,
        grant: &SessionGrant,
        caller: &CallerBinding,
        at: u64,
    ) -> Result<(), StoreError> {
        self.verify_grant(grant, caller, at)?;
        self.connection.execute(
            "UPDATE session_grants SET revoked=1 WHERE nonce=?1",
            [&grant.nonce],
        )?;
        Ok(())
    }
    pub fn begin_enrollment(
        &mut self,
        identity: &EnrollmentIdentity,
        caller: CallerBinding,
        at: u64,
        lifetime: u64,
    ) -> Result<EnrollmentProof, StoreError> {
        identity.validate()?;
        caller.validate()?;
        if lifetime == 0 || lifetime > 300 {
            return Err(StoreError::InvalidInput);
        }
        let expiry = at.checked_add(lifetime).ok_or(StoreError::InvalidInput)?;
        number(expiry)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<String> = tx
            .query_row(
                "SELECT id FROM accounts WHERE username=?1",
                [&identity.username],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            let target = account(&tx, &id)?;
            let self_revoked = target.status == AccountStatus::Revoked && !target.deny_enrollment;
            if target.kind != AccountKind::Linux
                || target.uid != identity.uid
                || target.gid != identity.gid
                || target.home != identity.home
                || target.deny_enrollment
                || matches!(
                    target.status,
                    AccountStatus::Suspended | AccountStatus::Deleted
                )
                || (target.password_enrolled && !self_revoked)
            {
                return Err(StoreError::Forbidden);
            }
        }
        tx.execute("DELETE FROM enrollment WHERE expires_at<=?1 OR consumed=1 OR (username=?2 AND caller_uid=?3 AND caller_pid=?4)",params![number(at)?,identity.username,caller.uid,caller.pid])?;
        let total: i64 = tx.query_row("SELECT count(*) FROM enrollment", [], |r| r.get(0))?;
        if total >= 1024 {
            return Err(StoreError::Conflict);
        }
        let token = uuid::Uuid::new_v4().to_string();
        let hash = token_hash(&token)?;
        tx.execute("INSERT INTO enrollment(token_hash,username,uid,gid,home,caller_uid,caller_pid,caller_start,caller_boot,expires_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![hash,identity.username,identity.uid,identity.gid,identity.home.to_str().ok_or(StoreError::InvalidInput)?,caller.uid,caller.pid,number(caller.start_time)?,caller.boot_id,number(expiry)?])?;
        tx.commit()?;
        Ok(EnrollmentProof {
            id: token,
            expires_at: expiry,
        })
    }
    pub fn enrollment_identity(
        &self,
        token: &str,
        caller: &CallerBinding,
        at: u64,
    ) -> Result<EnrollmentIdentity, StoreError> {
        let hash = token_hash(token)?;
        let identity=self.connection.query_row("SELECT username,uid,gid,home FROM enrollment WHERE token_hash=?1 AND caller_uid=?2 AND caller_pid=?3 AND caller_start=?4 AND caller_boot=?5 AND expires_at>?6 AND consumed=0",params![hash,caller.uid,caller.pid,number(caller.start_time)?,caller.boot_id,number(at)?],|row|Ok(EnrollmentIdentity{username:row.get(0)?,uid:row.get(1)?,gid:row.get(2)?,home:PathBuf::from(row.get::<_,String>(3)?)})).optional()?.ok_or(StoreError::InvalidCredentials)?;
        identity.validate()?;
        Ok(identity)
    }
    /// Gateway must re-resolve NSS identity and root eligibility before calling this.
    pub fn complete_enrollment(
        &mut self,
        token: &str,
        caller: &CallerBinding,
        current: &EnrollmentIdentity,
        password: &str,
        at: u64,
    ) -> Result<Account, StoreError> {
        let snapshot = self.enrollment_identity(token, caller, at)?;
        if &snapshot != current {
            return Err(StoreError::Forbidden);
        }
        let hash = super::password::hash(password)?;
        let token_hash = token_hash(token)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed=tx.execute("UPDATE enrollment SET consumed=1 WHERE token_hash=?1 AND caller_uid=?2 AND caller_pid=?3 AND caller_start=?4 AND caller_boot=?5 AND expires_at>?6 AND consumed=0",params![token_hash,caller.uid,caller.pid,number(caller.start_time)?,caller.boot_id,number(at)?])?;
        if changed != 1 {
            return Err(StoreError::InvalidCredentials);
        }
        let existing: Option<String> = tx
            .query_row(
                "SELECT id FROM accounts WHERE username=?1",
                [&current.username],
                |r| r.get(0),
            )
            .optional()?;
        let id = if let Some(id) = existing {
            let target = account(&tx, &id)?;
            if target.kind != AccountKind::Linux
                || target.uid != current.uid
                || target.gid != current.gid
                || target.home != current.home
                || target.deny_enrollment
                || matches!(
                    target.status,
                    AccountStatus::Suspended | AccountStatus::Deleted
                )
                || (target.password_enrolled && target.status != AccountStatus::Revoked)
            {
                return Err(StoreError::Forbidden);
            }
            id
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            tx.execute("INSERT INTO accounts(id,username,uid,gid,home,kind,role,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,'linux','user',?6,?6)",params![id,current.username,current.uid,current.gid,current.home.to_str().ok_or(StoreError::InvalidInput)?,number(at)?])?;
            id
        };
        tx.execute("UPDATE accounts SET web_password_hash=?1,status='active',login_enabled=1,deny_enrollment=0,generation=generation+1,updated_at=?2 WHERE id=?3",params![hash.as_str(),number(at)?,id])?;
        let enrolled = account(&tx, &id)?;
        tx.commit()?;
        Ok(enrolled)
    }
    pub fn grant_count(&self, id: &str) -> Result<u64, StoreError> {
        let count: i64 = self.connection.query_row(
            "SELECT count(*) FROM session_grants WHERE account_id=?1 AND revoked=0",
            [id],
            |r| r.get(0),
        )?;
        epoch(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::store::{Authority, NewAccount, Role};
    fn caller() -> CallerBinding {
        CallerBinding {
            uid: 995,
            pid: 123,
            start_time: 456,
            boot_id: uuid::Uuid::new_v4().to_string(),
        }
    }
    fn identity() -> EnrollmentIdentity {
        EnrollmentIdentity {
            username: "alice".into(),
            uid: 1000,
            gid: 1000,
            home: "/home/alice".into(),
        }
    }
    fn user(store: &mut AccountStore) -> Account {
        store
            .create(
                &Authority::Root,
                NewAccount {
                    username: "alice".into(),
                    uid: 1000,
                    gid: 1000,
                    home: "/home/alice".into(),
                    kind: AccountKind::Linux,
                    role: Role::User,
                    owner_admin_id: None,
                    quota_bytes: None,
                },
            )
            .unwrap()
    }
    #[test]
    fn logout_only_revokes_one_nonce_and_suspend_invalidates_all() {
        let mut store = AccountStore::memory();
        let account = user(&mut store);
        let caller = caller();
        let one = store
            .issue_grant(&account.id, account.generation, caller.clone(), 1000, 300)
            .unwrap();
        let two = store
            .issue_grant(&account.id, account.generation, caller.clone(), 1000, 300)
            .unwrap();
        assert_eq!(store.grant_count(&account.id).unwrap(), 2);
        store.revoke_grant(&one, &caller, 1001).unwrap();
        assert!(store.verify_grant(&one, &caller, 1001).is_err());
        assert!(store.verify_grant(&two, &caller, 1001).is_ok());
        store
            .transition(
                &Authority::Root,
                &account.id,
                AccountStatus::Suspended,
                false,
            )
            .unwrap();
        assert!(store.verify_grant(&two, &caller, 1001).is_err());
    }
    #[test]
    fn grant_binds_uid_pid_start_boot_and_expiry() {
        let mut store = AccountStore::memory();
        let account = user(&mut store);
        let caller = caller();
        let grant = store
            .issue_grant(&account.id, account.generation, caller.clone(), 1000, 300)
            .unwrap();
        for different in [
            CallerBinding {
                uid: 1000,
                ..caller.clone()
            },
            CallerBinding {
                pid: 124,
                ..caller.clone()
            },
            CallerBinding {
                start_time: 457,
                ..caller.clone()
            },
            CallerBinding {
                boot_id: uuid::Uuid::new_v4().to_string(),
                ..caller.clone()
            },
        ] {
            assert!(store.verify_grant(&grant, &different, 1000).is_err());
        }
        assert!(store.verify_grant(&grant, &caller, 1300).is_err());
        let mut tampered = grant.clone();
        tampered.expires_at += 1;
        assert!(store.verify_grant(&tampered, &caller, 1000).is_err());
        assert!(store
            .issue_grant(&account.id, account.generation, caller.clone(), 1000, 43201)
            .is_err());
        store
            .set_password(&Authority::Root, &account.id, "separate-web-password")
            .unwrap();
        assert!(store.verify_grant(&grant, &caller, 1000).is_err());
        let current = store.get(&account.id).unwrap();
        assert!(store
            .issue_grant(&account.id, current.generation, caller, 1000, 300)
            .is_ok());
    }
    #[test]
    fn enrollment_is_hashed_single_use_caller_bound_and_cannot_reset_existing_password() {
        let mut store = AccountStore::memory();
        let caller = caller();
        let identity = identity();
        let proof = store
            .begin_enrollment(&identity, caller.clone(), 1000, 300)
            .unwrap();
        let stored: String = store
            .connection
            .query_row("SELECT token_hash FROM enrollment", [], |r| r.get(0))
            .unwrap();
        assert_ne!(stored, proof.id);
        assert_eq!(stored.len(), 64);
        assert!(!format!("{proof:?}").contains(&proof.id));
        let different = CallerBinding {
            pid: 124,
            ..caller.clone()
        };
        assert!(store
            .enrollment_identity(&proof.id, &different, 1000)
            .is_err());
        assert!(store.enrollment_identity(&proof.id, &caller, 1300).is_err());
        let changed = EnrollmentIdentity {
            uid: 1001,
            ..identity.clone()
        };
        assert!(store
            .complete_enrollment(&proof.id, &caller, &changed, "separate-web-password", 1001)
            .is_err());
        let enrolled = store
            .complete_enrollment(&proof.id, &caller, &identity, "separate-web-password", 1001)
            .unwrap();
        assert!(enrolled.password_enrolled);
        assert!(store
            .verify_password("alice", "separate-web-password")
            .is_ok());
        assert!(store
            .complete_enrollment(&proof.id, &caller, &identity, "replayed-web-password", 1001)
            .is_err());
        assert!(store
            .begin_enrollment(&identity, caller.clone(), 1001, 300)
            .is_err());
        let own = Authority::Account {
            id: enrolled.id.clone(),
            generation: enrolled.generation,
        };
        let revoked = store
            .transition(&own, &enrolled.id, AccountStatus::Revoked, false)
            .unwrap();
        assert!(!revoked.deny_enrollment);
        let recovery = store
            .begin_enrollment(&identity, caller.clone(), 1002, 300)
            .unwrap();
        store
            .complete_enrollment(
                &recovery.id,
                &caller,
                &identity,
                "recovered-web-password",
                1003,
            )
            .unwrap();
        let latest = store.get(&enrolled.id).unwrap();
        store
            .transition(&Authority::Root, &latest.id, AccountStatus::Revoked, false)
            .unwrap();
        assert!(store
            .begin_enrollment(&identity, caller, 1004, 300)
            .is_err());
    }
    #[test]
    fn admin_deny_between_enrollment_steps_blocks_completion() {
        let mut store = AccountStore::memory();
        let account = user(&mut store);
        let caller = caller();
        let identity = identity();
        let proof = store
            .begin_enrollment(&identity, caller.clone(), 1000, 300)
            .unwrap();
        store
            .transition(
                &Authority::Root,
                &account.id,
                AccountStatus::Suspended,
                false,
            )
            .unwrap();
        assert!(store
            .complete_enrollment(&proof.id, &caller, &identity, "separate-web-password", 1001)
            .is_err());
        assert!(!store.get(&account.id).unwrap().login_enabled);
    }
    #[test]
    fn session_grants_are_bounded_and_expired_records_are_reclaimed() {
        let mut store = AccountStore::memory();
        let account = user(&mut store);
        let caller = caller();
        for _ in 0..64 {
            store
                .issue_grant(&account.id, account.generation, caller.clone(), 1000, 300)
                .unwrap();
        }
        assert!(store
            .issue_grant(&account.id, account.generation, caller.clone(), 1000, 300)
            .is_err());
        store
            .issue_grant(&account.id, account.generation, caller, 1300, 300)
            .unwrap();
        assert_eq!(store.grant_count(&account.id).unwrap(), 1);
    }
    #[test]
    fn capture_uses_real_kernel_caller() {
        let caller = CallerBinding::capture().unwrap();
        assert_eq!(caller.uid, unsafe { libc::getuid() });
        assert_eq!(caller.pid, unsafe { libc::getppid() } as u32);
        assert!(caller.start_time > 0);
    }
}
