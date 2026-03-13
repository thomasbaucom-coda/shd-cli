use crate::client::{CodaClient, ToolCaller};
use crate::error::Result;
use crate::output::{self, OutputFormat};
use crate::trace;
use serde_json::{json, Value};

/// Call any tool by name with a JSON payload, print the result, and return it.
/// Returns `Ok(Some(value))` on success, `Ok(None)` for dry-run, `Err` on failure.
/// The returned value is used by `--sync` to inspect the response for a `docUri`.
pub async fn call(
    client: &CodaClient,
    tool_name: &str,
    payload: Value,
    dry_run: bool,
    pick: Option<&str>,
    format: OutputFormat,
) -> Result<Option<Value>> {
    if dry_run {
        output::print_response(&client.dry_run_tool(tool_name, &payload)?, format)?;
        return Ok(None);
    }

    trace::emit_request(tool_name, &payload);
    let start = std::time::Instant::now();

    let result = client.call_tool(tool_name, payload).await;
    let elapsed_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(ref value) => {
            trace::emit_response(tool_name, value, elapsed_ms, false);
            if let Some(paths) = pick {
                pick_fields(value, paths)?;
            } else {
                output::print_response(value, format)?;
            }
        }
        Err(ref e) => {
            trace::emit_response(
                tool_name,
                &json!({"error": e.to_string()}),
                elapsed_ms,
                true,
            );
        }
    }

    result.map(Some)
}

/// Pick fields and print — public entry point for compound operations.
pub fn pick_and_print(value: &Value, paths: &str) -> Result<()> {
    pick_fields(value, paths)
}

/// Extract one or more fields from a JSON value.
/// Supports comma-separated paths: "name,id" extracts as JSON object {"name": ..., "id": ...}.
/// Each path is dot-separated: "items.0.id" walks into nested objects/arrays.
fn pick_fields(value: &Value, paths: &str) -> Result<()> {
    if paths.contains(',') {
        let parts: Vec<&str> = paths.split(',').map(|p| p.trim()).collect();
        let resolved: Vec<&Value> = parts
            .iter()
            .map(|p| super::resolve_path(value, p))
            .collect::<Result<Vec<_>>>()?;
        output::print_picked_multi(&parts, &resolved)
    } else {
        let picked = super::resolve_path(value, paths)?;
        output::print_picked(picked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pick_top_level_key() {
        let value = json!({"name": "Alice", "age": 30});
        assert!(pick_fields(&value, "name").is_ok());
    }

    #[test]
    fn pick_dot_path() {
        let value = json!({"user": {"name": "Alice", "email": "a@b.com"}});
        assert!(pick_fields(&value, "user.name").is_ok());
    }

    #[test]
    fn pick_array_index() {
        let value = json!({"items": [{"id": 1}, {"id": 2}]});
        assert!(pick_fields(&value, "items.0.id").is_ok());
    }

    #[test]
    fn pick_missing_field_errors() {
        let value = json!({"name": "Alice"});
        let err = pick_fields(&value, "missing").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn pick_deep_missing_errors() {
        let value = json!({"a": {"b": 1}});
        let err = pick_fields(&value, "a.c").unwrap_err();
        assert!(err.to_string().contains("failed at 'c'"));
    }

    #[test]
    fn pick_multi_fields() {
        let value = json!({"name": "Alice", "id": "abc123"});
        assert!(pick_fields(&value, "name,id").is_ok());
    }

    #[test]
    fn pick_multi_with_missing_errors() {
        let value = json!({"name": "Alice"});
        let err = pick_fields(&value, "name,missing").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }
}
