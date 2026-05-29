use serde::{Deserialize, Serialize};

pub const MIB: usize = 1024 * 1024;
pub const KIB: usize = 1024;

pub const MAX_FRAME_PAYLOAD: usize = MIB;
pub const MAX_CONTROL_PAYLOAD: usize = 256 * KIB;
pub const MAX_DECOMPRESSED_CONTROL_PAYLOAD: usize = 4 * MIB;
pub const MAX_CHUNK_SIZE: usize = 256 * KIB;
pub const MAX_IN_FLIGHT_UPLOAD: usize = MIB;
pub const MAX_DIRECTORY_PAGE_SIZE: usize = 500;
pub const DEFAULT_DIRECTORY_PAGE_SIZE: usize = 200;
pub const MAX_TEXT_EDIT_SIZE: usize = 5 * MIB;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    pub max_frame_payload: usize,
    pub max_control_payload: usize,
    pub max_decompressed_control_payload: usize,
    pub max_chunk_size: usize,
    pub max_in_flight_upload: usize,
    pub max_directory_page_size: usize,
    pub default_directory_page_size: usize,
    pub max_text_edit_size: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_frame_payload: MAX_FRAME_PAYLOAD,
            max_control_payload: MAX_CONTROL_PAYLOAD,
            max_decompressed_control_payload: MAX_DECOMPRESSED_CONTROL_PAYLOAD,
            max_chunk_size: MAX_CHUNK_SIZE,
            max_in_flight_upload: MAX_IN_FLIGHT_UPLOAD,
            max_directory_page_size: MAX_DIRECTORY_PAGE_SIZE,
            default_directory_page_size: DEFAULT_DIRECTORY_PAGE_SIZE,
            max_text_edit_size: MAX_TEXT_EDIT_SIZE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_limits_match_v1_architecture() {
        let limits = Limits::default();

        assert_eq!(limits.max_frame_payload, 1024 * 1024);
        assert_eq!(limits.max_control_payload, 256 * 1024);
        assert_eq!(limits.max_decompressed_control_payload, 4 * 1024 * 1024);
        assert_eq!(limits.max_chunk_size, 256 * 1024);
        assert_eq!(limits.max_in_flight_upload, 1024 * 1024);
        assert_eq!(limits.max_directory_page_size, 500);
        assert_eq!(limits.default_directory_page_size, 200);
        assert_eq!(limits.max_text_edit_size, 5 * 1024 * 1024);
    }
}
