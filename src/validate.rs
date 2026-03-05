use crate::error::CodaError;

/// Validates a Coda resource identifier (docId, tableId, rowId, etc.).
/// Rejects control characters, URL-special characters, path traversal,
/// and percent-encoded bypasses.
pub fn validate_resource_id<'a>(value: &'a str, name: &str) -> Result<&'a str, CodaError> {
    if value.is_empty() {
        return Err(CodaError::Validation(format!("{name} must not be empty")));
    }

    // Reject control characters (including null, tab, newline, DEL)
    if value.bytes().any(|b| b < 0x20 || b == 0x7F) {
        return Err(CodaError::Validation(format!(
            "{name} contains invalid control characters"
        )));
    }

    // Reject URL-special characters that could inject query params or fragments
    if value.contains('?') || value.contains('#') {
        return Err(CodaError::Validation(format!(
            "{name} must not contain '?' or '#': {value}"
        )));
    }

    // Reject percent signs to prevent URL-encoded bypasses (%2e%2e for ..)
    if value.contains('%') {
        return Err(CodaError::Validation(format!(
            "{name} must not contain '%' (possible URL encoding bypass): {value}"
        )));
    }

    // Reject path traversal
    if value.contains("..") {
        return Err(CodaError::Validation(format!(
            "{name} must not contain path traversal (..): {value}"
        )));
    }

    Ok(value)
}

/// Validates a raw JSON payload string parses as valid JSON.
/// Returns the parsed value.
pub fn validate_json_payload(raw: &str) -> Result<serde_json::Value, CodaError> {
    serde_json::from_str(raw).map_err(|e| {
        CodaError::Validation(format!("Invalid JSON payload: {e}"))
    })
}

/// Resolve a JSON payload from either a literal string or stdin (if value is "-").
pub fn resolve_json_payload(value: &str) -> Result<serde_json::Value, CodaError> {
    if value == "-" {
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
            .map_err(CodaError::Io)?;
        validate_json_payload(buf.trim())
    } else {
        validate_json_payload(value)
    }
}

/// Percent-encode a value for use in a URL path segment.
pub fn encode_path_segment(s: &str) -> String {
    use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_ids() {
        assert!(validate_resource_id("AbCdEf123", "docId").is_ok());
        assert!(validate_resource_id("i-row-abc_123", "rowId").is_ok());
    }

    #[test]
    fn rejects_empty() {
        assert!(validate_resource_id("", "docId").is_err());
    }

    #[test]
    fn rejects_control_chars() {
        assert!(validate_resource_id("abc\0def", "docId").is_err());
        assert!(validate_resource_id("abc\ndef", "docId").is_err());
        assert!(validate_resource_id("abc\tdef", "docId").is_err());
    }

    #[test]
    fn rejects_query_injection() {
        assert!(validate_resource_id("abc?fields=name", "docId").is_err());
        assert!(validate_resource_id("abc#fragment", "docId").is_err());
    }

    #[test]
    fn rejects_percent_encoding_bypass() {
        assert!(validate_resource_id("%2e%2e", "docId").is_err());
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(validate_resource_id("../../etc", "docId").is_err());
    }

    #[test]
    fn encodes_path_segment() {
        assert_eq!(encode_path_segment("abc123"), "abc123");
        let encoded = encode_path_segment("has spaces");
        assert!(!encoded.contains(' '));
    }
}
