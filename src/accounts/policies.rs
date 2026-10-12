//! Root-private policy state; file data and UI navigation are never stored here.
use super::store::{
    account, authorized, manage, now, number, Account, AccountStatus, AccountStore, Authority,
    StoreError,
};
use crate::policy::access::{AccessPolicy, Rule, UserRules};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyLayer {
    Root,
    Admin,
    Personal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u8,
    rules: UserRules,
}

fn key(id: &str) -> String {
    format!("account_policy_v1:{id}")
}
fn validate(rules: &UserRules) -> Result<(), StoreError> {
    if rules.root_rules.len() > 512
        || rules.admin_rules.len() > 512
        || rules.personal_rules.len() > 512
    {
        return Err(StoreError::InvalidInput);
    }
    let mut policy = AccessPolicy::default();
    policy.users.insert("subject".into(), rules.clone());
    policy.validate().map_err(|_| StoreError::InvalidInput)
}
fn load(connection: &Connection, id: &str) -> Result<UserRules, StoreError> {
    let raw: Option<String> = connection
        .query_row(
            "SELECT value FROM security_state WHERE key=?1",
            [key(id)],
            |row| row.get(0),
        )
        .optional()?;
    match raw {
        None => Ok(UserRules::default()),
        Some(raw) if raw.len() <= 524288 => {
            let record: Record =
                serde_json::from_str(&raw).map_err(|_| StoreError::InvalidInput)?;
            if record.version != 1 {
                return Err(StoreError::InvalidInput);
            }
            validate(&record.rules)?;
            Ok(record.rules)
        }
        _ => Err(StoreError::InvalidInput),
    }
}
impl AccountStore {
    pub fn policy(&self, actor: &Authority, id: &str) -> Result<UserRules, StoreError> {
        let target = account(&self.connection, id)?;
        if let Some(caller) = authorized(&self.connection, actor)? {
            if caller.id != target.id {
                manage(&self.connection, actor, &target)?;
            }
        }
        load(&self.connection, id)
    }

    /// Layer replacement and generation invalidation share one immediate transaction.
    pub fn set_policy(
        &mut self,
        actor: &Authority,
        id: &str,
        expected_generation: u64,
        layer: PolicyLayer,
        replacement: Vec<Rule>,
    ) -> Result<Account, StoreError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let target = account(&tx, id)?;
        let caller = authorized(&tx, actor)?;
        if let Some(caller) = caller {
            match layer {
                PolicyLayer::Root => return Err(StoreError::Forbidden),
                PolicyLayer::Personal if caller.id == target.id => {}
                PolicyLayer::Personal => return Err(StoreError::Forbidden),
                PolicyLayer::Admin => manage(&tx, actor, &target)?,
            }
        }
        if target.generation != expected_generation || target.status == AccountStatus::Deleted {
            return Err(StoreError::Conflict);
        }
        let mut rules = load(&tx, id)?;
        match layer {
            PolicyLayer::Root => rules.root_rules = replacement,
            PolicyLayer::Admin => rules.admin_rules = replacement,
            PolicyLayer::Personal => rules.personal_rules = replacement,
        }
        validate(&rules)?;
        let raw = serde_json::to_string(&Record { version: 1, rules })
            .map_err(|_| StoreError::InvalidInput)?;
        if raw.len() > 524288 {
            return Err(StoreError::InvalidInput);
        }
        tx.execute("INSERT INTO security_state(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key(id),raw])?;
        tx.execute(
            "UPDATE accounts SET generation=generation+1,updated_at=?1 WHERE id=?2",
            params![number(now())?, id],
        )?;
        tx.execute(
            "DELETE FROM enrollment WHERE username=?1",
            [&target.username],
        )?;
        let updated = account(&tx, id)?;
        tx.commit()?;
        Ok(updated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::store::{AccountKind, NewAccount, Role};
    use crate::{config::RelativePath, policy::access::Access};
    fn create(store: &mut AccountStore, name: &str, uid: u32) -> Account {
        store
            .create(
                &Authority::Root,
                NewAccount {
                    username: name.into(),
                    uid,
                    gid: uid,
                    home: format!("/home/{name}").into(),
                    kind: AccountKind::Linux,
                    role: Role::User,
                    owner_admin_id: None,
                    quota_bytes: None,
                },
            )
            .unwrap()
    }
    fn actor(a: &Account) -> Authority {
        Authority::Account {
            id: a.id.clone(),
            generation: a.generation,
        }
    }
    fn rule() -> Rule {
        Rule {
            path: RelativePath::new("private"),
            root_id: None,
            recursive: true,
            deny: vec![Access::Discover],
        }
    }
    #[test]
    fn personal_scope_root_union_and_atomic_generation() {
        let mut store = AccountStore::memory();
        let alice = create(&mut store, "alice", 1000);
        let bob = create(&mut store, "bob", 1001);
        assert!(matches!(
            store.set_policy(
                &actor(&alice),
                &bob.id,
                1,
                PolicyLayer::Personal,
                vec![rule()]
            ),
            Err(StoreError::Forbidden)
        ));
        assert!(matches!(
            store.set_policy(
                &actor(&alice),
                &alice.id,
                1,
                PolicyLayer::Root,
                vec![rule()]
            ),
            Err(StoreError::Forbidden)
        ));
        let alice = store
            .set_policy(
                &Authority::Root,
                &alice.id,
                1,
                PolicyLayer::Root,
                vec![rule()],
            )
            .unwrap();
        let updated = store
            .set_policy(
                &actor(&alice),
                &alice.id,
                alice.generation,
                PolicyLayer::Personal,
                vec![],
            )
            .unwrap();
        assert_eq!(updated.generation, alice.generation + 1);
        assert!(store.policy(&actor(&alice), &alice.id).is_err());
        assert_eq!(
            store
                .policy(&actor(&updated), &alice.id)
                .unwrap()
                .root_rules
                .len(),
            1
        );
        assert!(store.policy(&actor(&updated), &bob.id).is_err());
        assert!(matches!(
            store.set_policy(&Authority::Root, &alice.id, 1, PolicyLayer::Admin, vec![]),
            Err(StoreError::Conflict)
        ));
    }
    #[test]
    fn malformed_state_fails_closed_and_invalid_rule_does_not_commit() {
        let mut store = AccountStore::memory();
        let alice = create(&mut store, "alice", 1000);
        let mut invalid = rule();
        invalid.path = RelativePath::new("../escape");
        assert!(store
            .set_policy(
                &actor(&alice),
                &alice.id,
                1,
                PolicyLayer::Personal,
                vec![invalid]
            )
            .is_err());
        assert_eq!(store.get(&alice.id).unwrap().generation, 1);
        store
            .connection
            .execute(
                "INSERT INTO security_state VALUES(?1,'{broken')",
                [key(&alice.id)],
            )
            .unwrap();
        assert!(store.policy(&actor(&alice), &alice.id).is_err());
        assert!(store
            .set_policy(&Authority::Root, &alice.id, 1, PolicyLayer::Root, vec![])
            .is_err());
        assert_eq!(store.get(&alice.id).unwrap().generation, 1);
    }

    #[test]
    fn tenant_admin_only_manages_its_children_and_cannot_replace_personal_layer() {
        let mut store = AccountStore::memory();
        let mut parent = |name: &str, uid: u32| {
            store
                .create(
                    &Authority::Root,
                    NewAccount {
                        username: name.into(),
                        uid,
                        gid: uid,
                        home: format!("/home/{name}").into(),
                        kind: AccountKind::Virtual,
                        role: Role::Admin,
                        owner_admin_id: None,
                        quota_bytes: None,
                    },
                )
                .unwrap()
        };
        let first = parent("tenant-a", 200000);
        let second = parent("tenant-b", 200001);
        store.set_pool(&Authority::Root, 1000).unwrap();
        store
            .set_admin_budget(&Authority::Root, &first.id, 500)
            .unwrap();
        store
            .set_admin_budget(&Authority::Root, &second.id, 500)
            .unwrap();
        let child = store
            .create(
                &actor(&first),
                NewAccount {
                    username: "child".into(),
                    uid: 200002,
                    gid: 200002,
                    home: "/home/child".into(),
                    kind: AccountKind::Virtual,
                    role: Role::User,
                    owner_admin_id: Some(first.id.clone()),
                    quota_bytes: Some(100),
                },
            )
            .unwrap();
        assert!(matches!(
            store.set_policy(
                &actor(&second),
                &child.id,
                1,
                PolicyLayer::Admin,
                vec![rule()]
            ),
            Err(StoreError::Forbidden)
        ));
        assert!(store.policy(&actor(&second), &child.id).is_err());
        assert!(matches!(
            store.set_policy(&actor(&first), &child.id, 1, PolicyLayer::Personal, vec![]),
            Err(StoreError::Forbidden)
        ));
        let updated = store
            .set_policy(
                &actor(&first),
                &child.id,
                1,
                PolicyLayer::Admin,
                vec![rule()],
            )
            .unwrap();
        let cleared = store
            .set_policy(
                &actor(&updated),
                &child.id,
                updated.generation,
                PolicyLayer::Personal,
                vec![],
            )
            .unwrap();
        assert_eq!(
            store
                .policy(&actor(&cleared), &child.id)
                .unwrap()
                .admin_rules
                .len(),
            1
        );
    }
}
