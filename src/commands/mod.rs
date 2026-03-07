pub mod auth_cmd;
pub mod discover;
pub mod mcp;
pub mod shell;
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
            CodaError::Validation(format!("Field '{path}' not found (failed at '{segment}')"))
        })?;
    }
    Ok(current)
}
