pub mod auth_cmd;
pub mod compound;
pub mod discover;
pub mod mcp;
pub mod shell;
pub mod sync;
pub mod tools;

use crate::error::{CodaError, Result};
use serde_json::Value;

/// Walk a dot-separated path through a JSON value.
/// Shared by tools::call (CLI pick) and shell (pick in JSON protocol).
pub fn resolve_path<'a>(value: &'a Value, path: &str) -> Result<&'a Value> {
    let mut current = value;
    for segment in path.split('.') {
        current = if let Ok(idx) = segment.parse::<usize>() {
            current.get(idx)
        } else {
            current.get(segment)
        }
        .ok_or_else(|| {
            let available = available_keys(current);
            let hint = if available.is_empty() {
                String::new()
            } else {
                format!(". Available: {}", available.join(", "))
            };
            CodaError::Validation(format!(
                "Field '{path}' not found (failed at '{segment}'){hint}"
            ))
        })?;
    }
    Ok(current)
}

/// List available keys/indices for error hints.
fn available_keys(value: &Value) -> Vec<String> {
    match value {
        Value::Object(map) => map.keys().cloned().collect(),
        Value::Array(arr) => {
            if arr.len() <= 5 {
                (0..arr.len()).map(|i| i.to_string()).collect()
            } else {
                vec![format!("0..{}", arr.len() - 1)]
            }
        }
        _ => vec![],
    }
}
