use crate::client::CodaClient;
use crate::error::{CodaError, Result};
use crate::output;
use crate::schema_cache;
use crate::trace;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Persistent REPL mode for agents. Keeps the HTTP client alive across calls.
/// Protocol: one JSON object per line on stdin, one JSON response per line on stdout.
///
/// Input format:
///   {"tool": "whoami"}
///   {"tool": "document_create", "payload": {"title": "Test"}}
///   {"tool": "document_create", "payload": {"title": "Test"}, "pick": "docId"}
///
/// Output format:
///   {"ok": true, "result": {...}}
///   {"ok": true, "result": "picked_value"}
///   {"ok": false, "error": {"type": "validation_error", "message": "..."}}
pub async fn start(client: &CodaClient, dry_run: bool) -> Result<()> {
    output::info("[coda shell] Starting persistent shell. Send JSON lines to stdin.\n");

    let mut stdin = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();

    while let Ok(Some(line)) = stdin.next_line().await {
        if line.trim().is_empty() {
            continue;
        }

        let response = match process_line(client, &line, dry_run).await {
            Ok(val) => json!({"ok": true, "result": val}),
            Err(e) => json!({
                "ok": false,
                "error": {
                    "type": error_type(&e),
                    "message": e.to_string(),
                }
            }),
        };

        let mut out = serde_json::to_string(&response).unwrap_or_else(|_| {
            r#"{"ok":false,"error":{"type":"error","message":"Serialization error"}}"#.into()
        });
        out.push('\n');
        let _ = stdout.write_all(out.as_bytes()).await;
        let _ = stdout.flush().await;
    }

    Ok(())
}

async fn process_line(client: &CodaClient, line: &str, dry_run: bool) -> Result<Value> {
    let req: Value = serde_json::from_str(line)
        .map_err(|e| CodaError::Validation(format!("Invalid JSON: {e}")))?;

    let tool_name = req
        .get("tool")
        .and_then(|t| t.as_str())
        .ok_or_else(|| CodaError::Validation("Missing 'tool' field".into()))?;

    let payload = req.get("payload").cloned().unwrap_or(json!({}));
    let pick = req.get("pick").and_then(|p| p.as_str());

    // Schema validation if cache available
    if let Ok(Some(cached)) = schema_cache::load() {
        if let Some(tool_schema) = schema_cache::find_tool(&cached.tools, tool_name) {
            schema_cache::validate_payload(tool_schema, &payload)?;
        }
    }

    if dry_run {
        return Ok(client.dry_run_tool(tool_name, &payload));
    }

    trace::emit_request(tool_name, &payload);
    let start = std::time::Instant::now();

    let result = client.call_tool(tool_name, payload).await?;
    let elapsed_ms = start.elapsed().as_millis() as u64;
    trace::emit_response(tool_name, &result, elapsed_ms, false);

    // Apply pick if requested
    if let Some(paths) = pick {
        return pick_from_value(&result, paths);
    }

    Ok(result)
}

/// Pick fields from a Value, returning the extracted value(s).
fn pick_from_value(value: &Value, paths: &str) -> Result<Value> {
    if paths.contains(',') {
        let results: Vec<Value> = paths
            .split(',')
            .map(|p| resolve_path(value, p.trim()).cloned())
            .collect::<Result<Vec<_>>>()?;
        Ok(Value::Array(results))
    } else {
        resolve_path(value, paths).cloned()
    }
}

fn resolve_path<'a>(value: &'a Value, path: &str) -> Result<&'a Value> {
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

fn error_type(e: &CodaError) -> &'static str {
    match e {
        CodaError::ContractChanged { .. } => "contract_changed",
        CodaError::Api { .. } => "api_error",
        CodaError::Validation(_) => "validation_error",
        CodaError::NoToken => "auth_required",
        _ => "error",
    }
}
