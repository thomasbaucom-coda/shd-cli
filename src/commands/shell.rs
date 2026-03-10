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
                    "type": e.error_type(),
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
        return client.dry_run_tool(tool_name, &payload);
    }

    // Route compound operations
    if super::compound::is_compound(tool_name) {
        let result = super::compound::execute(client, tool_name, payload).await?;
        if let Some(paths) = pick {
            return pick_from_value(&result, paths);
        }
        return Ok(result);
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
/// Multi-field returns a JSON object keyed by each path's last segment.
fn pick_from_value(value: &Value, paths: &str) -> Result<Value> {
    if paths.contains(',') {
        let mut obj = serde_json::Map::new();
        for p in paths.split(',') {
            let p = p.trim();
            let key = p.rsplit('.').next().unwrap_or(p);
            let val = super::resolve_path(value, p)?.clone();
            obj.insert(key.to_string(), val);
        }
        Ok(Value::Object(obj))
    } else {
        super::resolve_path(value, paths).cloned()
    }
}
