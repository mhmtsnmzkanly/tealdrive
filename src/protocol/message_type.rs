use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

use crate::errors::TealDriveError;
use crate::protocol::direction::MessageTypeDirection;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u16)]
pub enum MessageType {
    ServerHello = 0x0001,
    ClientHello = 0x0002,
    ProtocolReady = 0x0003,
    ConnectionStatus = 0x0004,
    Error = 0x0005,
    ListDirectory = 0x0100,
    DirectoryList = 0x0101,
    FileMetadata = 0x0102,
    ReadTextFile = 0x0103,
    TextFileContent = 0x0104,
    SaveTextFile = 0x0105,
    UploadBegin = 0x0200,
    UploadChunk = 0x0201,
    UploadEnd = 0x0202,
    UploadAbort = 0x0203,
    DownloadBegin = 0x0210,
    DownloadChunk = 0x0211,
    DownloadEnd = 0x0212,
    CreateFolder = 0x0300,
    RenameFile = 0x0301,
    MoveFile = 0x0302,
    CopyFile = 0x0303,
    MoveToTrash = 0x0304,
    RestoreFromTrash = 0x0305,
    DeletePermanently = 0x0306,
    ChmodFile = 0x0307,
    CompressZip = 0x0310,
    CompressTarGz = 0x0311,
    ExtractArchive = 0x0312,
    OperationStarted = 0x0400,
    OperationProgress = 0x0401,
    OperationDone = 0x0402,
    OperationFailed = 0x0403,
    PermissionDenied = 0x0404,
    PolicyDenied = 0x0405,
    ValidationError = 0x0406,
}

impl MessageType {
    pub fn direction(self) -> MessageTypeDirection {
        use MessageType::*;
        match self {
            ClientHello | ListDirectory | ReadTextFile | SaveTextFile | UploadBegin
            | UploadChunk | UploadEnd | UploadAbort | DownloadBegin | CreateFolder | RenameFile
            | MoveFile | CopyFile | MoveToTrash | RestoreFromTrash | DeletePermanently
            | ChmodFile | CompressZip | CompressTarGz | ExtractArchive => {
                MessageTypeDirection::ClientToServer
            }
            ServerHello | ProtocolReady | ConnectionStatus | DirectoryList | FileMetadata
            | TextFileContent | DownloadChunk | DownloadEnd | OperationStarted
            | OperationProgress | OperationDone | OperationFailed | PermissionDenied
            | PolicyDenied | ValidationError => MessageTypeDirection::ServerToClient,
            Error => MessageTypeDirection::ServerToClient,
        }
    }

    pub fn is_handshake(self) -> bool {
        matches!(
            self,
            Self::ServerHello | Self::ClientHello | Self::ProtocolReady
        )
    }

    pub fn is_operation(self) -> bool {
        !matches!(
            self,
            Self::ServerHello
                | Self::ClientHello
                | Self::ProtocolReady
                | Self::ConnectionStatus
                | Self::Error
        )
    }

    pub fn validate_inbound_client(self) -> Result<(), TealDriveError> {
        match self.direction() {
            MessageTypeDirection::ClientToServer | MessageTypeDirection::Both => Ok(()),
            MessageTypeDirection::ServerToClient => Err(TealDriveError::InvalidMessageDirection),
        }
    }
}

impl TryFrom<u16> for MessageType {
    type Error = TealDriveError;

    fn try_from(value: u16) -> Result<Self, TealDriveError> {
        use MessageType::*;
        match value {
            0x0001 => Ok(ServerHello),
            0x0002 => Ok(ClientHello),
            0x0003 => Ok(ProtocolReady),
            0x0004 => Ok(ConnectionStatus),
            0x0005 => Ok(Error),
            0x0100 => Ok(ListDirectory),
            0x0101 => Ok(DirectoryList),
            0x0102 => Ok(FileMetadata),
            0x0103 => Ok(ReadTextFile),
            0x0104 => Ok(TextFileContent),
            0x0105 => Ok(SaveTextFile),
            0x0200 => Ok(UploadBegin),
            0x0201 => Ok(UploadChunk),
            0x0202 => Ok(UploadEnd),
            0x0203 => Ok(UploadAbort),
            0x0210 => Ok(DownloadBegin),
            0x0211 => Ok(DownloadChunk),
            0x0212 => Ok(DownloadEnd),
            0x0300 => Ok(CreateFolder),
            0x0301 => Ok(RenameFile),
            0x0302 => Ok(MoveFile),
            0x0303 => Ok(CopyFile),
            0x0304 => Ok(MoveToTrash),
            0x0305 => Ok(RestoreFromTrash),
            0x0306 => Ok(DeletePermanently),
            0x0307 => Ok(ChmodFile),
            0x0310 => Ok(CompressZip),
            0x0311 => Ok(CompressTarGz),
            0x0312 => Ok(ExtractArchive),
            0x0400 => Ok(OperationStarted),
            0x0401 => Ok(OperationProgress),
            0x0402 => Ok(OperationDone),
            0x0403 => Ok(OperationFailed),
            0x0404 => Ok(PermissionDenied),
            0x0405 => Ok(PolicyDenied),
            0x0406 => Ok(ValidationError),
            _ => Err(TealDriveError::UnknownMessageType),
        }
    }
}

impl From<MessageType> for u16 {
    fn from(value: MessageType) -> Self {
        value as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_message_type_ids_parse() {
        assert_eq!(MessageType::try_from(0x0001), Ok(MessageType::ServerHello));
        assert_eq!(
            MessageType::try_from(0x0100),
            Ok(MessageType::ListDirectory)
        );
        assert_eq!(MessageType::try_from(0x0201), Ok(MessageType::UploadChunk));
        assert_eq!(
            MessageType::try_from(0x0402),
            Ok(MessageType::OperationDone)
        );
    }

    #[test]
    fn unknown_message_type_id_rejected() {
        assert!(matches!(
            MessageType::try_from(0x9999),
            Err(TealDriveError::UnknownMessageType)
        ));
    }

    #[test]
    fn stable_numeric_values_are_preserved() {
        assert_eq!(u16::from(MessageType::ServerHello), 0x0001);
        assert_eq!(u16::from(MessageType::DirectoryList), 0x0101);
        assert_eq!(u16::from(MessageType::DownloadChunk), 0x0211);
        assert_eq!(u16::from(MessageType::ExtractArchive), 0x0312);
    }

    #[test]
    fn direction_validation_rejects_client_sent_server_only_message() {
        assert_eq!(MessageType::ListDirectory.validate_inbound_client(), Ok(()));
        assert!(matches!(
            MessageType::DirectoryList.validate_inbound_client(),
            Err(TealDriveError::InvalidMessageDirection)
        ));
    }

    #[test]
    fn handshake_and_operation_messages_are_distinguishable() {
        assert!(MessageType::ClientHello.is_handshake());
        assert!(!MessageType::ListDirectory.is_handshake());
        assert!(MessageType::ListDirectory.is_operation());
        assert!(!MessageType::ProtocolReady.is_operation());
    }
}
