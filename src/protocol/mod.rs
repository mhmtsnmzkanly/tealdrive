pub mod chunk;
pub mod codec;
pub mod compression;
pub mod direction;
pub mod flags;
pub mod frame;
pub mod header;
pub mod message_type;
pub mod payload;
pub mod schema;

pub use frame::{RequestId, TdrvFrame, TransferId};
pub use header::TdrvHeader;
pub use message_type::MessageType;
pub const JAVASCRIPT_CONTRACT: &str = include_str!(concat!(env!("OUT_DIR"), "/protocol-contract.js"));
