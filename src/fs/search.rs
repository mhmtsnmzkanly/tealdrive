use crate::config::{AppConfig, UserContext};
use crate::errors::{ErrorKind, TealDriveError};
use crate::fs::path::validate_relative_path;
use crate::helper::client::HelperProcessClient;
use crate::helper::types::{
    HelperCommand, HelperLimits, HelperPolicyContext, HelperRequest, HelperResponse,
    HelperResponsePayload,
};
use crate::policy::decision::{FileOperation, PolicyDecision, PolicyEngine};
use crate::policy::roots::require_allowed_root;
use crate::protocol::frame::RequestId;
use crate::protocol::payload::{SearchFilesRequest, SearchResultsPayload};

pub trait SearchFilesHelper {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError>;
}

impl SearchFilesHelper for HelperProcessClient {
    fn execute_helper(&self, request: &HelperRequest) -> Result<HelperResponse, TealDriveError> {
        self.execute(request)
    }
}

pub fn search_files_request<H: SearchFilesHelper>(
    user_context: &UserContext,
    request: SearchFilesRequest,
    config: &AppConfig,
    policy_engine: &PolicyEngine<'_>,
    helper_client: &H,
) -> Result<SearchResultsPayload, TealDriveError> {
    let base_relative_path = validate_relative_path(&request.base_path)?;
    let root = require_allowed_root(config.allowed_roots(), &request.root_id)?;

    match policy_engine.check_operation(
        FileOperation::ListDirectory,
        &request.root_id,
        &base_relative_path,
        None,
        None,
        root.hidden_files_allowed,
    )? {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(_) => return Err(TealDriveError::PolicyDenied),
    }

    let helper_request = HelperRequest {
        request_id: RequestId::new(),
        username: user_context.username.clone(),
        uid: user_context.uid,
        gid: user_context.gid,
        command: HelperCommand::SearchFiles,
        root_id: request.root_id.clone(),
        relative_path: base_relative_path,
        target_relative_path: None,
        policy_context: HelperPolicyContext {
            read_only_root: root.read_only,
            is_web_root: root.is_web_root,
        },
        limits: HelperLimits {
            max_directory_page_size: config.limits.max_directory_page_size,
            max_text_edit_size: config.limits.max_text_edit_size,
            max_chunk_size: config.limits.max_chunk_size,
            max_download_helper_bytes: 10 * 1024 * 1024,
        },
        list_options: None,
        write_options: None,
        chmod_mode: None,
        archive_source_paths: None,
        search_options: Some(crate::helper::types::HelperSearchOptions {
            query: request.query,
            extension: request.extension,
            max_results: request.max_results,
            include_hidden: request.include_hidden,
        }),
    };

    let response = helper_client.execute_helper(&helper_request)?;
    if !response.success {
        return match response.error_kind {
            Some(ErrorKind::PermissionDenied) => Err(TealDriveError::PermissionDenied),
            Some(ErrorKind::PolicyDenied) => Err(TealDriveError::PolicyDenied),
            Some(ErrorKind::NotFound) => Err(TealDriveError::NotFound),
            Some(ErrorKind::InvalidPath) => Err(TealDriveError::InvalidPath),
            _ => Err(TealDriveError::Internal),
        };
    }

    match response.payload {
        HelperResponsePayload::SearchResults(payload) => Ok(payload),
        _ => Err(TealDriveError::HelperMalformedResponse),
    }
}
