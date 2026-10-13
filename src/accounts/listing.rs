//! Keyset pagination; scope is derived from live protected authority, never from the cursor.
use super::store::{
    authorized, row_account, Account, AccountKind, AccountStatus, AccountStore, Authority, Role,
    StoreError, COLUMNS,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountQuery {
    pub limit: u16,
    pub cursor: Option<String>,
    pub filter: Option<String>,
    pub status: Option<AccountStatus>,
    pub enrolled: Option<bool>,
}
impl Default for AccountQuery {
    fn default() -> Self {
        Self {
            limit: 100,
            cursor: None,
            filter: None,
            status: None,
            enrolled: None,
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountPage {
    pub items: Vec<Account>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}
impl AccountStore {
    pub fn list_page(
        &self,
        authority: &Authority,
        kind: AccountKind,
        query: &AccountQuery,
    ) -> Result<AccountPage, StoreError> {
        if !(1..=200).contains(&query.limit)
            || query
                .cursor
                .as_ref()
                .is_some_and(|v| v.len() > 256 || v.chars().any(char::is_control))
            || query
                .filter
                .as_ref()
                .is_some_and(|v| v.len() > 128 || v.chars().any(char::is_control))
        {
            return Err(StoreError::InvalidInput);
        }
        let actor = authorized(&self.connection, authority)?;
        if actor
            .as_ref()
            .is_some_and(|actor| actor.role != Role::Admin || kind == AccountKind::Linux)
        {
            return Err(StoreError::Forbidden);
        }
        let status = query.status.map(|status| match status {
            AccountStatus::Active => "active",
            AccountStatus::Suspended => "suspended",
            AccountStatus::Revoked => "revoked",
            AccountStatus::Deleted => "deleted",
        });
        let mut statement=self.connection.prepare(&format!("SELECT {COLUMNS} FROM accounts WHERE kind=?1 AND (?2 IS NULL OR owner_admin_id=?2) AND (?3 IS NULL OR username>?3) AND (?4 IS NULL OR instr(lower(username),lower(?4))>0) AND (?5 IS NULL OR status=?5) AND (?6 IS NULL OR (web_password_hash IS NOT NULL)=?6) ORDER BY username LIMIT ?7"))?;
        let rows = statement.query_map(
            params![
                if kind == AccountKind::Linux {
                    "linux"
                } else {
                    "virtual"
                },
                actor.as_ref().map(|actor| actor.id.as_str()),
                query.cursor,
                query.filter,
                status,
                query.enrolled,
                i64::from(query.limit) + 1
            ],
            row_account,
        )?;
        let mut items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        let has_more = items.len() > usize::from(query.limit);
        if has_more {
            items.pop();
        }
        let next_cursor = if has_more {
            items.last().map(|account| account.username.clone())
        } else {
            None
        };
        Ok(AccountPage {
            items,
            next_cursor,
            has_more,
        })
    }
    /// Root CLI/repair helpers need all identities; public gateway uses bounded pages.
    pub fn all_accounts(
        &self,
        authority: &Authority,
        kind: AccountKind,
    ) -> Result<Vec<Account>, StoreError> {
        let mut result = Vec::new();
        let mut query = AccountQuery {
            limit: 200,
            ..Default::default()
        };
        loop {
            let page = self.list_page(authority, kind, &query)?;
            result.extend(page.items);
            if result.len() > 100000 {
                return Err(StoreError::InvalidInput);
            }
            if !page.has_more {
                break;
            }
            query.cursor = page.next_cursor;
        }
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::store::NewAccount;
    fn create(
        store: &mut AccountStore,
        name: &str,
        uid: u32,
        kind: AccountKind,
        owner: Option<String>,
        role: Role,
    ) -> Account {
        store
            .create(
                &Authority::Root,
                NewAccount {
                    username: name.into(),
                    uid,
                    gid: uid,
                    home: format!("/home/{name}").into(),
                    kind,
                    role,
                    owner_admin_id: owner,
                    quota_bytes: if kind == AccountKind::Virtual && role == Role::User {
                        Some(1)
                    } else {
                        None
                    },
                },
            )
            .unwrap()
    }
    #[test]
    fn filtering_precedes_pagination_and_scopes_do_not_follow_cursor() {
        let mut store = AccountStore::memory();
        let owner = create(
            &mut store,
            "owner",
            1000,
            AccountKind::Linux,
            None,
            Role::Admin,
        );
        let stranger = create(
            &mut store,
            "stranger",
            1001,
            AccountKind::Linux,
            None,
            Role::Admin,
        );
        store.set_pool(&Authority::Root, 100).unwrap();
        store
            .set_admin_budget(&Authority::Root, &owner.id, 50)
            .unwrap();
        store
            .set_admin_budget(&Authority::Root, &stranger.id, 50)
            .unwrap();
        create(
            &mut store,
            "alpha",
            200000,
            AccountKind::Virtual,
            Some(owner.id.clone()),
            Role::User,
        );
        create(
            &mut store,
            "beta",
            200001,
            AccountKind::Virtual,
            Some(owner.id.clone()),
            Role::User,
        );
        create(
            &mut store,
            "foreign",
            200002,
            AccountKind::Virtual,
            Some(stranger.id.clone()),
            Role::User,
        );
        let authority = Authority::Account {
            id: owner.id,
            generation: owner.generation,
        };
        let first = store
            .list_page(
                &authority,
                AccountKind::Virtual,
                &AccountQuery {
                    limit: 1,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(first.items[0].username, "alpha");
        assert!(first.has_more);
        let next = store
            .list_page(
                &authority,
                AccountKind::Virtual,
                &AccountQuery {
                    limit: 1,
                    cursor: first.next_cursor,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(next.items[0].username, "beta");
        assert!(!next.has_more);
        let filtered = store
            .list_page(
                &authority,
                AccountKind::Virtual,
                &AccountQuery {
                    limit: 1,
                    filter: Some("BETA".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(filtered.items[0].username, "beta");
        assert!(store
            .list_page(&authority, AccountKind::Linux, &AccountQuery::default())
            .is_err());
        let empty = store
            .list_page(
                &authority,
                AccountKind::Virtual,
                &AccountQuery {
                    filter: Some("foreign".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(empty.items.is_empty());
        assert!(store
            .list_page(
                &authority,
                AccountKind::Virtual,
                &AccountQuery {
                    limit: 201,
                    ..Default::default()
                }
            )
            .is_err());
    }
}
