pub fn redact_secret(_field_name: &str, value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        "[REDACTED]".to_owned()
    }
}
