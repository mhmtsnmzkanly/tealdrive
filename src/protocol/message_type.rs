use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum MessageType {
    // Auth & Connection
    AuthSession = 0x0001,
    ServerHello = 0x0002,

    // Directory Operations
    ListDirectory = 0x0010,
    DirectoryList = 0x0011,

    // File Metadata
    GetFileMetadata = 0x0020,
    FileMetadata = 0x0021,

    // Text File Operations
    ReadTextFile = 0x0030,
    SaveTextFile = 0x0031,
    TextFileContent = 0x0032,

    // Upload Operations
    UploadBegin = 0x0100,
    UploadChunk = 0x0101,
    UploadEnd = 0x0102,
    UploadProgress = 0x0103,
    UploadError = 0x0104,

    // Download Operations
    DownloadRequest = 0x0200,
    DownloadBegin = 0x0201,
    DownloadChunk = 0x0202,
    DownloadEnd = 0x0203,

    // File Management
    RenameFile = 0x0300,
    MoveFile = 0x0301,
    CopyFile = 0x0302,
    MoveToTrash = 0x0303,
    RestoreFromTrash = 0x0304,
    DeletePermanently = 0x0305,
    ChmodFile = 0x0306,

    // Archive Operations
    CompressZip = 0x0310,
    CompressTarGz = 0x0311,
    ExtractArchive = 0x0312,

    // Feedback & System
    OperationStarted = 0x0400,
    OperationProgress = 0x0401,
    OperationDone = 0x0402,
    OperationFailed = 0x0403,
    PermissionDenied = 0x0404,
    PolicyDenied = 0x0405,
    FileChanged = 0x0406,
    Error = 0x0407,

    // Control
    CancelOperation = 0x0500,
    Ping = 0x0501,
    Pong = 0x0502,
}

impl From<u16> for MessageType {
    fn from(val: u16) -> Self {
        match val {
            0x0001 => MessageType::AuthSession,
            0x0002 => MessageType::ServerHello,
            0x0010 => MessageType::ListDirectory,
            0x0011 => MessageType::DirectoryList,
            0x0020 => MessageType::GetFileMetadata,
            0x0021 => MessageType::FileMetadata,
            0x0030 => MessageType::ReadTextFile,
            0x0031 => MessageType::SaveTextFile,
            0x0032 => MessageType::TextFileContent,
            0x0100 => MessageType::UploadBegin,
            0x0101 => MessageType::UploadChunk,
            0x0102 => MessageType::UploadEnd,
            0x0103 => MessageType::UploadProgress,
            0x0104 => MessageType::UploadError,
            0x0200 => MessageType::DownloadRequest,
            0x0201 => MessageType::DownloadBegin,
            0x0202 => MessageType::DownloadChunk,
            0x0203 => MessageType::DownloadEnd,
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
            0x0406 => MessageType::FileChanged,
            0x0407 => MessageType::Error,
            0x0500 => MessageType::CancelOperation,
            0x0501 => MessageType::Ping,
            0x0502 => MessageType::Pong,
            _ => MessageType::Error,
        }
    }
}
