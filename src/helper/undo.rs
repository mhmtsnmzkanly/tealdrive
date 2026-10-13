//! A signed Root record supplies inverse paths; the User worker verifies actual filesystem state.
use super::types::{HelperRequest, HelperResponse};
use crate::{accounts::journal::OperationRecord, config::AppConfig, errors::ErrorKind};
pub(super) fn apply(
    store: &mut crate::accounts::store::AccountStore,
    config: &AppConfig,
    caller: &crate::accounts::grants::CallerBinding,
    grant: crate::accounts::grants::SessionGrant,
    operation_id: &str,
    expected_state: &str,
) -> Result<HelperResponse, super::account_gateway::GatewayFailure> {
    use super::{
        account_gateway::GatewayFailure,
        types::{HelperCommand, HelperLimits, HelperPolicyContext},
    };
    if !config.journal.enabled {
        return Err(GatewayFailure::Disabled);
    }
    let at = crate::session::store::InMemorySessionStore::now_epoch_seconds();
    let actor = store.verify_grant(&grant, caller, at)?;
    super::account_gateway::validate_account(config, &actor)?;
    let mut view = crate::policy::home::resolve(config, &actor, config.home_template.as_ref())
        .map_err(|_| GatewayFailure::Forbidden)?;
    super::account_gateway::apply_persisted_policy(&mut view, &actor)?;
    let authority = crate::accounts::store::Authority::Account {
        id: actor.id.clone(),
        generation: actor.generation,
    };
    let candidate = store
        .undo_candidate(&authority, &config.journal, operation_id, at)
        .map_err(|error| match error {
            crate::accounts::store::StoreError::NotFound => GatewayFailure::Forbidden,
            other => GatewayFailure::from(other),
        })?;
    if !candidate.available || candidate.expected_state.as_deref() != Some(expected_state) {
        return Err(GatewayFailure::Conflict);
    }
    let record = candidate.operation;
    if record.action == "chmod" && !view.features.chmod_file {
        return Err(GatewayFailure::Disabled);
    }
    let root = view
        .roots
        .iter()
        .find(|root| root.root_id == record.root_id && root.base_path == actor.home)
        .ok_or(GatewayFailure::Forbidden)?;
    let _lock =
        super::quota::Reservation::acquire(root).map_err(|_| GatewayFailure::Unavailable)?;
    let fresh = store.verify_grant(
        &grant,
        caller,
        crate::session::store::InMemorySessionStore::now_epoch_seconds(),
    )?;
    super::account_gateway::validate_account(config, &fresh)?;
    // Rebuild current protected policy after waiting for the shared mutation lock.
    view = crate::policy::home::resolve(config, &fresh, config.home_template.as_ref())
        .map_err(|_| GatewayFailure::Forbidden)?;
    super::account_gateway::apply_persisted_policy(&mut view, &fresh)?;
    let root = view
        .roots
        .iter()
        .find(|root| root.root_id == record.root_id && root.base_path == actor.home)
        .ok_or(GatewayFailure::Forbidden)?;
    let request = HelperRequest {
        request_id: crate::protocol::RequestId::new(),
        username: actor.username.clone(),
        uid: actor.uid,
        gid: actor.gid,
        command: match record.action.as_str() {
            "chmod" => HelperCommand::Chmod,
            "trash" => HelperCommand::RestoreFromTrash,
            _ => HelperCommand::Move,
        },
        root_id: record.root_id.clone(),
        relative_path: if record.action == "chmod" || record.action == "trash" {
            record.source_path.clone()
        } else {
            record.target_path.clone().ok_or(GatewayFailure::Conflict)?
        },
        target_relative_path: if record.action == "chmod" {
            None
        } else {
            Some(record.source_path.clone())
        },
        policy_context: HelperPolicyContext {
            read_only_root: root.read_only,
            is_web_root: root.is_web_root,
        },
        limits: HelperLimits {
            max_directory_page_size: view.limits.max_directory_page_size,
            max_text_edit_size: view.limits.max_text_edit_size,
            max_chunk_size: view.limits.max_chunk_size,
            max_download_helper_bytes: view.limits.max_chunk_size,
        },
        list_options: None,
        write_options: None,
        chmod_mode: record.old_mode.map(crate::protocol::schema::UnixMode),
        archive_source_paths: None,
        search_options: None,
    };
    super::policy::authorize(&request, &view).map_err(|_| GatewayFailure::Forbidden)?;
    // Discover remains mandatory even for private trash aliases.
    view.access_policy
        .check(
            &actor.username,
            &record.root_id,
            &record.source_path,
            crate::policy::access::Access::Discover,
        )
        .map_err(|_| GatewayFailure::Forbidden)?;
    let claimed = store.claim_undo(
        &authority,
        &config.journal,
        operation_id,
        expected_state,
        crate::session::store::InMemorySessionStore::now_epoch_seconds(),
    )?;
    let result = super::worker::spawn_with_undo(&request, &view, None, Some(claimed.clone()));
    let finished = store.finish_undo(&claimed, &config.journal, result.as_ref().ok());
    match result {
        Ok(mut response) => {
            if finished.is_err() && response.success {
                response.safe_message =
                    Some("Operation undone; protected completion remains pending.".into());
            }
            Ok(response)
        }
        Err(_) => Err(GatewayFailure::Unavailable),
    }
}
pub(super) fn execute(
    request: &HelperRequest,
    config: &AppConfig,
    record: &OperationRecord,
) -> HelperResponse {
    let Some(expected) = record.after.as_ref() else {
        return HelperResponse::permission_denied(request, "Missing expected state.");
    };
    if record.action == "chmod" {
        return super::chmod::chmod_with_expected_state(
            request,
            &config.roots,
            &config.sensitive_policy,
            Some(expected),
        );
    }
    let Some(root) = config
        .roots
        .iter()
        .find(|root| root.root_id == record.root_id)
    else {
        return HelperResponse::permission_denied(request, "Unknown root.");
    };
    let Some(current) = record.target_path.as_ref() else {
        return HelperResponse::permission_denied(request, "Missing inverse source.");
    };
    let run = || {
        let observed = match super::observation::capture(root, current) {
            Ok(state) => state,
            Err(error) => return super::listing::helper_error_from_io(request, error),
        };
        if observed != *expected || !observed.fully_observed() {
            return super::listing::helper_error(
                request,
                ErrorKind::RevisionConflict,
                "Target changed since the recorded operation.",
                "undo_expected_state_mismatch",
            );
        }
        if let Err(error) =
            super::access::check(&record.source_path, crate::policy::access::Access::Write)
        {
            return super::listing::helper_error_from_tealdrive(request, error);
        }
        if let Err(error) = super::cancellation::check() {
            return super::listing::helper_error_from_io(request, error);
        }
        let rename = || super::transaction::rename(root, current, &record.source_path, false);
        let result = if record.action == "trash" {
            let parent = record
                .source_path
                .0
                .rsplit_once('/')
                .map_or("", |(parent, _)| parent);
            super::access::with_alias(
                crate::config::RelativePath::new(".tealdrive-trash/files"),
                crate::config::RelativePath::new(parent),
                rename,
            )
        } else {
            rename()
        };
        match result {
            Ok(()) => {
                let mut response = HelperResponse::operation_done(request, "Operation undone.");
                if record.action == "trash" {
                    let cleanup = || -> std::io::Result<()> {
                        use std::os::fd::AsRawFd;
                        let id = current
                            .0
                            .rsplit('/')
                            .next()
                            .ok_or_else(|| std::io::Error::other("missing trash id"))?;
                        uuid::Uuid::parse_str(id).map_err(std::io::Error::other)?;
                        let directory = super::sandbox::open(
                            root,
                            &crate::config::RelativePath::new(".tealdrive-trash/info"),
                            libc::O_PATH | libc::O_DIRECTORY,
                            0,
                        )?;
                        let name = std::ffi::CString::new(format!("{id}.msgpack"))
                            .map_err(std::io::Error::other)?;
                        if unsafe { libc::unlinkat(directory.as_raw_fd(), name.as_ptr(), 0) } != 0 {
                            return Err(std::io::Error::last_os_error());
                        }
                        Ok(())
                    };
                    let parent = record
                        .source_path
                        .0
                        .rsplit_once('/')
                        .map_or("", |(parent, _)| parent);
                    if super::access::with_alias(
                        crate::config::RelativePath::new(".tealdrive-trash/info"),
                        crate::config::RelativePath::new(parent),
                        cleanup,
                    )
                    .is_err()
                    {
                        response.safe_message = Some(
                            "Operation undone; trash metadata cleanup remains pending.".into(),
                        );
                    }
                }
                response
            }
            Err(error) => super::listing::helper_error_from_io(request, error),
        }
    };
    if record.action == "trash" {
        super::access::with_alias(current.clone(), record.source_path.clone(), run)
    } else {
        run()
    }
}
