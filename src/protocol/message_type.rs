use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum MessageType {
    // Protocol Handshake & System
    ServerHello = 0x0001,
    ClientHello = 0x0002,
    ProtocolReady = 0x0003,
    ConnectionStatus = 0x0004,
    AuthSession = 0x0005, // Added for session binding

    // Directory Operations
    ListDirectory = 0x0010,
    DirectoryList = 0x0011,

    // File Metadata
    FileMetadata = 0x0021,

    // Text/Code Operations
    ReadTextFile = 0x0030,
    TextFileContent = 0x0031,
    SaveTextFile = 0x0032,

    // Upload Operations
    UploadBegin = 0x0100,
    UploadChunk = 0x0101,
    UploadEnd = 0x0102,
    UploadAbort = 0x0103,

    // Download Operations
    DownloadBegin = 0x0200,
    DownloadChunk = 0x0201,
    DownloadEnd = 0x0202,

    // File Operations
    RenameFile = 0x0300,
    MoveFile = 0x0301,
    CopyFile = 0x0302,
    MoveToTrash = 0x0303,
    RestoreFromTrash = 0x0304,
    DeletePermanently = 0x0305,
    ChmodFile = 0x0306,
    CompressZip = 0x0310,
    CompressTarGz = 0x0311,
    ExtractArchive = 0x0312,

    // Operation Status
    OperationStarted = 0x0400,
    OperationProgress = 0x0401,
    OperationDone = 0x0402,
    OperationFailed = 0x0403,
    PermissionDenied = 0x0404,
    PolicyDenied = 0x0405,
    ValidationError = 0x0406,
    FileChanged = 0x0407,
    Error = 0x0408,
}

impl From<u16> for MessageType {
    fn from(val: u16) -> Self {
        match val {
            0x0001 => MessageType::ServerHello,
            0x0002 => MessageType::ClientHello,
            0x0003 => MessageType::ProtocolReady,
            0x0004 => MessageType::ConnectionStatus,
            0x0005 => MessageType::AuthSession,
            0x0010 => MessageType::ListDirectory,
            0x0011 => MessageType::DirectoryList,
            0x0021 => MessageType::FileMetadata,
            0x0030 => MessageType::ReadTextFile,
            0x0031 => MessageType::TextFileContent,
            0x0032 => MessageType::SaveTextFile,
            0x0100 => MessageType::UploadBegin,
            0x0101 => MessageType::UploadChunk,
            0x0102 => MessageType::UploadEnd,
            0x0103 => MessageType::UploadAbort,
            0x0200 => MessageType::DownloadBegin,
            0x0201 => MessageType::DownloadChunk,
            0x0202 => MessageType::DownloadEnd,
            0x0300 => MessageType::RenameFile,
            0x0301 => MessageType::MoveFile,
            0x0302 => MessageType::CopyFile,
            0x0303 => MessageType::MoveToTrash,
            0x0304 => MessageType::RestoreFromTrash,
            0x0305 => MessageType::DeletePermanently,
            0x0306 => MessageType::ChmodFile,
            0x0310 => MessageType::CompressZip,
            0x0311 => MessageType::CompressTarGz,
            0x0312 => MessageType::ExtractArchive,
            0x0400 => MessageType::OperationStarted,
            0x0401 => MessageType::OperationProgress,
            0x0402 => MessageType::OperationDone,
            0x0403 => MessageType::OperationFailed,
            0x0404 => MessageType::PermissionDenied,
            0x0405 => MessageType::PolicyDenied,
            0x0406 => MessageType::ValidationError,
            0x0407 => MessageType::FileChanged,
            0x0408 => MessageType::Error,
            _ => MessageType::OperationFailed, // Default to fail for unknown
        }
    }
}
