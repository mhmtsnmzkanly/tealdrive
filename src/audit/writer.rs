use crate::audit::event::AuditEvent;

/// Audit JSONL is allowed because it is storage, not the production wire protocol.
pub fn to_jsonl(event: &AuditEvent) -> Result<String, serde_json::Error> {
    serde_json::to_string(event)
}
