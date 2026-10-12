use std::io::{Read, Write};

use crate::errors::TealDriveError;
use crate::helper::archive::{
    compress_targz_with_roots, compress_zip_with_roots, extract_archive_with_roots,
};
use crate::helper::chmod::chmod_file_with_roots;
use crate::helper::create_folder::create_folder_with_roots;
use crate::helper::download::download_file_with_roots;
use crate::helper::listing::{allowed_roots_from_env, list_directory_with_roots};
use crate::helper::metadata::file_metadata_with_roots;
use crate::helper::move_copy::{copy_file_with_roots, move_file_with_roots};
use crate::helper::rename::rename_with_roots;
use crate::helper::text::read_text_file_with_roots;
use crate::helper::trash::{
    delete_permanently_with_roots, move_file_to_trash_with_roots,
    restore_file_from_trash_with_roots,
};
use crate::helper::types::{command_disabled_in_v1, HelperCommand, HelperRequest, HelperResponse};
use crate::helper::upload::write_file_with_roots;

pub const MAX_HELPER_REQUEST_SIZE: usize = 8 * 1024 * 1024;
pub const MAX_HELPER_RESPONSE_SIZE: usize = 8 * 1024 * 1024;
pub const HELPER_LEN_PREFIX_SIZE: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelperIpc;

pub fn encode_helper_request(request: &HelperRequest) -> Result<Vec<u8>, TealDriveError> {
    encode_length_prefixed(
        request,
        MAX_HELPER_REQUEST_SIZE,
        TealDriveError::HelperRequestTooLarge,
    )
}

pub fn decode_helper_request(bytes: &[u8]) -> Result<HelperRequest, TealDriveError> {
    decode_length_prefixed(
        bytes,
        MAX_HELPER_REQUEST_SIZE,
        TealDriveError::HelperRequestTooLarge,
    )
    .map_err(helper_decode_error)
}

pub fn encode_helper_response(response: &HelperResponse) -> Result<Vec<u8>, TealDriveError> {
    encode_length_prefixed(
        response,
        MAX_HELPER_RESPONSE_SIZE,
        TealDriveError::HelperResponseTooLarge,
    )
}

pub fn decode_helper_response(bytes: &[u8]) -> Result<HelperResponse, TealDriveError> {
    decode_length_prefixed(
        bytes,
        MAX_HELPER_RESPONSE_SIZE,
        TealDriveError::HelperResponseTooLarge,
    )
    .map_err(helper_decode_error)
}

pub fn read_helper_request<R: Read>(reader: &mut R) -> Result<HelperRequest, TealDriveError> {
    read_length_prefixed(
        reader,
        MAX_HELPER_REQUEST_SIZE,
        TealDriveError::HelperRequestTooLarge,
    )
    .and_then(|bytes| rmp_serde::from_slice(&bytes).map_err(|_| TealDriveError::HelperDecodeFailed))
}

pub fn write_helper_response<W: Write>(
    writer: &mut W,
    response: &HelperResponse,
) -> Result<(), TealDriveError> {
    let bytes = encode_helper_response(response)?;
    writer
        .write_all(&bytes)
        .map_err(|_| TealDriveError::HelperStdoutFailed)
}

pub fn read_helper_response<R: Read>(reader: &mut R) -> Result<HelperResponse, TealDriveError> {
    read_length_prefixed(
        reader,
        MAX_HELPER_RESPONSE_SIZE,
        TealDriveError::HelperResponseTooLarge,
    )
    .and_then(|bytes| rmp_serde::from_slice(&bytes).map_err(|_| TealDriveError::HelperDecodeFailed))
}

pub fn run_helper_once<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
) -> Result<(), TealDriveError> {
    let request = read_helper_request(reader)?;
    let response = handle_helper_request(&request);
    write_helper_response(writer, &response)
}

pub fn handle_helper_request(request: &HelperRequest) -> HelperResponse {
    let config = crate::config::AppConfig {
        roots: allowed_roots_from_env(),
        features: crate::policy::feature_gate::FeatureFlags {
            save_text_file: true,
            chmod_file: true,
            extract_archive: true,
            delete_permanently: true,
            chown: false,
        },
        ..Default::default()
    };
    handle_helper_request_with_config(request, &config)
}
pub fn handle_helper_request_with_config(
    request: &HelperRequest,
    config: &crate::config::AppConfig,
) -> HelperResponse {
    if request.command == HelperCommand::DeletePermanently
        && crate::helper::trash::internal_trash_id(&request.relative_path).is_some()
    {
        let Some(root) = config.roots.iter().find(|r| r.root_id == request.root_id) else {
            return HelperResponse::permission_denied(request, "Unknown root.");
        };
        let original = match crate::helper::trash::policy_original_path(request, root) {
            Ok(path) => path,
            Err(error) => {
                return crate::helper::listing::helper_error_from_tealdrive(request, error)
            }
        };
        return crate::helper::access::with_context(request, config, || {
            crate::helper::access::with_alias(request.relative_path.clone(), original, || {
                handle_helper_operation(request, config)
            })
        });
    }
    crate::helper::access::with_context(request, config, || {
        handle_helper_operation(request, config)
    })
}
fn handle_helper_operation(
    request: &HelperRequest,
    config: &crate::config::AppConfig,
) -> HelperResponse {
    if request.uid == 0 {
        return HelperResponse::permission_denied(request, "Helper refuses uid 0.");
    }
    if let Err(error) = crate::helper::access::authorize(request, config)
        .and_then(|_| crate::helper::access::preflight(request, config))
    {
        return crate::helper::listing::helper_error_from_tealdrive(request, error);
    }
    if crate::helper::policy::authorize(request, config).is_err() {
        return HelperResponse::permission_denied(request, "Operation denied by helper policy.");
    }
    let disabled = match request.command {
        HelperCommand::WriteFile => !config.features.save_text_file,
        HelperCommand::Chmod => !config.features.chmod_file,
        HelperCommand::ExtractArchive => !config.features.extract_archive,
        HelperCommand::DeletePermanently => !config.features.delete_permanently,
        _ => false,
    };
    if disabled {
        return HelperResponse::feature_disabled(request);
    }
    if command_disabled_in_v1(request.command) {
        return HelperResponse::feature_disabled(request);
    }
    if request.command == HelperCommand::QuotaUsage {
        return crate::helper::usage::measure(request, config);
    }
    if request.command == HelperCommand::ListDirectory {
        let roots = &config.roots;
        return list_directory_with_roots(request, roots, &config.sensitive_policy);
    }
    if request.command == HelperCommand::FileMetadata {
        let roots = &config.roots;
        return file_metadata_with_roots(request, roots, &config.sensitive_policy);
    }
    if request.command == HelperCommand::ReadFileRange {
        return crate::helper::download::read_file_range(
            request,
            &config.roots,
            &config.sensitive_policy,
        );
    }
    if request.command == HelperCommand::DownloadFile {
        let roots = &config.roots;
        return download_file_with_roots(request, roots, &config.sensitive_policy);
    }
    if request.command == HelperCommand::ReadTextFile {
        let roots = &config.roots;
        return read_text_file_with_roots(request, roots, &config.sensitive_policy);
    }
    if request.command == HelperCommand::CreateFolder {
        let roots = &config.roots;
        return create_folder_with_roots(request, roots, &config.sensitive_policy);
    }
    if request.command == HelperCommand::Rename {
        let roots = &config.roots;
        return rename_with_roots(request, roots, &config.sensitive_policy);
    }
    if request.command == HelperCommand::Move {
        let roots = &config.roots;
        return move_file_with_roots(request, roots, &config.sensitive_policy);
    }
    if request.command == HelperCommand::Copy {
        let roots = &config.roots;
        return copy_file_with_roots(request, roots, &config.sensitive_policy);
    }
    if request.command == HelperCommand::MoveToTrash {
        let roots = &config.roots;
        return move_file_to_trash_with_roots(request, roots, &config.sensitive_policy);
    }
    if request.command == HelperCommand::RestoreFromTrash {
        let roots = &config.roots;
        return restore_file_from_trash_with_roots(request, roots, &config.sensitive_policy);
    }
    if matches!(
        request.command,
        HelperCommand::WriteFile | HelperCommand::UploadCommit
    ) {
        let roots = &config.roots;
        return write_file_with_roots(request, roots, &config.sensitive_policy);
    }
    if request.command == HelperCommand::DeletePermanently {
        let roots = &config.roots;
        return delete_permanently_with_roots(request, roots, &config.sensitive_policy);
    }
    if request.command == HelperCommand::Chmod {
        let roots = &config.roots;
        return chmod_file_with_roots(request, roots, &config.sensitive_policy);
    }
    if request.command == HelperCommand::CompressZip {
        let roots = &config.roots;
        return compress_zip_with_roots(request, roots, &config.sensitive_policy);
    }
    if request.command == HelperCommand::CompressTarGz {
        let roots = &config.roots;
        return compress_targz_with_roots(request, roots, &config.sensitive_policy);
    }
    if request.command == HelperCommand::ExtractArchive {
        let roots = &config.roots;
        return extract_archive_with_roots(request, roots, &config.sensitive_policy);
    }
    if request.command == HelperCommand::SearchFiles {
        let roots = &config.roots;
        return crate::helper::search::search_files_with_roots(
            request,
            roots,
            &config.sensitive_policy,
        );
    }
    if request.command == HelperCommand::ListTrash {
        let roots = &config.roots;
        return crate::helper::trash::list_trash_with_roots(
            request,
            roots,
            &config.sensitive_policy,
        );
    }
    HelperResponse::not_implemented(request)
}

fn encode_length_prefixed<T: serde::Serialize>(
    value: &T,
    max_size: usize,
    too_large_error: TealDriveError,
) -> Result<Vec<u8>, TealDriveError> {
    let payload = zeroize::Zeroizing::new(
        rmp_serde::to_vec_named(value).map_err(|_| TealDriveError::HelperEncodeFailed)?,
    );
    if payload.is_empty() {
        return Err(TealDriveError::HelperEncodeFailed);
    }
    if payload.len() > max_size {
        return Err(too_large_error);
    }
    let len = u32::try_from(payload.len()).map_err(|_| too_large_error)?;
    let mut frame = Vec::with_capacity(HELPER_LEN_PREFIX_SIZE + payload.len());
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

fn decode_length_prefixed<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    max_size: usize,
    too_large_error: TealDriveError,
) -> Result<T, TealDriveError> {
    if bytes.len() < HELPER_LEN_PREFIX_SIZE {
        return Err(TealDriveError::HelperMalformedResponse);
    }
    let len = u32::from_be_bytes(bytes[0..4].try_into().expect("fixed length")) as usize;
    if len == 0 {
        return Err(TealDriveError::HelperMalformedResponse);
    }
    if len > max_size {
        return Err(too_large_error);
    }
    if bytes.len() - HELPER_LEN_PREFIX_SIZE != len {
        return Err(TealDriveError::HelperMalformedResponse);
    }
    rmp_serde::from_slice(&bytes[4..]).map_err(|_| TealDriveError::HelperDecodeFailed)
}

fn read_length_prefixed<R: Read>(
    reader: &mut R,
    max_size: usize,
    too_large_error: TealDriveError,
) -> Result<Vec<u8>, TealDriveError> {
    let mut len_bytes = [0_u8; HELPER_LEN_PREFIX_SIZE];
    reader
        .read_exact(&mut len_bytes)
        .map_err(|_| TealDriveError::HelperMalformedResponse)?;
    let len = u32::from_be_bytes(len_bytes) as usize;
    if len == 0 {
        return Err(TealDriveError::HelperMalformedResponse);
    }
    if len > max_size {
        return Err(too_large_error);
    }
    let mut payload = vec![0_u8; len];
    reader
        .read_exact(&mut payload)
        .map_err(|_| TealDriveError::HelperMalformedResponse)?;
    Ok(payload)
}

fn helper_decode_error(error: TealDriveError) -> TealDriveError {
    match error {
        TealDriveError::HelperRequestTooLarge => TealDriveError::HelperRequestTooLarge,
        TealDriveError::HelperResponseTooLarge => TealDriveError::HelperResponseTooLarge,
        TealDriveError::HelperMalformedResponse => TealDriveError::HelperMalformedResponse,
        _ => TealDriveError::HelperDecodeFailed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::ErrorKind;
    use crate::helper::ipc::tests_support::sample_request;
    use crate::helper::types::{HelperCommand, HelperResponse};

    #[test]
    fn helper_request_length_prefixed_roundtrip() {
        let request = sample_request(HelperCommand::ListDirectory);
        let encoded = encode_helper_request(&request).expect("encoded");
        let decoded = decode_helper_request(&encoded).expect("decoded");

        assert_eq!(decoded, request);
        assert_eq!(
            u32::from_be_bytes(encoded[0..4].try_into().unwrap()) as usize,
            encoded.len() - 4
        );
    }

    #[test]
    fn helper_response_length_prefixed_roundtrip() {
        let request = sample_request(HelperCommand::ListDirectory);
        let response = HelperResponse::not_implemented(&request);
        let encoded = encode_helper_response(&response).expect("encoded");
        let decoded = decode_helper_response(&encoded).expect("decoded");

        assert_eq!(decoded, response);
    }

    #[test]
    fn truncated_request_rejected() {
        assert_eq!(
            decode_helper_request(&[0, 0, 0, 10, 1, 2]),
            Err(TealDriveError::HelperMalformedResponse)
        );
    }

    #[test]
    fn oversized_request_rejected() {
        let len = (MAX_HELPER_REQUEST_SIZE as u32) + 1;
        assert_eq!(
            decode_helper_request(&len.to_be_bytes()),
            Err(TealDriveError::HelperRequestTooLarge)
        );
    }

    #[test]
    fn malformed_msgpack_rejected() {
        let mut frame = Vec::new();
        frame.extend_from_slice(&1_u32.to_be_bytes());
        frame.push(0xc1);

        assert_eq!(
            decode_helper_request(&frame),
            Err(TealDriveError::HelperDecodeFailed)
        );
    }

    #[test]
    fn big_endian_length_parsing() {
        let request = sample_request(HelperCommand::ReadFile);
        let encoded = encode_helper_request(&request).expect("encoded");
        let parsed = u32::from_be_bytes(encoded[0..4].try_into().unwrap()) as usize;

        assert_eq!(parsed, encoded.len() - 4);
    }

    #[test]
    fn uid_zero_request_rejected() {
        let mut request = sample_request(HelperCommand::ListDirectory);
        request.uid = 0;
        let response = handle_helper_request(&request);

        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::PermissionDenied));
    }

    #[test]
    fn unsupported_command_returns_not_implemented() {
        let request = sample_request(HelperCommand::ReadFile);
        let response = handle_helper_request(&request);

        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::NotImplemented));
    }

    #[test]
    fn disabled_command_returns_feature_disabled() {
        let request = sample_request(HelperCommand::ReadFile);
        let response = HelperResponse::feature_disabled(&request);

        assert!(!response.success);
        assert_eq!(response.error_kind, Some(ErrorKind::FeatureDisabled));
    }

    #[test]
    fn run_helper_once_writes_one_response() {
        let request = sample_request(HelperCommand::ReadFile);
        let input = encode_helper_request(&request).expect("request");
        let mut output = Vec::new();

        run_helper_once(&mut input.as_slice(), &mut output).expect("run");
        let response = decode_helper_response(&output).expect("response");

        assert_eq!(response.error_kind, Some(ErrorKind::NotImplemented));
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    use crate::config::{RelativePath, RootId};
    use crate::helper::types::{
        HelperCommand, HelperLimits, HelperListDirectoryOptions, HelperPolicyContext, HelperRequest,
    };
    use crate::protocol::frame::RequestId;
    use crate::protocol::schema::{FileFilter, FilterMode, SortMode};

    pub fn sample_request(command: HelperCommand) -> HelperRequest {
        HelperRequest {
            request_id: RequestId::new(),
            username: "alice".to_owned(),
            uid: 1000,
            gid: 1000,
            command,
            root_id: RootId::new("home"),
            relative_path: RelativePath::new("docs"),
            target_relative_path: None,
            policy_context: HelperPolicyContext {
                read_only_root: false,
                is_web_root: false,
            },
            limits: HelperLimits {
                max_directory_page_size: 500,
                max_text_edit_size: 5 * 1024 * 1024,
                max_chunk_size: crate::limits::MAX_CHUNK_SIZE,
                max_download_helper_bytes: crate::download::transfer::MAX_DOWNLOAD_HELPER_BYTES,
            },
            list_options: Some(HelperListDirectoryOptions {
                cursor: None,
                limit: 200,
                include_hidden: false,
                sort: SortMode::NameAsc,
                filter: FilterMode {
                    extensions: None,
                    query: None,
                    file_kind: None,
                    filter: Some(FileFilter::All),
                },
            }),
            write_options: None,
            chmod_mode: None,
            archive_source_paths: None,
            search_options: None,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum HelperInvocation {
    Account(crate::helper::account_gateway::AccountCommand),
    Authenticate(crate::auth::account::AuthRequest),
    Operation {
        #[serde(default)]
        grant: Option<crate::accounts::grants::SessionGrant>,
        request: Box<HelperRequest>,
        ticket: Option<crate::helper::security::IdentityTicket>,
        policy: Option<Box<crate::helper::policy::PolicySnapshot>>,
    },
}
#[derive(serde::Serialize, serde::Deserialize)]
pub struct HelperAuthentication {
    pub result: Result<
        (
            crate::auth::account::AuthSuccess,
            crate::helper::security::IdentityTicket,
        ),
        crate::auth::account::AuthFailure,
    >,
}
pub fn encode_invocation(value: &HelperInvocation) -> Result<Vec<u8>, TealDriveError> {
    encode_length_prefixed(
        value,
        MAX_HELPER_REQUEST_SIZE,
        TealDriveError::HelperRequestTooLarge,
    )
}
pub fn read_authentication<R: Read>(
    reader: &mut R,
) -> Result<HelperAuthentication, TealDriveError> {
    let bytes = read_length_prefixed(
        reader,
        MAX_HELPER_RESPONSE_SIZE,
        TealDriveError::HelperResponseTooLarge,
    )?;
    rmp_serde::from_slice(&bytes).map_err(|_| TealDriveError::HelperDecodeFailed)
}
pub fn run_secure_helper<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
) -> Result<(), TealDriveError> {
    let bytes = zeroize::Zeroizing::new(read_length_prefixed(
        reader,
        MAX_HELPER_REQUEST_SIZE,
        TealDriveError::HelperRequestTooLarge,
    )?);
    let invocation: HelperInvocation =
        rmp_serde::from_slice(&bytes).map_err(|_| TealDriveError::HelperDecodeFailed)?;
    match invocation {
        HelperInvocation::Account(command) => {
            let response = crate::helper::account_gateway::execute(command);
            let bytes = zeroize::Zeroizing::new(encode_length_prefixed(
                &response,
                MAX_HELPER_RESPONSE_SIZE,
                TealDriveError::HelperResponseTooLarge,
            )?);
            writer
                .write_all(&bytes)
                .map_err(|_| TealDriveError::HelperStdoutFailed)?;
            Ok(())
        }
        HelperInvocation::Authenticate(request) => {
            let response = HelperAuthentication {
                result: crate::helper::security::authenticate(&request)
                    .map_err(|e| e.failure_kind()),
            };
            let bytes = encode_length_prefixed(
                &response,
                MAX_HELPER_RESPONSE_SIZE,
                TealDriveError::HelperResponseTooLarge,
            )?;
            writer
                .write_all(&bytes)
                .map_err(|_| TealDriveError::HelperStdoutFailed)
        }
        HelperInvocation::Operation {
            grant,
            mut request,
            ticket,
            policy,
        } => {
            let elevated = unsafe { libc::geteuid() != libc::getuid() };
            let user = crate::config::UserContext {
                username: request.username.clone(),
                uid: request.uid,
                gid: request.gid,
            };
            let mut authenticated_account = None;
            let mut authenticated_grant = None;
            let config = if elevated {
                let mut config = crate::helper::security::trusted_config()
                    .map_err(|_| TealDriveError::PermissionDenied)?;
                if config.web_accounts.enabled {
                    let Some(grant) = grant else {
                        return write_helper_response(
                            writer,
                            &HelperResponse::permission_denied(
                                &request,
                                "Session authorization required.",
                            ),
                        );
                    };
                    authenticated_grant = Some(grant.clone());
                    let reply = crate::helper::account_gateway::execute(
                        crate::helper::account_gateway::AccountCommand::Validate(grant),
                    );
                    let Ok(crate::helper::account_gateway::AccountReply::Validated(account)) =
                        reply
                    else {
                        return write_helper_response(
                            writer,
                            &HelperResponse::permission_denied(
                                &request,
                                "Session authorization expired or revoked.",
                            ),
                        );
                    };
                    config = crate::policy::home::resolve(
                        &config,
                        &account,
                        config.home_template.as_ref(),
                    )?;
                    crate::helper::account_gateway::apply_persisted_policy(&mut config, &account)
                        .map_err(|_| TealDriveError::PermissionDenied)?;
                    if account.username != user.username
                        || account.uid != user.uid
                        || account.gid != user.gid
                        || !config
                            .roots
                            .iter()
                            .filter(|root| root.root_id == request.root_id)
                            .any(|root| root.base_path == account.home)
                    {
                        return write_helper_response(
                            writer,
                            &HelperResponse::permission_denied(
                                &request,
                                "Root is outside the authenticated home.",
                            ),
                        );
                    }
                    if account.kind == crate::accounts::store::AccountKind::Virtual {
                        let quota = account.quota_bytes.unwrap_or(0);
                        for root in &mut config.roots {
                            root.quota_bytes =
                                Some(root.quota_bytes.map_or(quota, |ceiling| ceiling.min(quota)));
                        }
                    }
                    authenticated_account = Some(account);
                } else {
                    if grant.is_some() {
                        return Err(TealDriveError::PermissionDenied);
                    }
                    let ticket = ticket.ok_or(TealDriveError::PermissionDenied)?;
                    crate::helper::security::verify(&ticket, &user)
                        .map_err(|_| TealDriveError::PermissionDenied)?;
                }
                config
            } else {
                crate::config::AppConfig {
                    roots: allowed_roots_from_env(),
                    ..Default::default()
                }
            };
            let config = match policy.as_ref() {
                Some(snapshot) if elevated => crate::helper::policy::restrict(snapshot, &config)?,
                Some(snapshot) => {
                    snapshot.validate()?;
                    snapshot.config.clone()
                }
                None if elevated => return Err(TealDriveError::PermissionDenied),
                None => config,
            };
            let mut config = match authenticated_account.as_ref() {
                Some(account) => crate::policy::home::resolve(&config, account, None)?,
                None => config,
            };
            request.limits.max_chunk_size = request
                .limits
                .max_chunk_size
                .min(config.limits.max_chunk_size);
            request.limits.max_text_edit_size = request
                .limits
                .max_text_edit_size
                .min(config.limits.max_text_edit_size);
            request.limits.max_directory_page_size = request
                .limits
                .max_directory_page_size
                .min(config.limits.max_directory_page_size);
            let Some(root) = config.roots.iter().find(|r| r.root_id == request.root_id) else {
                return write_helper_response(
                    writer,
                    &HelperResponse::permission_denied(&request, "Unknown root."),
                );
            };
            // Validate first; privileged accounting never reads an untrusted root.
            if let Err(error) = crate::helper::access::authorize(&request, &config) {
                return write_helper_response(
                    writer,
                    &crate::helper::listing::helper_error_from_tealdrive(&request, error),
                );
            }
            if crate::helper::policy::authorize(&request, &config).is_err() {
                return write_helper_response(
                    writer,
                    &HelperResponse::permission_denied(
                        &request,
                        "Operation denied by helper policy.",
                    ),
                );
            }
            let mut reservation = if matches!(
                request.command,
                HelperCommand::QuotaUsage
                    | HelperCommand::ListDirectory
                    | HelperCommand::FileMetadata
                    | HelperCommand::ReadTextFile
                    | HelperCommand::ReadFileRange
                    | HelperCommand::DownloadFile
                    | HelperCommand::ListTrash
                    | HelperCommand::SearchFiles
            ) {
                None
            } else {
                Some(
                    crate::helper::quota::Reservation::acquire(root)
                        .map_err(|_| TealDriveError::PermissionDenied)?,
                )
            };
            if let (Some(held), Some(grant)) = (reservation.as_mut(), authenticated_grant.as_ref())
            {
                // A quota/policy transition may have happened while flock waited.
                // Recheck authority and ceilings inside the same directory lock.
                let reply = crate::helper::account_gateway::execute(
                    crate::helper::account_gateway::AccountCommand::Validate(grant.clone()),
                );
                let Ok(crate::helper::account_gateway::AccountReply::Validated(account)) = reply
                else {
                    return Err(TealDriveError::PermissionDenied);
                };
                let trusted = crate::helper::security::trusted_config()
                    .map_err(|_| TealDriveError::PermissionDenied)?;
                let mut trusted = crate::policy::home::resolve(
                    &trusted,
                    &account,
                    trusted.home_template.as_ref(),
                )?;
                crate::helper::account_gateway::apply_persisted_policy(&mut trusted, &account)
                    .map_err(|_| TealDriveError::PermissionDenied)?;
                config = crate::helper::policy::restrict(
                    policy.as_ref().ok_or(TealDriveError::PermissionDenied)?,
                    &trusted,
                )?;
                crate::helper::policy::authorize(&request, &config)?;
                let root = config
                    .roots
                    .iter()
                    .find(|root| root.root_id == request.root_id)
                    .ok_or(TealDriveError::PermissionDenied)?;
                let used = if account.kind == crate::accounts::store::AccountKind::Virtual
                    && config.virtual_accounts.enforcement
                        == crate::accounts::virtuals::Enforcement::UserQuota
                {
                    crate::helper::native_quota::assert_limit(
                        root,
                        account.uid,
                        trusted
                            .roots
                            .iter()
                            .find(|root| root.root_id == request.root_id)
                            .and_then(|root| root.quota_bytes)
                            .unwrap_or(0),
                        config.virtual_accounts.max_inodes,
                    )
                    .map_err(|_| TealDriveError::PermissionDenied)?
                    .used_bytes
                } else {
                    held.used_bytes
                };
                held.available = root.quota_bytes.map(|limit| limit.saturating_sub(used));
            } else if let Some(account) = authenticated_account.as_ref() {
                if account.kind == crate::accounts::store::AccountKind::Virtual
                    && config.virtual_accounts.enforcement
                        == crate::accounts::virtuals::Enforcement::UserQuota
                {
                    let trusted = crate::helper::security::trusted_config()
                        .map_err(|_| TealDriveError::PermissionDenied)?;
                    let trusted = crate::policy::home::resolve(
                        &trusted,
                        account,
                        trusted.home_template.as_ref(),
                    )?;
                    let root = trusted
                        .roots
                        .iter()
                        .find(|root| root.root_id == request.root_id)
                        .ok_or(TealDriveError::PermissionDenied)?;
                    crate::helper::native_quota::assert_limit(
                        root,
                        account.uid,
                        root.quota_bytes.unwrap_or(0),
                        config.virtual_accounts.max_inodes,
                    )
                    .map_err(|_| TealDriveError::PermissionDenied)?;
                }
            }
            if crate::helper::security::enter_identity(&user).is_err() {
                return write_helper_response(
                    writer,
                    &HelperResponse::permission_denied(&request, "Identity verification failed."),
                );
            }
            let Some(root) = config.roots.iter().find(|r| r.root_id == request.root_id) else {
                return write_helper_response(
                    writer,
                    &HelperResponse::permission_denied(&request, "Unknown root."),
                );
            };
            if crate::helper::sandbox::confine(root).is_err() {
                return write_helper_response(
                    writer,
                    &HelperResponse::permission_denied(
                        &request,
                        "Kernel filesystem confinement is unavailable.",
                    ),
                );
            }
            let mut response = crate::helper::quota::with_budget(
                reservation.as_ref().and_then(|r| r.available),
                || crate::helper::transaction::execute(&request, &config),
            );
            response.policy_revision = policy.map(|p| p.revision);
            write_helper_response(writer, &response)
        }
    }
}

pub fn read_wire_payload<R: Read>(reader: &mut R) -> Result<Vec<u8>, TealDriveError> {
    read_length_prefixed(
        reader,
        MAX_HELPER_RESPONSE_SIZE,
        TealDriveError::HelperResponseTooLarge,
    )
}
