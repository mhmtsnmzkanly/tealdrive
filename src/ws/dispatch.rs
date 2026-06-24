use crate::config::AppConfig;
use crate::download::transfer::{download_file_request, DownloadHelper};
use crate::errors::{ErrorKind, ErrorPayload, TealDriveError};
use crate::fs::create_folder::{create_folder_request, CreateFolderHelper};
use crate::fs::listing::{list_directory_request, DirectoryListingHelper};
use crate::fs::metadata::{file_metadata_request, MetadataHelper};
use crate::fs::move_copy::{copy_file_request, move_file_request, CopyHelper, MoveHelper};
use crate::fs::rename::{rename_file_request, RenameHelper};
use crate::fs::text::{read_text_file_request, TextPreviewHelper};
use crate::fs::trash::{
    move_to_trash_request, restore_from_trash_request, MoveToTrashHelper, RestoreFromTrashHelper,
};
use crate::policy::decision::PolicyEngine;
use crate::policy::feature_gate::{FeatureGate, FeatureGateResult};
use crate::protocol::codec::{decode_msgpack, encode_msgpack};
use crate::protocol::frame::{RequestId, TdrvFrame};
use crate::protocol::header::{Encoding, TdrvHeader};
use crate::protocol::message_type::MessageType;
use crate::protocol::payload::{
    ClientHelloPayload, CopyFileRequest, CreateFolderRequest, DownloadBeginRequest,
    FileMetadataRequest, ListDirectoryRequest, MoveFileRequest, MoveToTrashRequest,
    OperationFailedPayload, ReadTextFileRequest, RenameFileRequest, RestoreFromTrashRequest,
};
use crate::ws::connection::{classify_message, InboundFrameKind, WsConnectionContext};
use crate::ws::handshake::{
    apply_protocol_ready, create_protocol_ready_frame, decode_client_hello, validate_client_hello,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchDecision {
    Outbound(Vec<TdrvFrame>),
    DispatchPlaceholder(TdrvFrame),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchSkeleton;

pub fn handle_inbound_frame(
    context: &mut WsConnectionContext,
    bytes: &[u8],
) -> Result<DispatchDecision, TealDriveError> {
    let frame = TdrvFrame::decode(bytes)?;
    frame.header.message_type.validate_inbound_client()?;

    match classify_message(frame.header.message_type) {
        InboundFrameKind::Handshake(MessageType::ClientHello) => {
            handle_client_hello(context, &frame)
        }
        InboundFrameKind::Handshake(_) => Err(TealDriveError::InvalidMessageDirection),
        InboundFrameKind::Operation(_) if !context.protocol_ready => {
            Err(TealDriveError::Validation)
        }
        InboundFrameKind::Operation(message_type) => {
            placeholder_dispatch(context, frame, message_type)
        }
        InboundFrameKind::Protocol(_) => {
            Ok(DispatchDecision::Outbound(vec![connection_status_frame(
                context,
                "connected",
            )?]))
        }
    }
}

pub fn handle_inbound_frame_with_service<H>(
    context: &mut WsConnectionContext,
    bytes: &[u8],
    config: &AppConfig,
    helper_client: &H,
) -> Result<DispatchDecision, TealDriveError>
where
    H: DirectoryListingHelper
        + MetadataHelper
        + DownloadHelper
        + TextPreviewHelper
        + CreateFolderHelper
        + RenameHelper
        + MoveHelper
        + CopyHelper
        + MoveToTrashHelper
        + RestoreFromTrashHelper,
{
    let frame = TdrvFrame::decode(bytes)?;
    handle_decoded_frame_with_service(context, frame, config, helper_client)
}

pub fn handle_decoded_frame_with_service<H>(
    context: &mut WsConnectionContext,
    frame: TdrvFrame,
    config: &AppConfig,
    helper_client: &H,
) -> Result<DispatchDecision, TealDriveError>
where
    H: DirectoryListingHelper
        + MetadataHelper
        + DownloadHelper
        + TextPreviewHelper
        + CreateFolderHelper
        + RenameHelper
        + MoveHelper
        + CopyHelper
        + MoveToTrashHelper
        + RestoreFromTrashHelper,
{
    frame.header.message_type.validate_inbound_client()?;

    match classify_message(frame.header.message_type) {
        InboundFrameKind::Handshake(MessageType::ClientHello) => {
            handle_client_hello(context, &frame)
        }
        InboundFrameKind::Handshake(_) => Err(TealDriveError::InvalidMessageDirection),
        InboundFrameKind::Operation(_) if !context.protocol_ready => {
            Err(TealDriveError::Validation)
        }
        InboundFrameKind::Operation(message_type) => {
            handle_operation_with_service(context, frame, message_type, config, helper_client)
        }
        InboundFrameKind::Protocol(_) => Err(TealDriveError::InvalidMessageDirection),
    }
}

fn handle_client_hello(
    context: &mut WsConnectionContext,
    frame: &TdrvFrame,
) -> Result<DispatchDecision, TealDriveError> {
    let payload: ClientHelloPayload = decode_client_hello(frame)?;
    let limits = context
        .selected_protocol
        .as_ref()
        .map_or_else(Default::default, |selected| selected.effective_limits);
    let selected = validate_client_hello(&payload, &limits)?;
    let ready = create_protocol_ready_frame(&selected)?;
    apply_protocol_ready(context, selected);
    Ok(DispatchDecision::Outbound(vec![ready]))
}

fn placeholder_dispatch(
    context: &WsConnectionContext,
    frame: TdrvFrame,
    message_type: MessageType,
) -> Result<DispatchDecision, TealDriveError> {
    match FeatureGate::check(message_type, &context.features) {
        FeatureGateResult::Disabled { .. } => {
            Ok(DispatchDecision::Outbound(vec![operation_failed_frame(
                frame.header.request_id,
                message_type,
                ErrorKind::FeatureDisabled,
                "This operation is disabled in TealDrive V1.",
            )?]))
        }
        FeatureGateResult::Allowed => Ok(DispatchDecision::Outbound(vec![operation_failed_frame(
            frame.header.request_id,
            message_type,
            ErrorKind::NotImplemented,
            "This operation is not implemented yet.",
        )?])),
    }
}

fn handle_operation_with_service<H>(
    context: &WsConnectionContext,
    frame: TdrvFrame,
    message_type: MessageType,
    config: &AppConfig,
    helper_client: &H,
) -> Result<DispatchDecision, TealDriveError>
where
    H: DirectoryListingHelper
        + MetadataHelper
        + DownloadHelper
        + TextPreviewHelper
        + CreateFolderHelper
        + RenameHelper
        + MoveHelper
        + CopyHelper
        + MoveToTrashHelper
        + RestoreFromTrashHelper,
{
    match message_type {
        MessageType::ListDirectory => Ok(DispatchDecision::Outbound(vec![
            handle_list_directory_frame_with_service(context, &frame, config, helper_client)?,
        ])),
        MessageType::FileMetadataRequest => Ok(DispatchDecision::Outbound(vec![
            handle_file_metadata_frame_with_service(context, &frame, config, helper_client)?,
        ])),
        MessageType::DownloadBegin => Ok(DispatchDecision::Outbound(
            handle_download_begin_frame_with_service(context, &frame, config, helper_client)?,
        )),
        MessageType::ReadTextFile => Ok(DispatchDecision::Outbound(vec![
            handle_read_text_file_frame_with_service(context, &frame, config, helper_client)?,
        ])),
        MessageType::CreateFolder => Ok(DispatchDecision::Outbound(vec![
            handle_create_folder_frame_with_service(context, &frame, config, helper_client)?,
        ])),
        MessageType::RenameFile => Ok(DispatchDecision::Outbound(vec![
            handle_rename_file_frame_with_service(context, &frame, config, helper_client)?,
        ])),
        MessageType::MoveFile => Ok(DispatchDecision::Outbound(vec![
            handle_move_file_frame_with_service(context, &frame, config, helper_client)?,
        ])),
        MessageType::CopyFile => Ok(DispatchDecision::Outbound(vec![
            handle_copy_file_frame_with_service(context, &frame, config, helper_client)?,
        ])),
        MessageType::MoveToTrash => Ok(DispatchDecision::Outbound(vec![
            handle_move_to_trash_frame_with_service(context, &frame, config, helper_client)?,
        ])),
        MessageType::RestoreFromTrash => Ok(DispatchDecision::Outbound(vec![
            handle_restore_from_trash_frame_with_service(context, &frame, config, helper_client)?,
        ])),
        _ => placeholder_dispatch(context, frame, message_type),
    }
}

pub fn error_frame(request_id: RequestId, message: &str) -> Result<TdrvFrame, TealDriveError> {
    let payload = ErrorPayload {
        code: "PROTOCOL_ERROR".to_owned(),
        error_kind: ErrorKind::ProtocolError,
        safe_message: message.to_owned(),
        request_id,
        operation: None,
        retryable: false,
        policy_reason: None,
        debug_ref: None,
    };
    let bytes = encode_msgpack(&payload)?;
    let mut header = TdrvHeader::new(MessageType::Error, request_id, 0);
    header.encoding = Encoding::MessagePack;
    TdrvFrame::new(header, bytes)
}

pub fn operation_failed_frame(
    request_id: RequestId,
    message_type: MessageType,
    error_kind: ErrorKind,
    safe_message: &str,
) -> Result<TdrvFrame, TealDriveError> {
    let payload = OperationFailedPayload {
        request_id,
        operation: format!("{message_type:?}"),
        error: ErrorPayload {
            code: format!("{error_kind:?}").to_uppercase(),
            error_kind,
            safe_message: safe_message.to_owned(),
            request_id,
            operation: Some(format!("{message_type:?}")),
            retryable: false,
            policy_reason: None,
            debug_ref: None,
        },
    };
    let bytes = encode_msgpack(&payload)?;
    let mut header = TdrvHeader::new(MessageType::OperationFailed, request_id, 0);
    header.encoding = Encoding::MessagePack;
    TdrvFrame::new(header, bytes)
}

pub fn connection_status_frame(
    context: &WsConnectionContext,
    message: &str,
) -> Result<TdrvFrame, TealDriveError> {
    let payload = crate::protocol::payload::ConnectionStatusPayload {
        protocol_ready: context.protocol_ready,
        message: Some(message.to_owned()),
    };
    let bytes = encode_msgpack(&payload)?;
    let mut header = TdrvHeader::new(MessageType::ConnectionStatus, RequestId::new(), 0);
    header.encoding = Encoding::MessagePack;
    TdrvFrame::new(header, bytes)
}

pub fn handle_list_directory_frame_with_service<H: DirectoryListingHelper>(
    context: &WsConnectionContext,
    frame: &TdrvFrame,
    config: &AppConfig,
    helper_client: &H,
) -> Result<TdrvFrame, TealDriveError> {
    if !context.protocol_ready {
        return Err(TealDriveError::Validation);
    }
    if frame.header.message_type != MessageType::ListDirectory {
        return Err(TealDriveError::Validation);
    }
    if frame.header.encoding != Encoding::MessagePack {
        return Err(TealDriveError::InvalidEncoding);
    }

    let request: ListDirectoryRequest = decode_msgpack(&frame.payload)?;
    let policy_engine = PolicyEngine::new(config);
    let response = list_directory_request(
        &context.user_context(),
        request,
        config,
        &policy_engine,
        helper_client,
    )?;
    let bytes = encode_msgpack(&response)?;
    let mut header = TdrvHeader::new(MessageType::DirectoryList, frame.header.request_id, 0);
    header.encoding = Encoding::MessagePack;
    TdrvFrame::new(header, bytes)
}

pub fn handle_file_metadata_frame_with_service<H: MetadataHelper>(
    context: &WsConnectionContext,
    frame: &TdrvFrame,
    config: &AppConfig,
    helper_client: &H,
) -> Result<TdrvFrame, TealDriveError> {
    if !context.protocol_ready {
        return Err(TealDriveError::Validation);
    }
    if frame.header.message_type != MessageType::FileMetadataRequest {
        return Err(TealDriveError::Validation);
    }
    if frame.header.encoding != Encoding::MessagePack {
        return Err(TealDriveError::InvalidEncoding);
    }

    let request: FileMetadataRequest = decode_msgpack(&frame.payload)?;
    let policy_engine = PolicyEngine::new(config);
    let response = file_metadata_request(
        &context.user_context(),
        request,
        config,
        &policy_engine,
        helper_client,
    )?;
    let bytes = encode_msgpack(&response)?;
    let mut header = TdrvHeader::new(MessageType::FileMetadata, frame.header.request_id, 0);
    header.encoding = Encoding::MessagePack;
    TdrvFrame::new(header, bytes)
}

pub fn handle_download_begin_frame_with_service<H: DownloadHelper + MetadataHelper>(
    context: &WsConnectionContext,
    frame: &TdrvFrame,
    config: &AppConfig,
    helper_client: &H,
) -> Result<Vec<TdrvFrame>, TealDriveError> {
    if !context.protocol_ready {
        return Err(TealDriveError::Validation);
    }
    if frame.header.message_type != MessageType::DownloadBegin {
        return Err(TealDriveError::Validation);
    }
    if frame.header.encoding != Encoding::MessagePack {
        return Err(TealDriveError::InvalidEncoding);
    }

    let request: DownloadBeginRequest = decode_msgpack(&frame.payload)?;
    let policy_engine = PolicyEngine::new(config);
    Ok(download_file_request(
        &context.user_context(),
        request,
        config,
        &policy_engine,
        helper_client,
    )?
    .frames)
}

pub fn handle_read_text_file_frame_with_service<H: TextPreviewHelper + MetadataHelper>(
    context: &WsConnectionContext,
    frame: &TdrvFrame,
    config: &AppConfig,
    helper_client: &H,
) -> Result<TdrvFrame, TealDriveError> {
    if !context.protocol_ready {
        return Err(TealDriveError::Validation);
    }
    if frame.header.message_type != MessageType::ReadTextFile {
        return Err(TealDriveError::Validation);
    }
    if frame.header.encoding != Encoding::MessagePack {
        return Err(TealDriveError::InvalidEncoding);
    }

    let request: ReadTextFileRequest = decode_msgpack(&frame.payload)?;
    let policy_engine = PolicyEngine::new(config);
    let response = read_text_file_request(
        &context.user_context(),
        request,
        config,
        &policy_engine,
        helper_client,
    )?;
    let bytes = encode_msgpack(&response)?;
    let mut header = TdrvHeader::new(MessageType::TextFileContent, frame.header.request_id, 0);
    header.encoding = Encoding::MessagePack;
    TdrvFrame::new(header, bytes)
}

pub fn handle_create_folder_frame_with_service<H: CreateFolderHelper>(
    context: &WsConnectionContext,
    frame: &TdrvFrame,
    config: &AppConfig,
    helper_client: &H,
) -> Result<TdrvFrame, TealDriveError> {
    if !context.protocol_ready {
        return Err(TealDriveError::Validation);
    }
    if frame.header.message_type != MessageType::CreateFolder {
        return Err(TealDriveError::Validation);
    }
    if frame.header.encoding != Encoding::MessagePack {
        return Err(TealDriveError::InvalidEncoding);
    }

    let request: CreateFolderRequest = decode_msgpack(&frame.payload)?;
    let policy_engine = PolicyEngine::new(config);
    let response = create_folder_request(
        &context.user_context(),
        request,
        config,
        &policy_engine,
        helper_client,
    )?;
    let bytes = encode_msgpack(&response)?;
    let mut header = TdrvHeader::new(MessageType::OperationDone, frame.header.request_id, 0);
    header.encoding = Encoding::MessagePack;
    TdrvFrame::new(header, bytes)
}

pub fn handle_rename_file_frame_with_service<H: RenameHelper>(
    context: &WsConnectionContext,
    frame: &TdrvFrame,
    config: &AppConfig,
    helper_client: &H,
) -> Result<TdrvFrame, TealDriveError> {
    if !context.protocol_ready {
        return Err(TealDriveError::Validation);
    }
    if frame.header.message_type != MessageType::RenameFile {
        return Err(TealDriveError::Validation);
    }
    if frame.header.encoding != Encoding::MessagePack {
        return Err(TealDriveError::InvalidEncoding);
    }

    let request: RenameFileRequest = decode_msgpack(&frame.payload)?;
    let policy_engine = PolicyEngine::new(config);
    let response = rename_file_request(
        &context.user_context(),
        request,
        config,
        &policy_engine,
        helper_client,
    )?;
    let bytes = encode_msgpack(&response)?;
    let mut header = TdrvHeader::new(MessageType::OperationDone, frame.header.request_id, 0);
    header.encoding = Encoding::MessagePack;
    TdrvFrame::new(header, bytes)
}

pub fn handle_move_file_frame_with_service<H: MoveHelper>(
    context: &WsConnectionContext,
    frame: &TdrvFrame,
    config: &AppConfig,
    helper_client: &H,
) -> Result<TdrvFrame, TealDriveError> {
    if !context.protocol_ready {
        return Err(TealDriveError::Validation);
    }
    if frame.header.message_type != MessageType::MoveFile {
        return Err(TealDriveError::Validation);
    }
    if frame.header.encoding != Encoding::MessagePack {
        return Err(TealDriveError::InvalidEncoding);
    }

    let request: MoveFileRequest = decode_msgpack(&frame.payload)?;
    let policy_engine = PolicyEngine::new(config);
    let response = move_file_request(
        &context.user_context(),
        request,
        config,
        &policy_engine,
        helper_client,
    )?;
    let bytes = encode_msgpack(&response)?;
    let mut header = TdrvHeader::new(MessageType::OperationDone, frame.header.request_id, 0);
    header.encoding = Encoding::MessagePack;
    TdrvFrame::new(header, bytes)
}

pub fn handle_copy_file_frame_with_service<H: CopyHelper>(
    context: &WsConnectionContext,
    frame: &TdrvFrame,
    config: &AppConfig,
    helper_client: &H,
) -> Result<TdrvFrame, TealDriveError> {
    if !context.protocol_ready {
        return Err(TealDriveError::Validation);
    }
    if frame.header.message_type != MessageType::CopyFile {
        return Err(TealDriveError::Validation);
    }
    if frame.header.encoding != Encoding::MessagePack {
        return Err(TealDriveError::InvalidEncoding);
    }

    let request: CopyFileRequest = decode_msgpack(&frame.payload)?;
    let policy_engine = PolicyEngine::new(config);
    let response = copy_file_request(
        &context.user_context(),
        request,
        config,
        &policy_engine,
        helper_client,
    )?;
    let bytes = encode_msgpack(&response)?;
    let mut header = TdrvHeader::new(MessageType::OperationDone, frame.header.request_id, 0);
    header.encoding = Encoding::MessagePack;
    TdrvFrame::new(header, bytes)
}

pub fn handle_move_to_trash_frame_with_service<H: MoveToTrashHelper>(
    context: &WsConnectionContext,
    frame: &TdrvFrame,
    config: &AppConfig,
    helper_client: &H,
) -> Result<TdrvFrame, TealDriveError> {
    if !context.protocol_ready {
        return Err(TealDriveError::Validation);
    }
    if frame.header.message_type != MessageType::MoveToTrash {
        return Err(TealDriveError::Validation);
    }
    if frame.header.encoding != Encoding::MessagePack {
        return Err(TealDriveError::InvalidEncoding);
    }

    let request: MoveToTrashRequest = decode_msgpack(&frame.payload)?;
    let policy_engine = PolicyEngine::new(config);
    let response = move_to_trash_request(
        &context.user_context(),
        request,
        config,
        &policy_engine,
        helper_client,
    )?;
    let bytes = encode_msgpack(&response)?;
    let mut header = TdrvHeader::new(MessageType::OperationDone, frame.header.request_id, 0);
    header.encoding = Encoding::MessagePack;
    TdrvFrame::new(header, bytes)
}

pub fn handle_restore_from_trash_frame_with_service<H: RestoreFromTrashHelper>(
    context: &WsConnectionContext,
    frame: &TdrvFrame,
    config: &AppConfig,
    helper_client: &H,
) -> Result<TdrvFrame, TealDriveError> {
    if !context.protocol_ready {
        return Err(TealDriveError::Validation);
    }
    if frame.header.message_type != MessageType::RestoreFromTrash {
        return Err(TealDriveError::Validation);
    }
    if frame.header.encoding != Encoding::MessagePack {
        return Err(TealDriveError::InvalidEncoding);
    }

    let request: RestoreFromTrashRequest = decode_msgpack(&frame.payload)?;
    let policy_engine = PolicyEngine::new(config);
    let response = restore_from_trash_request(
        &context.user_context(),
        request,
        config,
        &policy_engine,
        helper_client,
    )?;
    let bytes = encode_msgpack(&response)?;
    let mut header = TdrvHeader::new(MessageType::OperationDone, frame.header.request_id, 0);
    header.encoding = Encoding::MessagePack;
    TdrvFrame::new(header, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AllowedRoot, AppConfig};
    use crate::helper::create_folder::create_folder_with_roots;
    use crate::helper::download::download_file_with_roots;
    use crate::helper::listing::list_directory_with_roots;
    use crate::helper::metadata::file_metadata_with_roots;
    use crate::helper::move_copy::{copy_file_with_roots, move_file_with_roots};
    use crate::helper::rename::rename_with_roots;
    use crate::helper::text::read_text_file_with_roots;
    use crate::helper::types::{HelperRequest, HelperResponse};
    use crate::limits::Limits;
    use crate::protocol::compression::Compression;
    use crate::protocol::flags::FLAG_COMPRESSED;
    use crate::protocol::header::{TDRV_MAGIC, TDRV_VERSION};
    use crate::protocol::schema::{FileFilter, FilterMode, SortMode};
    use crate::{config::RelativePath, config::RootId};
    use std::fs;
    use std::path::PathBuf;

    struct InProcessHelper {
        roots: Vec<AllowedRoot>,
    }

    impl DirectoryListingHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(list_directory_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl MetadataHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(file_metadata_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl DownloadHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(download_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl TextPreviewHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(read_text_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl CreateFolderHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(create_folder_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl RenameHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(rename_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl MoveHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(move_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl CopyHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(copy_file_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl MoveToTrashHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(crate::helper::trash::move_file_to_trash_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    impl RestoreFromTrashHelper for InProcessHelper {
        fn execute_helper(
            &self,
            request: &HelperRequest,
        ) -> Result<HelperResponse, TealDriveError> {
            Ok(crate::helper::trash::restore_file_from_trash_with_roots(
                request,
                &self.roots,
                &crate::config::SensitivePolicyConfig::default(),
            ))
        }
    }

    fn client_frame(message_type: MessageType, payload: Vec<u8>) -> TdrvFrame {
        let mut header = TdrvHeader::new(message_type, RequestId::new(), 0);
        header.encoding = Encoding::MessagePack;
        TdrvFrame::new(header, payload).expect("frame")
    }

    fn client_hello_frame() -> Vec<u8> {
        let limits = Limits::default();
        let payload = ClientHelloPayload {
            protocol_version: 1,
            supported_encodings: vec![Encoding::MessagePack],
            supported_compressions: vec![Compression::None],
            max_frame_size: limits.max_frame_payload,
            max_control_payload_size: limits.max_control_payload,
            max_chunk_size: limits.max_chunk_size,
            feature_flags: Default::default(),
        };
        client_frame(
            MessageType::ClientHello,
            encode_msgpack(&payload).expect("payload"),
        )
        .encode()
        .expect("encoded")
    }

    fn temp_root() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("tealdrive-dispatch-list-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temp root");
        path
    }

    fn list_directory_request_payload() -> ListDirectoryRequest {
        ListDirectoryRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(""),
            cursor: None,
            limit: 200,
            include_hidden: false,
            sort: SortMode::NameAsc,
            filter: FilterMode {
                query: None,
                file_kind: None,
                filter: Some(FileFilter::All),
            },
        }
    }

    fn file_metadata_request_payload(path: &str) -> FileMetadataRequest {
        FileMetadataRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(path),
        }
    }

    fn download_begin_request_payload(path: &str) -> DownloadBeginRequest {
        DownloadBeginRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(path),
            requested_chunk_size: Some(4),
        }
    }

    fn read_text_file_request_payload(path: &str) -> ReadTextFileRequest {
        ReadTextFileRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(path),
            max_bytes: None,
            encoding: None,
        }
    }

    fn create_folder_request_payload(name: &str) -> CreateFolderRequest {
        CreateFolderRequest {
            root_id: RootId::new("home"),
            parent_relative_path: RelativePath::new(""),
            folder_name: name.to_owned(),
        }
    }

    fn rename_file_request_payload(path: &str, new_name: &str) -> RenameFileRequest {
        RenameFileRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(path),
            new_name: new_name.to_owned(),
        }
    }

    fn move_file_request_payload(src: &str, dst: &str) -> MoveFileRequest {
        MoveFileRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(src),
            target_root_id: RootId::new("home"),
            target_relative_path: RelativePath::new(dst),
        }
    }

    fn copy_file_request_payload(src: &str, dst: &str) -> CopyFileRequest {
        CopyFileRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(src),
            target_root_id: RootId::new("home"),
            target_relative_path: RelativePath::new(dst),
        }
    }

    #[test]
    fn client_sent_server_hello_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let frame = client_frame(MessageType::ServerHello, Vec::new())
            .encode()
            .expect("encoded");

        assert!(matches!(
            handle_inbound_frame(&mut context, &frame),
            Err(TealDriveError::InvalidMessageDirection)
        ));
    }

    #[test]
    fn client_sent_directory_list_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let frame = client_frame(MessageType::DirectoryList, Vec::new())
            .encode()
            .expect("encoded");

        assert!(matches!(
            handle_inbound_frame(&mut context, &frame),
            Err(TealDriveError::InvalidMessageDirection)
        ));
    }

    #[test]
    fn list_directory_before_protocol_ready_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let frame = client_frame(
            MessageType::ListDirectory,
            encode_msgpack(&list_directory_request_payload()).expect("payload"),
        )
        .encode()
        .expect("encoded");

        assert!(matches!(
            handle_inbound_frame(&mut context, &frame),
            Err(TealDriveError::Validation)
        ));
    }

    #[test]
    fn file_metadata_before_protocol_ready_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let frame = client_frame(
            MessageType::FileMetadataRequest,
            encode_msgpack(&file_metadata_request_payload("file.txt")).expect("payload"),
        )
        .encode()
        .expect("encoded");

        assert!(matches!(
            handle_inbound_frame(&mut context, &frame),
            Err(TealDriveError::Validation)
        ));
    }

    #[test]
    fn client_sent_file_metadata_response_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let frame = client_frame(MessageType::FileMetadata, Vec::new())
            .encode()
            .expect("encoded");

        assert!(matches!(
            handle_inbound_frame(&mut context, &frame),
            Err(TealDriveError::InvalidMessageDirection)
        ));
    }

    #[test]
    fn client_sent_download_chunk_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let mut header = TdrvHeader::new(MessageType::DownloadChunk, RequestId::new(), 0);
        header.encoding = Encoding::RawBinary;
        let frame = TdrvFrame::new(header, Vec::new()).expect("frame");
        let bytes = frame.encode().expect("encoded");

        assert!(matches!(
            handle_inbound_frame(&mut context, &bytes),
            Err(TealDriveError::InvalidMessageDirection)
        ));
    }

    #[test]
    fn client_sent_text_file_content_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let frame = client_frame(MessageType::TextFileContent, Vec::new())
            .encode()
            .expect("encoded");

        assert!(matches!(
            handle_inbound_frame(&mut context, &frame),
            Err(TealDriveError::InvalidMessageDirection)
        ));
    }

    #[test]
    fn download_begin_before_protocol_ready_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let frame = client_frame(
            MessageType::DownloadBegin,
            encode_msgpack(&download_begin_request_payload("file.txt")).expect("payload"),
        )
        .encode()
        .expect("encoded");

        assert!(matches!(
            handle_inbound_frame(&mut context, &frame),
            Err(TealDriveError::Validation)
        ));
    }

    #[test]
    fn read_text_file_before_protocol_ready_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let frame = client_frame(
            MessageType::ReadTextFile,
            encode_msgpack(&read_text_file_request_payload("notes.txt")).expect("payload"),
        )
        .encode()
        .expect("encoded");

        assert!(matches!(
            handle_inbound_frame(&mut context, &frame),
            Err(TealDriveError::Validation)
        ));
    }

    #[test]
    fn create_folder_before_protocol_ready_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let frame = client_frame(
            MessageType::CreateFolder,
            encode_msgpack(&create_folder_request_payload("new-folder")).expect("payload"),
        )
        .encode()
        .expect("encoded");

        assert!(matches!(
            handle_inbound_frame(&mut context, &frame),
            Err(TealDriveError::Validation)
        ));
    }

    #[test]
    fn rename_file_before_protocol_ready_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let frame = client_frame(
            MessageType::RenameFile,
            encode_msgpack(&rename_file_request_payload("old.txt", "new.txt")).expect("payload"),
        )
        .encode()
        .expect("encoded");

        assert!(matches!(
            handle_inbound_frame(&mut context, &frame),
            Err(TealDriveError::Validation)
        ));
    }

    #[test]
    fn move_file_before_protocol_ready_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let frame = client_frame(
            MessageType::MoveFile,
            encode_msgpack(&move_file_request_payload("old.txt", "new.txt")).expect("payload"),
        )
        .encode()
        .expect("encoded");

        assert!(matches!(
            handle_inbound_frame(&mut context, &frame),
            Err(TealDriveError::Validation)
        ));
    }

    #[test]
    fn copy_file_before_protocol_ready_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let frame = client_frame(
            MessageType::CopyFile,
            encode_msgpack(&copy_file_request_payload("orig.txt", "copy.txt")).expect("payload"),
        )
        .encode()
        .expect("encoded");

        assert!(matches!(
            handle_inbound_frame(&mut context, &frame),
            Err(TealDriveError::Validation)
        ));
    }

    #[test]
    fn list_directory_after_ready_reaches_listing_service() {
        let base = temp_root();
        fs::write(base.join("visible.txt"), b"hello").expect("file");
        let mut context = crate::ws::upgrade::tests::valid_context();
        context.protocol_ready = true;
        let config = AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: base,
                read_only: false,
                uploads_allowed: true,
                hidden_files_allowed: false,
                is_web_root: false,
            }],
            ..AppConfig::default()
        };
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let frame = client_frame(
            MessageType::ListDirectory,
            encode_msgpack(&list_directory_request_payload()).expect("payload"),
        );

        let outbound = handle_list_directory_frame_with_service(&context, &frame, &config, &helper)
            .expect("directory list");

        assert_eq!(outbound.header.message_type, MessageType::DirectoryList);
    }

    #[test]
    fn file_metadata_after_ready_reaches_metadata_service() {
        let base = temp_root();
        fs::write(base.join("visible.txt"), b"hello").expect("file");
        let mut context = crate::ws::upgrade::tests::valid_context();
        context.protocol_ready = true;
        let config = AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: base,
                read_only: false,
                uploads_allowed: true,
                hidden_files_allowed: false,
                is_web_root: false,
            }],
            ..AppConfig::default()
        };
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let frame = client_frame(
            MessageType::FileMetadataRequest,
            encode_msgpack(&file_metadata_request_payload("visible.txt")).expect("payload"),
        );

        let outbound = handle_file_metadata_frame_with_service(&context, &frame, &config, &helper)
            .expect("metadata");

        assert_eq!(outbound.header.message_type, MessageType::FileMetadata);
    }

    #[test]
    fn download_begin_after_ready_reaches_download_service() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"hello").expect("file");
        let mut context = crate::ws::upgrade::tests::valid_context();
        context.protocol_ready = true;
        let config = AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: base,
                read_only: false,
                uploads_allowed: true,
                hidden_files_allowed: false,
                is_web_root: false,
            }],
            ..AppConfig::default()
        };
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let frame = client_frame(
            MessageType::DownloadBegin,
            encode_msgpack(&download_begin_request_payload("file.txt")).expect("payload"),
        );

        let frames = handle_download_begin_frame_with_service(&context, &frame, &config, &helper)
            .expect("download frames");

        assert_eq!(frames[0].header.message_type, MessageType::DownloadChunk);
        assert_eq!(
            frames.last().expect("end").header.message_type,
            MessageType::DownloadEnd
        );
    }

    #[test]
    fn read_text_file_after_ready_reaches_text_service() {
        let base = temp_root();
        fs::write(base.join("notes.txt"), b"hello").expect("file");
        let mut context = crate::ws::upgrade::tests::valid_context();
        context.protocol_ready = true;
        let config = AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: base,
                read_only: false,
                uploads_allowed: true,
                hidden_files_allowed: false,
                is_web_root: false,
            }],
            ..AppConfig::default()
        };
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let frame = client_frame(
            MessageType::ReadTextFile,
            encode_msgpack(&read_text_file_request_payload("notes.txt")).expect("payload"),
        );

        let outbound = handle_read_text_file_frame_with_service(&context, &frame, &config, &helper)
            .expect("text content");

        assert_eq!(outbound.header.message_type, MessageType::TextFileContent);
    }

    #[test]
    fn create_folder_after_ready_reaches_create_service() {
        let base = temp_root();
        let mut context = crate::ws::upgrade::tests::valid_context();
        context.protocol_ready = true;
        let config = AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: base.clone(),
                read_only: false,
                uploads_allowed: true,
                hidden_files_allowed: false,
                is_web_root: false,
            }],
            ..AppConfig::default()
        };
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let frame = client_frame(
            MessageType::CreateFolder,
            encode_msgpack(&create_folder_request_payload("new-folder")).expect("payload"),
        );

        let outbound = handle_create_folder_frame_with_service(&context, &frame, &config, &helper)
            .expect("operation done");

        assert_eq!(outbound.header.message_type, MessageType::OperationDone);
        assert!(base.join("new-folder").is_dir());
    }

    #[test]
    fn rename_file_after_ready_reaches_rename_service() {
        let base = temp_root();
        fs::write(base.join("old.txt"), b"hello").expect("file");
        let mut context = crate::ws::upgrade::tests::valid_context();
        context.protocol_ready = true;
        let config = AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: base.clone(),
                read_only: false,
                uploads_allowed: true,
                hidden_files_allowed: false,
                is_web_root: false,
            }],
            ..AppConfig::default()
        };
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let frame = client_frame(
            MessageType::RenameFile,
            encode_msgpack(&rename_file_request_payload("old.txt", "new.txt")).expect("payload"),
        );

        let outbound = handle_rename_file_frame_with_service(&context, &frame, &config, &helper)
            .expect("operation done");

        assert_eq!(outbound.header.message_type, MessageType::OperationDone);
        assert!(base.join("new.txt").is_file());
    }

    #[test]
    fn move_file_after_ready_reaches_move_service() {
        let base = temp_root();
        fs::write(base.join("old.txt"), b"hello").expect("file");
        let mut context = crate::ws::upgrade::tests::valid_context();
        context.protocol_ready = true;
        let config = AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: base.clone(),
                read_only: false,
                uploads_allowed: true,
                hidden_files_allowed: false,
                is_web_root: false,
            }],
            ..AppConfig::default()
        };
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let frame = client_frame(
            MessageType::MoveFile,
            encode_msgpack(&move_file_request_payload("old.txt", "new.txt")).expect("payload"),
        );

        let outbound = handle_move_file_frame_with_service(&context, &frame, &config, &helper)
            .expect("operation done");

        assert_eq!(outbound.header.message_type, MessageType::OperationDone);
        assert!(!base.join("old.txt").exists());
        assert!(base.join("new.txt").is_file());
    }

    #[test]
    fn copy_file_after_ready_reaches_copy_service() {
        let base = temp_root();
        fs::write(base.join("orig.txt"), b"hello").expect("file");
        let mut context = crate::ws::upgrade::tests::valid_context();
        context.protocol_ready = true;
        let config = AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: base.clone(),
                read_only: false,
                uploads_allowed: true,
                hidden_files_allowed: false,
                is_web_root: false,
            }],
            ..AppConfig::default()
        };
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let frame = client_frame(
            MessageType::CopyFile,
            encode_msgpack(&copy_file_request_payload("orig.txt", "copy.txt")).expect("payload"),
        );

        let outbound = handle_copy_file_frame_with_service(&context, &frame, &config, &helper)
            .expect("operation done");

        assert_eq!(outbound.header.message_type, MessageType::OperationDone);
        assert!(base.join("orig.txt").is_file());
        assert!(base.join("copy.txt").is_file());
    }

    fn move_to_trash_request_payload(path: &str) -> MoveToTrashRequest {
        MoveToTrashRequest {
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(path),
        }
    }

    fn restore_from_trash_request_payload(trash_id: &str) -> RestoreFromTrashRequest {
        RestoreFromTrashRequest {
            trash_id: trash_id.to_owned(),
            target_root_id: Some(RootId::new("home")),
            target_relative_path: None,
        }
    }

    #[test]
    fn move_to_trash_before_protocol_ready_rejected() {
        let context = crate::ws::upgrade::tests::valid_context();
        let frame = client_frame(
            MessageType::MoveToTrash,
            encode_msgpack(&move_to_trash_request_payload("file.txt")).expect("payload"),
        );
        let config = AppConfig::default();
        let helper = InProcessHelper { roots: vec![] };

        assert_eq!(
            handle_move_to_trash_frame_with_service(&context, &frame, &config, &helper),
            Err(TealDriveError::Validation)
        );
    }

    #[test]
    fn restore_from_trash_before_protocol_ready_rejected() {
        let context = crate::ws::upgrade::tests::valid_context();
        let frame = client_frame(
            MessageType::RestoreFromTrash,
            encode_msgpack(&restore_from_trash_request_payload(
                "00000000-0000-0000-0000-000000000000",
            ))
            .expect("payload"),
        );
        let config = AppConfig::default();
        let helper = InProcessHelper { roots: vec![] };

        assert_eq!(
            handle_restore_from_trash_frame_with_service(&context, &frame, &config, &helper),
            Err(TealDriveError::Validation)
        );
    }

    #[test]
    fn move_to_trash_after_ready_reaches_trash_service() {
        let base = temp_root();
        fs::write(base.join("trash_me.txt"), b"bye").expect("file");
        let mut context = crate::ws::upgrade::tests::valid_context();
        context.protocol_ready = true;
        let config = AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: base.clone(),
                read_only: false,
                uploads_allowed: true,
                hidden_files_allowed: false,
                is_web_root: false,
            }],
            ..AppConfig::default()
        };
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };
        let frame = client_frame(
            MessageType::MoveToTrash,
            encode_msgpack(&move_to_trash_request_payload("trash_me.txt")).expect("payload"),
        );

        let outbound = handle_move_to_trash_frame_with_service(&context, &frame, &config, &helper)
            .expect("operation done");

        assert_eq!(outbound.header.message_type, MessageType::OperationDone);
        assert!(!base.join("trash_me.txt").exists());
        let payload: crate::protocol::payload::MoveToTrashPayload =
            decode_msgpack(&outbound.payload).expect("payload");
        assert_eq!(payload.display_name, "trash_me.txt");
        assert!(!payload.trash_id.is_empty());
    }

    #[test]
    fn restore_from_trash_after_ready_reaches_trash_service() {
        let base = temp_root();
        fs::write(base.join("restore_me.txt"), b"hello").expect("file");
        let mut context = crate::ws::upgrade::tests::valid_context();
        context.protocol_ready = true;
        let config = AppConfig {
            roots: vec![AllowedRoot {
                root_id: RootId::new("home"),
                base_path: base.clone(),
                read_only: false,
                uploads_allowed: true,
                hidden_files_allowed: false,
                is_web_root: false,
            }],
            ..AppConfig::default()
        };
        let helper = InProcessHelper {
            roots: config.roots.clone(),
        };

        // First move to trash
        let trash_frame = client_frame(
            MessageType::MoveToTrash,
            encode_msgpack(&move_to_trash_request_payload("restore_me.txt")).expect("payload"),
        );
        let trash_resp =
            handle_move_to_trash_frame_with_service(&context, &trash_frame, &config, &helper)
                .expect("trash ok");
        let trash_payload: crate::protocol::payload::MoveToTrashPayload =
            decode_msgpack(&trash_resp.payload).expect("payload");

        // Then restore
        let restore_frame = client_frame(
            MessageType::RestoreFromTrash,
            encode_msgpack(&restore_from_trash_request_payload(&trash_payload.trash_id))
                .expect("payload"),
        );
        let restore_resp = handle_restore_from_trash_frame_with_service(
            &context,
            &restore_frame,
            &config,
            &helper,
        )
        .expect("restore ok");

        assert_eq!(restore_resp.header.message_type, MessageType::OperationDone);
        assert!(base.join("restore_me.txt").exists());
    }

    #[test]
    fn client_hello_moves_connection_to_ready() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let decision = handle_inbound_frame(&mut context, &client_hello_frame()).expect("decision");

        assert!(context.protocol_ready);
        assert!(
            matches!(decision, DispatchDecision::Outbound(frames) if frames[0].header.message_type == MessageType::ProtocolReady)
        );
    }

    #[test]
    fn invalid_magic_frame_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let mut bytes = client_hello_frame();
        bytes[0..4].copy_from_slice(b"NOPE");

        assert!(matches!(
            handle_inbound_frame(&mut context, &bytes),
            Err(TealDriveError::InvalidMagic)
        ));
    }

    #[test]
    fn unsupported_encoding_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let mut bytes = [0_u8; crate::protocol::header::HEADER_LEN].to_vec();
        bytes[0..4].copy_from_slice(&TDRV_MAGIC);
        bytes[4] = TDRV_VERSION;
        bytes[7..9].copy_from_slice(&u16::from(MessageType::ClientHello).to_be_bytes());
        bytes[10] = 9;

        assert!(matches!(
            handle_inbound_frame(&mut context, &bytes),
            Err(TealDriveError::InvalidEncoding)
        ));
    }

    #[test]
    fn unsupported_compression_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let mut bytes = client_hello_frame();
        bytes[5..7].copy_from_slice(&FLAG_COMPRESSED.to_be_bytes());
        bytes[9] = u8::from(Compression::ZstdFuture);

        assert!(matches!(
            handle_inbound_frame(&mut context, &bytes),
            Err(TealDriveError::InvalidCompression)
        ));
    }

    #[test]
    fn payload_too_large_rejected() {
        let mut context = crate::ws::upgrade::tests::valid_context();
        let mut bytes = client_hello_frame();
        let too_large = (crate::limits::MAX_FRAME_PAYLOAD as u32) + 1;
        bytes[28..32].copy_from_slice(&too_large.to_be_bytes());

        assert!(matches!(
            handle_inbound_frame(&mut context, &bytes),
            Err(TealDriveError::PayloadTooLarge)
        ));
    }
}
