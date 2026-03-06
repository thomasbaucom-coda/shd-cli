use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};

static TRACE_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn set_trace(enabled: bool) {
    TRACE_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn is_enabled() -> bool {
    TRACE_ENABLED.load(Ordering::Relaxed)
}

pub fn emit_request(tool_name: &str, payload: &Value) {
    if !is_enabled() {
        return;
    }
    let payload_keys: Vec<&str> = payload
        .as_object()
        .map(|obj| obj.keys().map(|k| k.as_str()).collect())
        .unwrap_or_default();

    let trace = serde_json::json!({
        "event": "request",
        "tool": tool_name,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "payload_keys": payload_keys,
    });
    eprintln!("{}", serde_json::to_string(&trace).unwrap_or_default());
}

pub fn emit_response(tool_name: &str, result: &Value, duration_ms: u64, is_error: bool) {
    if !is_enabled() {
        return;
    }
    let trace = serde_json::json!({
        "event": "response",
        "tool": tool_name,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "duration_ms": duration_ms,
        "is_error": is_error,
        "result_type": match result {
            Value::Object(m) => format!("object({})", m.len()),
            Value::Array(a) => format!("array({})", a.len()),
            Value::Null => "null".to_string(),
            _ => "scalar".to_string(),
        },
    });
    eprintln!("{}", serde_json::to_string(&trace).unwrap_or_default());
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn silent_when_disabled() {
        set_trace(false);
        // These should be no-ops (no panic, no output)
        emit_request("whoami", &json!({}));
        emit_response("whoami", &json!({"name": "test"}), 42, false);
    }

    #[test]
    fn trace_request_format() {
        // We can't easily capture stderr in a unit test, but we can verify
        // the serialization logic doesn't panic when enabled
        set_trace(true);
        emit_request("doc_create", &json!({"docId": "abc", "name": "test"}));
        emit_response("doc_create", &json!({"id": "123"}), 150, false);
        set_trace(false); // reset
    }
}
