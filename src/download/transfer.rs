use crate::protocol::frame::TransferId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadState {
    pub transfer_id: TransferId,
    pub bytes_sent: u64,
}
