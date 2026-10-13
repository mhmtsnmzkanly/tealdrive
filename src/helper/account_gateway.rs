//! Root control plane. This gateway never opens user content or trusts client UID/roles.
use crate::accounts::grants::{CallerBinding, EnrollmentIdentity, EnrollmentProof, SessionGrant};
use crate::accounts::store::{Account, AccountKind, AccountStore, StoreError};
use crate::auth::{AuthRequest, AuthSuccess};
use crate::config::AppConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WebAccountsConfig {
    pub enabled: bool,
    pub server_uid: Option<u32>,
    pub self_registration: bool,
    pub denied_users: Vec<String>,
    pub virtual_min_uid: u32,
    pub virtual_max_uid: u32,
    pub allowed_nologin_users: Vec<String>,
}
impl Default for WebAccountsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            server_uid: None,
            self_registration: true,
            denied_users: Vec::new(),
            virtual_min_uid: 200000,
            virtual_max_uid: 299999,
            allowed_nologin_users: Vec::new(),
        }
    }
}
impl WebAccountsConfig {
    pub fn validate(&self) -> Result<(), crate::errors::TealDriveError> {
        if self.enabled && self.server_uid.is_none_or(|uid| uid == 0)
            || self.virtual_min_uid < 1000
            || self.virtual_min_uid > self.virtual_max_uid
            || self.denied_users.len() > 1024
            || self.allowed_nologin_users.len() > 1024
            || self.allowed_nologin_users.iter().any(|name| {
                name.is_empty()
                    || name.len() > 256
                    || name.chars().any(|c| {
                        c.is_control() || c.is_whitespace() || matches!(c, '/' | '\\' | ':')
                    })
            })
            || self.denied_users.iter().any(|s| {
                s.is_empty()
                    || s.len() > 256
                    || s.chars().any(|c| {
                        c.is_control() || c.is_whitespace() || matches!(c, '/' | '\\' | ':')
                    })
            })
        {
            return Err(crate::errors::TealDriveError::Validation);
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
pub enum AccountCommand {
    ApplyUndo {
        grant: SessionGrant,
        operation_id: String,
        expected_state: String,
    },
    UndoPreferences {
        grant: SessionGrant,
    },
    SetUndoPreferences {
        grant: SessionGrant,
        hours: Option<u64>,
        expected_revision: u64,
    },
    History {
        grant: SessionGrant,
        query: crate::accounts::journal::HistoryQuery,
        #[serde(default)]
        undo_only: bool,
    },
    Login(AuthRequest),
    BeginEnrollment(AuthRequest),
    CompleteEnrollment {
        proof: EnrollmentProof,
        password: zeroize::Zeroizing<String>,
    },
    Validate(SessionGrant),
    Logout(SessionGrant),
    RevokeSelf(SessionGrant),
    ListAccounts {
        grant: SessionGrant,
        kind: AccountKind,
    },
    SetAccountAccess {
        grant: SessionGrant,
        account_id: String,
        kind: AccountKind,
        enabled: bool,
    },
    ChangePassword {
        grant: SessionGrant,
        current_password: zeroize::Zeroizing<String>,
        new_password: zeroize::Zeroizing<String>,
    },
    ApproveLinux {
        grant: SessionGrant,
        username: String,
    },
    GetPolicy {
        grant: SessionGrant,
        account_id: Option<String>,
        kind: Option<AccountKind>,
    },
    SetPolicy {
        grant: SessionGrant,
        account_id: Option<String>,
        kind: Option<AccountKind>,
        expected_generation: u64,
        layer: crate::accounts::policies::PolicyLayer,
        rules: Vec<crate::policy::access::Rule>,
    },
    CreateVirtual {
        grant: SessionGrant,
        name: String,
        quota_bytes: u64,
        password: zeroize::Zeroizing<String>,
        idempotency_key: String,
    },
    SetVirtualQuota {
        grant: SessionGrant,
        account_id: String,
        expected_generation: u64,
        quota_bytes: u64,
    },
    DeleteVirtual {
        grant: SessionGrant,
        account_id: String,
        expected_generation: u64,
    },
    Budget(SessionGrant),
    CandidateRegistration {
        grant: SessionGrant,
        names: Vec<String>,
    },
    ListAccountPage {
        grant: SessionGrant,
        kind: AccountKind,
        query: crate::accounts::listing::AccountQuery,
    },
    LocalEnroll {
        linux_password: zeroize::Zeroizing<String>,
        web_password: zeroize::Zeroizing<String>,
    },
    LocalStatus,
    LocalRevoke,
    LocalPolicy,
    LocalSetPolicy {
        expected_generation: u64,
        rules: Vec<crate::policy::access::Rule>,
    },
}
#[derive(Debug, Serialize, Deserialize)]
pub enum AccountReply {
    UndoPage {
        items: Vec<crate::accounts::undo::UndoCandidate>,
        next_cursor: Option<String>,
        has_more: bool,
    },
    UndoApplied(Box<crate::helper::types::HelperResponse>),
    UndoPreferences(crate::accounts::undo::UndoPreferences),
    History(crate::accounts::journal::HistoryPage),
    Authenticated {
        account: Account,
        grant: SessionGrant,
    },
    Enrollment(EnrollmentProof),
    Validated(Account),
    LoggedOut,
    AccessRevoked,
    PasswordChanged,
    Accounts(Vec<Account>),
    RegisteredNames(Vec<String>),
    AccountPage(crate::accounts::listing::AccountPage),
    AccountUpdated(Account),
    Policy {
        account: Account,
        rules: crate::policy::access::UserRules,
        deny_system_components: Vec<String>,
    },
    Budget(serde_json::Value),
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum GatewayFailure {
    RateLimited,
    Disabled,
    Forbidden,
    InvalidCredentials,
    Conflict,
    Unavailable,
    QuotaExceeded,
}
impl From<StoreError> for GatewayFailure {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::RateLimited => Self::RateLimited,
            StoreError::InvalidCredentials | StoreError::NotFound => Self::InvalidCredentials,
            StoreError::Forbidden | StoreError::InvalidInput => Self::Forbidden,
            StoreError::Conflict => Self::Conflict,
            StoreError::QuotaExceeded => Self::QuotaExceeded,
            _ => Self::Unavailable,
        }
    }
}
fn authorize_caller(config: &AppConfig, caller: &CallerBinding) -> Result<(), GatewayFailure> {
    if !config.web_accounts.enabled {
        return Err(GatewayFailure::Disabled);
    }
    if config.web_accounts.server_uid != Some(caller.uid) {
        return Err(GatewayFailure::Forbidden);
    }
    Ok(())
}
fn snapshot(identity: AuthSuccess) -> Result<EnrollmentIdentity, GatewayFailure> {
    Ok(EnrollmentIdentity {
        username: identity.username,
        uid: identity.uid,
        gid: identity.primary_gid,
        home: identity.home_dir.ok_or(GatewayFailure::Forbidden)?,
    })
}
pub(crate) fn linux_policy(
    config: &AppConfig,
    identity: &AuthSuccess,
    approved: bool,
) -> Result<crate::config::AccountPolicyConfig, GatewayFailure> {
    let web = &config.web_accounts;
    if crate::auth::account::is_reserved_service_username(&identity.username)
        || identity.uid == 0
        || identity.primary_gid == 0
        || web.server_uid == Some(identity.uid)
        || web.denied_users.contains(&identity.username)
        || identity.uid >= web.virtual_min_uid && identity.uid <= web.virtual_max_uid
        || !approved
            && !web.self_registration
            && !config.account_policy.allows_login(&identity.username)
    {
        return Err(GatewayFailure::Forbidden);
    }
    let mut policy = config.account_policy.clone();
    // Registration extends only this authenticated subject; service/root checks stay mandatory.
    policy.reject_root = true;
    policy.reject_system_accounts = true;
    policy.reject_disabled_shells = !web.allowed_nologin_users.contains(&identity.username);
    if !policy.allowed_users.contains(&identity.username) {
        policy.allowed_users.push(identity.username.clone());
    }
    let user = crate::auth::MockUser {
        username: identity.username.clone(),
        password: String::new(),
        uid: identity.uid,
        primary_gid: identity.primary_gid,
        home_dir: identity.home_dir.clone(),
        shell: identity.shell.clone(),
        groups: identity.groups.clone(),
        status: crate::auth::AccountStatus::Active,
    };
    if crate::auth::account::evaluate_account_policy(&user, &policy)
        != crate::auth::AccountPolicyResult::Allowed
    {
        return Err(GatewayFailure::Forbidden);
    }
    Ok(policy)
}
fn matches_nss(account: &Account, identity: &AuthSuccess) -> bool {
    account.uid == identity.uid
        && account.gid == identity.primary_gid
        && Some(&account.home) == identity.home_dir.as_ref()
        && (account.kind == AccountKind::Virtual || account.username == identity.username)
}
pub(crate) fn validate_account(
    config: &AppConfig,
    account: &Account,
) -> Result<(), GatewayFailure> {
    if config.web_accounts.denied_users.contains(&account.username) {
        return Err(GatewayFailure::Forbidden);
    }
    let actual = if account.kind == AccountKind::Linux {
        super::security::account(&account.username)
    } else {
        super::security::account_by_uid(account.uid)
    }
    .map_err(|_| GatewayFailure::Forbidden)?;
    if !matches_nss(account, &actual) {
        return Err(GatewayFailure::Forbidden);
    }
    if account.kind == AccountKind::Linux {
        linux_policy(config, &actual, true)?;
    } else if account.uid < config.web_accounts.virtual_min_uid
        || account.uid > config.web_accounts.virtual_max_uid
        || account.uid == 0
        || account.gid == 0
        || config.web_accounts.server_uid == Some(account.uid)
        || !matches!(
            actual.shell.as_deref(),
            Some("/usr/sbin/nologin" | "/sbin/nologin" | "/bin/false")
        )
        || !crate::accounts::virtuals::locked_os_password(&actual.username)
    {
        return Err(GatewayFailure::Forbidden);
    }
    Ok(())
}
/// Config and caller are captured here, before opening the root-only DB; no request-selected path.
pub fn execute(command: AccountCommand) -> Result<AccountReply, GatewayFailure> {
    if unsafe { libc::geteuid() } != 0 {
        return Err(GatewayFailure::Forbidden);
    }
    let config = super::security::trusted_config().map_err(|_| GatewayFailure::Unavailable)?;
    let caller = CallerBinding::capture().map_err(|_| GatewayFailure::Forbidden)?;
    if !matches!(
        &command,
        AccountCommand::LocalEnroll { .. }
            | AccountCommand::LocalStatus
            | AccountCommand::LocalRevoke
            | AccountCommand::LocalPolicy
            | AccountCommand::LocalSetPolicy { .. }
    ) {
        authorize_caller(&config, &caller)?;
    } else if !config.web_accounts.enabled
        || caller.uid == 0
        || config.web_accounts.server_uid == Some(caller.uid)
    {
        return Err(GatewayFailure::Forbidden);
    }
    let mut store = AccountStore::open(std::path::Path::new(crate::layout::ACCOUNTS))
        .map_err(GatewayFailure::from)?;
    dispatch(&mut store, &config, caller, command)
}
fn account_view(store: &AccountStore, mut account: Account) -> Result<Account, GatewayFailure> {
    if account.kind == AccountKind::Virtual {
        account.quota_bytes = Some(
            store
                .effective_quota(&account.id)
                .map_err(GatewayFailure::from)?
                .unwrap_or(0),
        );
    }
    Ok(account)
}
fn management_authority(
    config: &AppConfig,
    actor: &Account,
    kind: AccountKind,
) -> Result<crate::accounts::store::Authority, GatewayFailure> {
    use crate::accounts::store::{Authority, Role};
    if actor.role != Role::Admin {
        return Err(GatewayFailure::Forbidden);
    }
    if actor.kind == AccountKind::Linux && config.account_policy.is_admin(&actor.username) {
        Ok(Authority::Root)
    } else if kind == AccountKind::Virtual {
        Ok(Authority::Account {
            id: actor.id.clone(),
            generation: actor.generation,
        })
    } else {
        Err(GatewayFailure::Forbidden)
    }
}

fn dispatch(
    store: &mut AccountStore,
    config: &AppConfig,
    caller: CallerBinding,
    command: AccountCommand,
) -> Result<AccountReply, GatewayFailure> {
    let at = crate::session::store::InMemorySessionStore::now_epoch_seconds();
    match command {
        AccountCommand::LocalEnroll {
            mut linux_password,
            web_password,
        } => {
            let identity = super::security::account_by_uid(caller.uid)
                .map_err(|_| GatewayFailure::Forbidden)?;
            let proof = dispatch(
                store,
                config,
                caller.clone(),
                AccountCommand::BeginEnrollment(AuthRequest {
                    username: identity.username,
                    password: std::mem::take(&mut *linux_password),
                    ip: None,
                }),
            )?;
            let AccountReply::Enrollment(proof) = proof else {
                return Err(GatewayFailure::Forbidden);
            };
            let reply = dispatch(
                store,
                config,
                caller.clone(),
                AccountCommand::CompleteEnrollment {
                    proof,
                    password: web_password,
                },
            )?;
            let AccountReply::Authenticated { account, grant } = reply else {
                return Err(GatewayFailure::Forbidden);
            };
            store
                .revoke_grant(&grant, &caller, at)
                .map_err(GatewayFailure::from)?;
            Ok(AccountReply::AccountUpdated(account))
        }
        AccountCommand::LocalStatus
        | AccountCommand::LocalRevoke
        | AccountCommand::LocalPolicy
        | AccountCommand::LocalSetPolicy { .. } => {
            let account = store
                .find_uid(caller.uid)
                .map_err(GatewayFailure::from)?
                .ok_or(GatewayFailure::Forbidden)?;
            validate_account(config, &account)?;
            let authority = crate::accounts::store::Authority::Account {
                id: account.id.clone(),
                generation: account.generation,
            };
            match command {
                AccountCommand::LocalStatus => Ok(AccountReply::AccountUpdated(account)),
                AccountCommand::LocalRevoke => {
                    if account.kind != AccountKind::Linux {
                        return Err(GatewayFailure::Forbidden);
                    }
                    Ok(AccountReply::AccountUpdated(
                        store
                            .transition(
                                &authority,
                                &account.id,
                                crate::accounts::store::AccountStatus::Revoked,
                                true,
                            )
                            .map_err(GatewayFailure::from)?,
                    ))
                }
                AccountCommand::LocalPolicy => {
                    let mut rules = store
                        .policy(&authority, &account.id)
                        .map_err(GatewayFailure::from)?;
                    rules
                        .root_rules
                        .extend(config.access_policy.root_rules.clone());
                    if let Some(trusted) = config.access_policy.users.get(&account.username) {
                        rules.root_rules.extend(trusted.root_rules.clone());
                        rules.root_rules.extend(trusted.admin_rules.clone());
                        rules.root_rules.extend(trusted.personal_rules.clone());
                    }
                    Ok(AccountReply::Policy {
                        rules,
                        account,
                        deny_system_components: config.access_policy.deny_system_components.clone(),
                    })
                }
                AccountCommand::LocalSetPolicy {
                    expected_generation,
                    rules,
                } => Ok(AccountReply::AccountUpdated(
                    store
                        .set_policy(
                            &authority,
                            &account.id,
                            expected_generation,
                            crate::accounts::policies::PolicyLayer::Personal,
                            rules,
                        )
                        .map_err(GatewayFailure::from)?,
                )),
                _ => Err(GatewayFailure::Forbidden),
            }
        }
        AccountCommand::CreateVirtual {
            grant,
            name,
            quota_bytes,
            password,
            idempotency_key,
        } => {
            if !config.virtual_accounts.enabled || config.home_template.is_none() {
                return Err(GatewayFailure::Disabled);
            }
            let actor = store
                .verify_grant(&grant, &caller, at)
                .map_err(GatewayFailure::from)?;
            validate_account(config, &actor)?;
            management_authority(config, &actor, AccountKind::Virtual)?;
            let created = crate::accounts::virtuals::provision(
                store,
                config,
                &actor,
                &name,
                quota_bytes,
                &password,
                &idempotency_key,
            )
            .map_err(GatewayFailure::from)?;
            Ok(AccountReply::AccountUpdated(created))
        }
        AccountCommand::SetVirtualQuota {
            grant,
            account_id,
            expected_generation,
            quota_bytes,
        } => {
            if !config.virtual_accounts.enabled || config.home_template.is_none() {
                return Err(GatewayFailure::Disabled);
            }
            let actor = store
                .verify_grant(&grant, &caller, at)
                .map_err(GatewayFailure::from)?;
            validate_account(config, &actor)?;
            let authority = management_authority(config, &actor, AccountKind::Virtual)?;
            let target = store.get(&account_id).map_err(GatewayFailure::from)?;
            store
                .policy(&authority, &target.id)
                .map_err(GatewayFailure::from)?;
            if target.kind != AccountKind::Virtual
                || target.role != crate::accounts::store::Role::User
            {
                return Err(GatewayFailure::Forbidden);
            }
            let updated = crate::accounts::virtuals::set_quota(
                store,
                config,
                &authority,
                &target,
                expected_generation,
                quota_bytes,
            )
            .map_err(GatewayFailure::from)?;
            Ok(AccountReply::AccountUpdated(updated))
        }
        AccountCommand::DeleteVirtual {
            grant,
            account_id,
            expected_generation,
        } => {
            if !config.virtual_accounts.enabled || !config.virtual_accounts.allow_admin_delete {
                return Err(GatewayFailure::Disabled);
            }
            let actor = store
                .verify_grant(&grant, &caller, at)
                .map_err(GatewayFailure::from)?;
            validate_account(config, &actor)?;
            let authority = management_authority(config, &actor, AccountKind::Virtual)?;
            let target = store.get(&account_id).map_err(GatewayFailure::from)?;
            if target.kind != AccountKind::Virtual
                || target.role != crate::accounts::store::Role::User
            {
                return Err(GatewayFailure::Forbidden);
            }
            if target.generation != expected_generation {
                return Err(GatewayFailure::Conflict);
            }
            let updated = store
                .transition_expected(
                    &authority,
                    &target.id,
                    crate::accounts::store::AccountStatus::Deleted,
                    true,
                    expected_generation,
                )
                .map_err(GatewayFailure::from)?;
            Ok(AccountReply::AccountUpdated(updated))
        }
        AccountCommand::Budget(grant) => {
            let actor = store
                .verify_grant(&grant, &caller, at)
                .map_err(GatewayFailure::from)?;
            validate_account(config, &actor)?;
            management_authority(config, &actor, AccountKind::Virtual)?;
            let mut budget = store.budget(&actor.id).map_err(GatewayFailure::from)?;
            if actor.kind == AccountKind::Virtual {
                let root = crate::accounts::virtuals::root(config, &actor)
                    .map_err(GatewayFailure::from)?;
                budget["used_bytes"] = serde_json::Value::from(
                    crate::accounts::virtuals::usage(config, &root, &actor)
                        .map_err(GatewayFailure::from)?,
                );
            }
            budget["available_bytes"] = serde_json::Value::from(
                budget["remaining_bytes"]
                    .as_u64()
                    .unwrap_or(0)
                    .saturating_sub(budget["used_bytes"].as_u64().unwrap_or(0)),
            );
            budget["minimum_quota_bytes"] =
                serde_json::Value::from(crate::accounts::virtuals::minimum_quota(config));
            budget["enforcement"] = serde_json::to_value(config.virtual_accounts.enforcement)
                .map_err(|_| GatewayFailure::Unavailable)?;
            Ok(AccountReply::Budget(budget))
        }
        AccountCommand::GetPolicy {
            grant,
            account_id,
            kind,
        } => {
            let actor = store
                .verify_grant(&grant, &caller, at)
                .map_err(GatewayFailure::from)?;
            validate_account(config, &actor)?;
            let (target, authority) = policy_target(store, config, &actor, account_id, kind)?;
            let mut rules = store
                .policy(&authority, &target.id)
                .map_err(GatewayFailure::from)?;
            rules
                .root_rules
                .extend(config.access_policy.root_rules.clone());
            if let Some(trusted) = config.access_policy.users.get(&target.username) {
                rules.root_rules.extend(trusted.root_rules.clone());
                // TOML rules remain Root-owned even if their historical labels
                // describe an admin/personal source. Web edits cannot remove them.
                rules.root_rules.extend(trusted.admin_rules.clone());
                rules.root_rules.extend(trusted.personal_rules.clone());
            }
            Ok(AccountReply::Policy {
                account: target,
                rules,
                deny_system_components: config.access_policy.deny_system_components.clone(),
            })
        }
        AccountCommand::SetPolicy {
            grant,
            account_id,
            kind,
            expected_generation,
            layer,
            rules,
        } => {
            use crate::accounts::policies::PolicyLayer;
            if layer == PolicyLayer::Root || rules.len() > 512 || expected_generation == 0 {
                return Err(GatewayFailure::Forbidden);
            }
            let actor = store
                .verify_grant(&grant, &caller, at)
                .map_err(GatewayFailure::from)?;
            validate_account(config, &actor)?;
            if (layer == PolicyLayer::Personal && (account_id.is_some() || kind.is_some()))
                || (layer == PolicyLayer::Admin && (account_id.is_none() || kind.is_none()))
            {
                return Err(GatewayFailure::Forbidden);
            }
            let (target, authority) = policy_target(store, config, &actor, account_id, kind)?;
            if layer == PolicyLayer::Admin && target.role == crate::accounts::store::Role::Admin {
                return Err(GatewayFailure::Forbidden);
            }
            let updated = store
                .set_policy(&authority, &target.id, expected_generation, layer, rules)
                .map_err(GatewayFailure::from)?;
            Ok(AccountReply::AccountUpdated(updated))
        }
        AccountCommand::ApplyUndo {
            grant,
            operation_id,
            expected_state,
        } => super::undo::apply(
            store,
            config,
            &caller,
            grant,
            &operation_id,
            &expected_state,
        )
        .map(|response| AccountReply::UndoApplied(Box::new(response))),
        AccountCommand::UndoPreferences { grant } => {
            if !config.journal.enabled {
                return Err(GatewayFailure::Disabled);
            }
            let actor = store
                .verify_grant(&grant, &caller, at)
                .map_err(GatewayFailure::from)?;
            validate_account(config, &actor)?;
            Ok(AccountReply::UndoPreferences(
                store
                    .undo_preferences(
                        &crate::accounts::store::Authority::Account {
                            id: actor.id,
                            generation: actor.generation,
                        },
                        &config.journal,
                    )
                    .map_err(GatewayFailure::from)?,
            ))
        }
        AccountCommand::SetUndoPreferences {
            grant,
            hours,
            expected_revision,
        } => {
            if !config.journal.enabled {
                return Err(GatewayFailure::Disabled);
            }
            let actor = store
                .verify_grant(&grant, &caller, at)
                .map_err(GatewayFailure::from)?;
            validate_account(config, &actor)?;
            Ok(AccountReply::UndoPreferences(
                store
                    .set_undo_preferences(
                        &crate::accounts::store::Authority::Account {
                            id: actor.id,
                            generation: actor.generation,
                        },
                        &config.journal,
                        hours,
                        expected_revision,
                    )
                    .map_err(GatewayFailure::from)?,
            ))
        }
        AccountCommand::History {
            grant,
            query,
            undo_only,
        } => {
            if !config.journal.enabled {
                return Err(GatewayFailure::Disabled);
            }
            query.validate().map_err(GatewayFailure::from)?;
            let actor = store
                .verify_grant(&grant, &caller, at)
                .map_err(GatewayFailure::from)?;
            validate_account(config, &actor)?;
            let mut view =
                crate::policy::home::resolve(config, &actor, config.home_template.as_ref())
                    .map_err(|_| GatewayFailure::Forbidden)?;
            apply_persisted_policy(&mut view, &actor)?;
            let authority = crate::accounts::store::Authority::Account {
                id: actor.id.clone(),
                generation: actor.generation,
            };
            let mut cursor = query.cursor.clone();
            let mut items = Vec::new();
            let mut scanned = 0usize;
            let mut has_more = false;
            loop {
                let batch = store
                    .operation_records_filtered(
                        &authority,
                        &crate::accounts::journal::HistoryQuery {
                            limit: 200,
                            cursor: cursor.clone(),
                            ..query.clone()
                        },
                    )
                    .map_err(GatewayFailure::from)?;
                let count = batch.len();
                if count == 0 {
                    break;
                }
                for record in batch {
                    scanned += 1;
                    cursor = Some(crate::accounts::store::AccountStore::operation_cursor(
                        &record,
                    ));
                    let visible =
                        view.roots.iter().any(|root| {
                            root.root_id == record.root_id && root.base_path == actor.home
                        }) && view
                            .access_policy
                            .check(
                                &actor.username,
                                &record.root_id,
                                &record.source_path,
                                crate::policy::access::Access::Discover,
                            )
                            .is_ok()
                            && (record.action == "trash"
                                || record.target_path.as_ref().is_none_or(|path| {
                                    view.access_policy
                                        .check(
                                            &actor.username,
                                            &record.root_id,
                                            path,
                                            crate::policy::access::Access::Discover,
                                        )
                                        .is_ok()
                                }));
                    if visible
                        && (!undo_only
                            || ["move", "rename", "trash", "chmod"]
                                .contains(&record.action.as_str()))
                    {
                        items.push(record);
                    }
                    if items.len() > usize::from(query.limit) {
                        has_more = true;
                        break;
                    }
                }
                if has_more || count < 200 {
                    break;
                }
                if scanned >= 1000 {
                    has_more = true;
                    break;
                }
            }
            if items.len() > usize::from(query.limit) {
                items.pop();
                cursor = items
                    .last()
                    .map(crate::accounts::store::AccountStore::operation_cursor);
            }
            if undo_only {
                let items = items
                    .into_iter()
                    .map(|record| store.undo_candidate(&authority, &config.journal, &record.id, at))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(GatewayFailure::from)?;
                return Ok(AccountReply::UndoPage {
                    items,
                    next_cursor: if has_more { cursor } else { None },
                    has_more,
                });
            }
            Ok(AccountReply::History(
                crate::accounts::journal::HistoryPage {
                    items,
                    next_cursor: if has_more { cursor } else { None },
                    has_more,
                },
            ))
        }
        AccountCommand::Login(request) => {
            let reservation = store
                .reserve_login(
                    &request.username,
                    request.ip.as_deref(),
                    &config.rate_limit,
                    at,
                )
                .map_err(GatewayFailure::from)?;
            let account = store
                .verify_password(&request.username, &request.password)
                .map_err(GatewayFailure::from)?;
            validate_account(config, &account)?;
            store
                .finish_login(reservation)
                .map_err(GatewayFailure::from)?;
            let grant = store
                .issue_grant(
                    &account.id,
                    account.generation,
                    caller,
                    at,
                    config.session.absolute_timeout_seconds.min(43200),
                )
                .map_err(GatewayFailure::from)?;
            Ok(AccountReply::Authenticated {
                account: account_view(store, account)?,
                grant,
            })
        }
        AccountCommand::BeginEnrollment(request) => {
            let reservation = store
                .reserve_login(
                    &request.username,
                    request.ip.as_deref(),
                    &config.rate_limit,
                    at,
                )
                .map_err(GatewayFailure::from)?;
            let existing = store
                .find_username(&request.username)
                .map_err(GatewayFailure::from)?;
            if existing.as_ref().is_some_and(|a| {
                a.kind != AccountKind::Linux
                    || a.deny_enrollment
                    || a.password_enrolled && a.login_enabled
            }) {
                return Err(GatewayFailure::Forbidden);
            }
            let actual = super::security::account(&request.username)
                .map_err(|_| GatewayFailure::InvalidCredentials)?;
            let policy = linux_policy(config, &actual, existing.is_some())?;
            if existing.as_ref().is_some_and(|a| !matches_nss(a, &actual)) {
                return Err(GatewayFailure::Forbidden);
            }
            let before = snapshot(actual)?;
            let authenticated =
                crate::auth::pam::authenticate_local(&request, &policy).map_err(|error| {
                    if error == crate::auth::AuthError::AuthBackendUnavailable {
                        GatewayFailure::Unavailable
                    } else {
                        GatewayFailure::InvalidCredentials
                    }
                })?;
            if snapshot(authenticated)? != before {
                return Err(GatewayFailure::Forbidden);
            }
            store
                .finish_login(reservation)
                .map_err(GatewayFailure::from)?;
            let proof = store
                .begin_enrollment(&before, caller, at, 300)
                .map_err(GatewayFailure::from)?;
            Ok(AccountReply::Enrollment(proof))
        }
        AccountCommand::CompleteEnrollment { proof, password } => {
            let enrolled = store
                .enrollment_identity(&proof.id, &caller, at)
                .map_err(GatewayFailure::from)?;
            let existing = store
                .find_username(&enrolled.username)
                .map_err(GatewayFailure::from)?;
            let fresh = super::security::account(&enrolled.username)
                .map_err(|_| GatewayFailure::Forbidden)?;
            linux_policy(config, &fresh, existing.is_some())?;
            let account = store
                .complete_enrollment(&proof.id, &caller, &snapshot(fresh)?, &password, at)
                .map_err(GatewayFailure::from)?;
            let grant = store
                .issue_grant(
                    &account.id,
                    account.generation,
                    caller,
                    at,
                    config.session.absolute_timeout_seconds.min(43200),
                )
                .map_err(GatewayFailure::from)?;
            Ok(AccountReply::Authenticated {
                account: account_view(store, account)?,
                grant,
            })
        }
        AccountCommand::Validate(grant) => {
            let account = store
                .verify_grant(&grant, &caller, at)
                .map_err(GatewayFailure::from)?;
            validate_account(config, &account)?;
            Ok(AccountReply::Validated(account_view(store, account)?))
        }
        AccountCommand::ApproveLinux { grant, username } => {
            let actor = store
                .verify_grant(&grant, &caller, at)
                .map_err(GatewayFailure::from)?;
            validate_account(config, &actor)?;
            let authority = management_authority(config, &actor, AccountKind::Linux)?;
            if username.is_empty()
                || username.len() > 256
                || username
                    .chars()
                    .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '/' | '\\' | ':'))
            {
                return Err(GatewayFailure::Forbidden);
            }
            if store
                .find_username(&username)
                .map_err(GatewayFailure::from)?
                .is_some()
            {
                return Err(GatewayFailure::Conflict);
            }
            let identity =
                super::security::account(&username).map_err(|_| GatewayFailure::Forbidden)?;
            linux_policy(config, &identity, true)?;
            let account = store
                .create(
                    &authority,
                    crate::accounts::store::NewAccount {
                        username: identity.username,
                        uid: identity.uid,
                        gid: identity.primary_gid,
                        home: identity.home_dir.ok_or(GatewayFailure::Forbidden)?,
                        kind: AccountKind::Linux,
                        role: crate::accounts::store::Role::User,
                        owner_admin_id: None,
                        quota_bytes: None,
                    },
                )
                .map_err(|error| match error {
                    StoreError::Database(rusqlite::Error::SqliteFailure(ref e, _))
                        if e.code == rusqlite::ErrorCode::ConstraintViolation =>
                    {
                        GatewayFailure::Conflict
                    }
                    _ => GatewayFailure::from(error),
                })?;
            Ok(AccountReply::AccountUpdated(account))
        }
        AccountCommand::CandidateRegistration { grant, names } => {
            let actor = store
                .verify_grant(&grant, &caller, at)
                .map_err(GatewayFailure::from)?;
            validate_account(config, &actor)?;
            management_authority(config, &actor, AccountKind::Linux)?;
            if names.len() > 1024
                || names.iter().any(|name| {
                    name.is_empty() || name.len() > 256 || name.chars().any(char::is_control)
                })
            {
                return Err(GatewayFailure::Forbidden);
            }
            let mut registered = Vec::new();
            for name in names {
                if store
                    .find_username(&name)
                    .map_err(GatewayFailure::from)?
                    .is_some()
                {
                    registered.push(name);
                }
            }
            Ok(AccountReply::RegisteredNames(registered))
        }
        AccountCommand::ListAccountPage { grant, kind, query } => {
            let actor = store
                .verify_grant(&grant, &caller, at)
                .map_err(GatewayFailure::from)?;
            validate_account(config, &actor)?;
            let authority = management_authority(config, &actor, kind)?;
            Ok(AccountReply::AccountPage(
                store
                    .list_page(&authority, kind, &query)
                    .map_err(GatewayFailure::from)?,
            ))
        }
        AccountCommand::ListAccounts { grant, kind } => {
            let actor = store
                .verify_grant(&grant, &caller, at)
                .map_err(GatewayFailure::from)?;
            validate_account(config, &actor)?;
            let authority = management_authority(config, &actor, kind)?;
            let accounts = store.list(&authority, kind).map_err(GatewayFailure::from)?;
            Ok(AccountReply::Accounts(accounts))
        }
        AccountCommand::SetAccountAccess {
            grant,
            account_id,
            kind,
            enabled,
        } => {
            if account_id.len() > 128 {
                return Err(GatewayFailure::Forbidden);
            }
            let actor = store
                .verify_grant(&grant, &caller, at)
                .map_err(GatewayFailure::from)?;
            validate_account(config, &actor)?;
            let target = store.get(&account_id).map_err(GatewayFailure::from)?;
            if target.kind != kind {
                return Err(GatewayFailure::Forbidden);
            }
            let authority = management_authority(config, &actor, target.kind)?;
            if target.id == actor.id || target.role == crate::accounts::store::Role::Admin {
                return Err(GatewayFailure::Forbidden);
            }
            // Enabling never overrides root eligibility or root explicit deny.
            if enabled {
                validate_account(config, &target)?;
            }
            let result = store
                .transition(
                    &authority,
                    &target.id,
                    if enabled {
                        crate::accounts::store::AccountStatus::Active
                    } else {
                        crate::accounts::store::AccountStatus::Suspended
                    },
                    false,
                )
                .map_err(GatewayFailure::from)?;
            Ok(AccountReply::AccountUpdated(result))
        }
        AccountCommand::RevokeSelf(grant) => {
            let account = store
                .verify_grant(&grant, &caller, at)
                .map_err(GatewayFailure::from)?;
            validate_account(config, &account)?;
            if account.kind != AccountKind::Linux {
                return Err(GatewayFailure::Forbidden);
            }
            store
                .transition(
                    &crate::accounts::store::Authority::Account {
                        id: account.id.clone(),
                        generation: account.generation,
                    },
                    &account.id,
                    crate::accounts::store::AccountStatus::Revoked,
                    false,
                )
                .map_err(GatewayFailure::from)?;
            Ok(AccountReply::AccessRevoked)
        }
        AccountCommand::ChangePassword {
            grant,
            current_password,
            new_password,
        } => {
            let account = store
                .verify_grant(&grant, &caller, at)
                .map_err(GatewayFailure::from)?;
            validate_account(config, &account)?;
            let reservation = store
                .reserve_login(&account.username, None, &config.rate_limit, at)
                .map_err(GatewayFailure::from)?;
            let verified = store
                .verify_password(&account.username, &current_password)
                .map_err(GatewayFailure::from)?;
            if verified.id != account.id || verified.generation != account.generation {
                return Err(GatewayFailure::Forbidden);
            }
            store
                .finish_login(reservation)
                .map_err(GatewayFailure::from)?;
            store
                .set_password(
                    &crate::accounts::store::Authority::Account {
                        id: account.id.clone(),
                        generation: account.generation,
                    },
                    &account.id,
                    &new_password,
                )
                .map_err(GatewayFailure::from)?;
            Ok(AccountReply::PasswordChanged)
        }
        AccountCommand::Logout(grant) => {
            store
                .revoke_grant(&grant, &caller, at)
                .map_err(GatewayFailure::from)?;
            Ok(AccountReply::LoggedOut)
        }
    }
}

fn policy_target(
    store: &AccountStore,
    config: &AppConfig,
    actor: &Account,
    id: Option<String>,
    kind: Option<AccountKind>,
) -> Result<(Account, crate::accounts::store::Authority), GatewayFailure> {
    match (id, kind) {
        (None, None) => Ok((
            actor.clone(),
            crate::accounts::store::Authority::Account {
                id: actor.id.clone(),
                generation: actor.generation,
            },
        )),
        (Some(id), Some(kind)) if id.len() <= 128 => {
            let authority = management_authority(config, actor, kind)?;
            let target = store.get(&id).map_err(GatewayFailure::from)?;
            if target.kind != kind {
                return Err(GatewayFailure::Forbidden);
            }
            store
                .policy(&authority, &id)
                .map_err(GatewayFailure::from)?;
            Ok((target, authority))
        }
        _ => Err(GatewayFailure::Forbidden),
    }
}

/// Read only after the operation grant and current NSS identity were verified.
pub(crate) fn apply_persisted_policy(
    config: &mut AppConfig,
    account: &Account,
) -> Result<(), GatewayFailure> {
    let store = AccountStore::open(std::path::Path::new(crate::layout::ACCOUNTS))
        .map_err(GatewayFailure::from)?;
    let rules = store
        .policy(&crate::accounts::store::Authority::Root, &account.id)
        .map_err(GatewayFailure::from)?;
    let mut ceiling = crate::policy::access::AccessPolicy::default();
    ceiling.users.insert(account.username.clone(), rules);
    config.access_policy.extend_ceiling(&ceiling);
    config
        .access_policy
        .validate()
        .map_err(|_| GatewayFailure::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity() -> AuthSuccess {
        AuthSuccess {
            username: "alice".into(),
            uid: 1000,
            primary_gid: 1000,
            home_dir: Some("/home/alice".into()),
            shell: Some("/bin/bash".into()),
            groups: vec![],
        }
    }
    fn caller() -> CallerBinding {
        CallerBinding {
            uid: 995,
            pid: 1,
            start_time: 1,
            boot_id: "00000000-0000-4000-8000-000000000001".into(),
        }
    }
    fn config() -> AppConfig {
        let mut config = AppConfig::default();
        config.web_accounts.enabled = true;
        config.web_accounts.server_uid = Some(995);
        config
    }
    #[test]
    fn management_authority_keeps_root_roster_and_tenant_boundaries() {
        use crate::accounts::store::{Authority, NewAccount, Role};
        let mut store = AccountStore::memory();
        let mut config = config();
        let mut actor = store
            .create(
                &Authority::Root,
                NewAccount {
                    username: "operator".into(),
                    uid: 1000,
                    gid: 1000,
                    home: "/home/operator".into(),
                    kind: AccountKind::Linux,
                    role: Role::Admin,
                    owner_admin_id: None,
                    quota_bytes: None,
                },
            )
            .unwrap();
        assert!(management_authority(&config, &actor, AccountKind::Linux).is_err());
        config.account_policy.admin_users.push("operator".into());
        assert!(matches!(
            management_authority(&config, &actor, AccountKind::Linux),
            Ok(Authority::Root)
        ));
        actor.kind = AccountKind::Virtual;
        assert!(management_authority(&config, &actor, AccountKind::Linux).is_err());
        assert!(matches!(
            management_authority(&config, &actor, AccountKind::Virtual),
            Ok(Authority::Account { .. })
        ));
        actor.role = Role::User;
        assert!(management_authority(&config, &actor, AccountKind::Virtual).is_err());
    }
    #[test]
    fn backend_must_be_explicit_and_nonroot_caller_bound() {
        assert!(matches!(
            authorize_caller(&AppConfig::default(), &caller()),
            Err(GatewayFailure::Disabled)
        ));
        let mut config = config();
        assert!(authorize_caller(&config, &caller()).is_ok());
        config.web_accounts.server_uid = Some(996);
        assert!(matches!(
            authorize_caller(&config, &caller()),
            Err(GatewayFailure::Forbidden)
        ));
        config.web_accounts.server_uid = Some(0);
        assert!(config.validate().is_err());
        config.web_accounts.server_uid = None;
        assert!(config.validate().is_err());
    }
    #[test]
    fn enrollment_extends_only_subject_and_never_root_policy() {
        let config = config();
        let policy = linux_policy(&config, &identity(), false).unwrap();
        assert_eq!(policy.allowed_users, vec!["alice"]);
        assert!(config.account_policy.allowed_users.is_empty());
        assert!(
            policy.reject_root && policy.reject_system_accounts && policy.reject_disabled_shells
        );
    }
    #[test]
    fn registration_off_preserves_approved_accounts_only() {
        let mut config = config();
        config.web_accounts.self_registration = false;
        assert!(linux_policy(&config, &identity(), false).is_err());
        assert!(linux_policy(&config, &identity(), true).is_ok());
        config.account_policy.allowed_users.push("alice".into());
        assert!(linux_policy(&config, &identity(), false).is_ok());
        config.web_accounts.denied_users.push("alice".into());
        assert!(linux_policy(&config, &identity(), true).is_err());
    }
    #[test]
    fn service_root_nologin_and_virtual_uids_cannot_self_enroll() {
        let config = config();
        for uid in [0, 995, 999, 200000, 299999] {
            let mut identity = identity();
            identity.uid = uid;
            assert!(linux_policy(&config, &identity, true).is_err());
        }
        for shell in ["/usr/sbin/nologin", "/sbin/nologin", "/bin/false"] {
            let mut identity = identity();
            identity.shell = Some(shell.into());
            assert!(linux_policy(&config, &identity, true).is_err());
        }
        for username in ["caddy", "www-data", "tealdrive", "root"] {
            let mut identity = identity();
            identity.username = username.into();
            assert!(linux_policy(&config, &identity, true).is_err());
        }
        let mut identity = identity();
        identity.primary_gid = 0;
        assert!(linux_policy(&config, &identity, true).is_err());
    }
    #[test]
    fn deny_names_and_uid_range_config_are_validated() {
        let mut config = config();
        config.web_accounts.denied_users.push("alice/other".into());
        assert!(config.validate().is_err());
        config.web_accounts.denied_users.clear();
        config.web_accounts.virtual_max_uid = 199999;
        assert!(config.validate().is_err());
    }
    #[test]
    fn wire_enrollment_password_roundtrips_and_reply_debug_redacts_proofs() {
        let command = AccountCommand::CompleteEnrollment {
            proof: EnrollmentProof {
                id: uuid::Uuid::new_v4().to_string(),
                expires_at: 100,
            },
            password: zeroize::Zeroizing::new("new-web-password".into()),
        };
        let wire = zeroize::Zeroizing::new(rmp_serde::to_vec_named(&command).unwrap());
        let decoded: AccountCommand = rmp_serde::from_slice(&wire).unwrap();
        match decoded {
            AccountCommand::CompleteEnrollment { password, .. } => {
                assert_eq!(&*password, "new-web-password")
            }
            _ => panic!("wrong command"),
        }
        let proof = EnrollmentProof {
            id: "secret-enrollment-token".into(),
            expires_at: 100,
        };
        assert!(
            !format!("{:?}", AccountReply::Enrollment(proof)).contains("secret-enrollment-token")
        );
        let grant = SessionGrant {
            nonce: "secret-session-nonce".into(),
            account_id: "alice".into(),
            generation: 1,
            caller: caller(),
            expires_at: 100,
        };
        assert!(!format!("{grant:?}").contains("secret-session-nonce"));
    }
    #[test]
    #[ignore = "requires root and explicit dedicated PAM fixture credentials"]
    fn pam_enrollment_web_login_and_session_revocation() {
        assert_eq!(unsafe { libc::geteuid() }, 0);
        let username = std::env::var("TEALDRIVE_TEST_USERNAME").unwrap();
        let linux_password = zeroize::Zeroizing::new(
            std::fs::read_to_string(std::env::var("TEALDRIVE_TEST_PASSWORD_FILE").unwrap())
                .unwrap(),
        );
        let mut store = AccountStore::memory();
        let mut config = config();
        let principal = super::super::security::account(&username).unwrap();
        assert!(principal.uid >= 1000 && principal.uid < config.web_accounts.virtual_min_uid);
        let binding = caller();
        let proof = match dispatch(
            &mut store,
            &config,
            binding.clone(),
            AccountCommand::BeginEnrollment(AuthRequest::new(&username, linux_password.trim())),
        )
        .unwrap()
        {
            AccountReply::Enrollment(proof) => proof,
            _ => panic!("expected enrollment"),
        };
        // A root denial introduced while the enrollment form is open wins.
        config.web_accounts.denied_users.push(username.clone());
        assert!(matches!(
            dispatch(
                &mut store,
                &config,
                binding.clone(),
                AccountCommand::CompleteEnrollment {
                    proof: proof.clone(),
                    password: zeroize::Zeroizing::new("private-web-password-for-test".into())
                }
            ),
            Err(GatewayFailure::Forbidden)
        ));
        config.web_accounts.denied_users.clear();
        let (account, first) = match dispatch(
            &mut store,
            &config,
            binding.clone(),
            AccountCommand::CompleteEnrollment {
                proof: proof.clone(),
                password: zeroize::Zeroizing::new("private-web-password-for-test".into()),
            },
        )
        .unwrap()
        {
            AccountReply::Authenticated { account, grant } => (account, grant),
            _ => panic!("expected authenticated"),
        };
        assert!(matches!(
            dispatch(
                &mut store,
                &config,
                binding.clone(),
                AccountCommand::ListAccounts {
                    grant: first.clone(),
                    kind: AccountKind::Linux
                }
            ),
            Err(GatewayFailure::Forbidden)
        ));
        assert!(matches!(
            dispatch(
                &mut store,
                &config,
                binding.clone(),
                AccountCommand::SetAccountAccess {
                    grant: first.clone(),
                    account_id: account.id.clone(),
                    kind: AccountKind::Linux,
                    enabled: false
                }
            ),
            Err(GatewayFailure::Forbidden)
        ));
        assert_eq!(account.uid, principal.uid);
        assert_eq!(account.home, principal.home_dir.unwrap());
        assert!(dispatch(
            &mut store,
            &config,
            binding.clone(),
            AccountCommand::CompleteEnrollment {
                proof,
                password: zeroize::Zeroizing::new("another-web-password-for-test".into())
            }
        )
        .is_err());
        assert!(matches!(
            dispatch(
                &mut store,
                &config,
                binding.clone(),
                AccountCommand::Login(AuthRequest::new(&username, linux_password.trim()))
            ),
            Err(GatewayFailure::InvalidCredentials)
        ));
        assert!(dispatch(
            &mut store,
            &config,
            binding.clone(),
            AccountCommand::BeginEnrollment(AuthRequest::new(&username, linux_password.trim()))
        )
        .is_err());
        let second = match dispatch(
            &mut store,
            &config,
            binding.clone(),
            AccountCommand::Login(AuthRequest::new(&username, "private-web-password-for-test")),
        )
        .unwrap()
        {
            AccountReply::Authenticated { grant, .. } => grant,
            _ => panic!("expected authenticated"),
        };
        dispatch(
            &mut store,
            &config,
            binding.clone(),
            AccountCommand::Logout(first.clone()),
        )
        .unwrap();
        assert!(dispatch(
            &mut store,
            &config,
            binding.clone(),
            AccountCommand::Validate(first)
        )
        .is_err());
        dispatch(
            &mut store,
            &config,
            binding.clone(),
            AccountCommand::Validate(second.clone()),
        )
        .unwrap();
        assert!(matches!(
            dispatch(
                &mut store,
                &config,
                binding.clone(),
                AccountCommand::ChangePassword {
                    grant: second.clone(),
                    current_password: zeroize::Zeroizing::new("wrong-password".into()),
                    new_password: zeroize::Zeroizing::new("replacement-web-password".into())
                }
            ),
            Err(GatewayFailure::InvalidCredentials)
        ));
        dispatch(
            &mut store,
            &config,
            binding.clone(),
            AccountCommand::ChangePassword {
                grant: second.clone(),
                current_password: zeroize::Zeroizing::new("private-web-password-for-test".into()),
                new_password: zeroize::Zeroizing::new("replacement-web-password".into()),
            },
        )
        .unwrap();
        assert!(dispatch(
            &mut store,
            &config,
            binding.clone(),
            AccountCommand::Validate(second.clone())
        )
        .is_err());
        assert!(store
            .verify_password(&username, "private-web-password-for-test")
            .is_err());
        let fresh = match dispatch(
            &mut store,
            &config,
            binding.clone(),
            AccountCommand::Login(AuthRequest::new(&username, "replacement-web-password")),
        )
        .unwrap()
        {
            AccountReply::Authenticated { grant, .. } => grant,
            _ => panic!("expected login"),
        };
        dispatch(
            &mut store,
            &config,
            binding.clone(),
            AccountCommand::RevokeSelf(fresh.clone()),
        )
        .unwrap();
        assert!(dispatch(
            &mut store,
            &config,
            binding.clone(),
            AccountCommand::Validate(fresh)
        )
        .is_err());
        assert!(!store.get(&account.id).unwrap().deny_enrollment);
        assert!(dispatch(
            &mut store,
            &config,
            binding.clone(),
            AccountCommand::BeginEnrollment(AuthRequest::new(&username, linux_password.trim()))
        )
        .is_ok());
        store
            .transition(
                &crate::accounts::store::Authority::Root,
                &account.id,
                crate::accounts::store::AccountStatus::Suspended,
                false,
            )
            .unwrap();
        assert!(dispatch(
            &mut store,
            &config,
            binding,
            AccountCommand::Validate(second)
        )
        .is_err());
    }
    #[test]
    fn virtual_account_view_uses_budget_minus_child_allocations() {
        use crate::accounts::store::{Authority, NewAccount, Role};
        let mut store = AccountStore::memory();
        let admin = store
            .create(
                &Authority::Root,
                NewAccount {
                    username: "tenant".into(),
                    uid: 200000,
                    gid: 200000,
                    home: "/home/tenant".into(),
                    kind: AccountKind::Virtual,
                    role: Role::Admin,
                    owner_admin_id: None,
                    quota_bytes: None,
                },
            )
            .unwrap();
        assert_eq!(
            account_view(&store, admin.clone()).unwrap().quota_bytes,
            Some(0)
        );
        store.set_pool(&Authority::Root, 1000).unwrap();
        store
            .set_admin_budget(&Authority::Root, &admin.id, 600)
            .unwrap();
        let child = store
            .create(
                &Authority::Root,
                NewAccount {
                    username: "child".into(),
                    uid: 200001,
                    gid: 200001,
                    home: "/home/child".into(),
                    kind: AccountKind::Virtual,
                    role: Role::User,
                    owner_admin_id: Some(admin.id.clone()),
                    quota_bytes: Some(200),
                },
            )
            .unwrap();
        assert_eq!(account_view(&store, admin).unwrap().quota_bytes, Some(400));
        assert_eq!(account_view(&store, child).unwrap().quota_bytes, Some(200));
    }
}
