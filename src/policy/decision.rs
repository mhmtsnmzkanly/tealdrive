use serde::{Deserialize, Serialize};

use crate::errors::PolicyReason;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyDecision {
    Allow,
    Deny(PolicyReason),
    WarnAllowed(PolicyReason),
}
