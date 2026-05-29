use crate::protocol::frame::TransferId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadState {
    pub transfer_id: TransferId,
    pub bytes_received: u64,
}
