use serde::{Deserialize, Serialize};

use crate::errors::PolicyReason;
use crate::protocol::message_type::MessageType;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureFlags {
    pub save_text_file: bool,
    pub chmod_file: bool,
    pub extract_archive: bool,
    pub delete_permanently: bool,
    pub chown: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeatureGateResult {
    Allowed,
    Disabled {
        operation: DisabledOperation,
        reason: PolicyReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisabledOperation {
    SaveTextFile,
    ChmodFile,
    ExtractArchive,
    DeletePermanently,
    Chown,
}

pub struct FeatureGate;

impl FeatureGate {
    pub fn check(message_type: MessageType, flags: &FeatureFlags) -> FeatureGateResult {
        let disabled = match message_type {
            MessageType::SaveTextFile if !flags.save_text_file => {
                Some(DisabledOperation::SaveTextFile)
            }
            MessageType::ChmodFile if !flags.chmod_file => Some(DisabledOperation::ChmodFile),
            MessageType::ExtractArchive if !flags.extract_archive => {
                Some(DisabledOperation::ExtractArchive)
            }
            MessageType::DeletePermanently if !flags.delete_permanently => {
                Some(DisabledOperation::DeletePermanently)
            }
            _ => None,
        };

        match disabled {
            Some(operation) => FeatureGateResult::Disabled {
                operation,
                reason: PolicyReason::FeatureDisabled,
            },
            None => FeatureGateResult::Allowed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dangerous_v1_features_are_disabled_by_default() {
        let flags = FeatureFlags::default();

        assert_eq!(
            FeatureGate::check(MessageType::SaveTextFile, &flags),
            FeatureGateResult::Disabled {
                operation: DisabledOperation::SaveTextFile,
                reason: PolicyReason::FeatureDisabled,
            }
        );
        assert_eq!(
            FeatureGate::check(MessageType::ChmodFile, &flags),
            FeatureGateResult::Disabled {
                operation: DisabledOperation::ChmodFile,
                reason: PolicyReason::FeatureDisabled,
            }
        );
        assert_eq!(
            FeatureGate::check(MessageType::ExtractArchive, &flags),
            FeatureGateResult::Disabled {
                operation: DisabledOperation::ExtractArchive,
                reason: PolicyReason::FeatureDisabled,
            }
        );
        assert_eq!(
            FeatureGate::check(MessageType::DeletePermanently, &flags),
            FeatureGateResult::Disabled {
                operation: DisabledOperation::DeletePermanently,
                reason: PolicyReason::FeatureDisabled,
            }
        );
    }
}
