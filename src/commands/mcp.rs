use crate::auth;
use crate::client::CodaClient;
use crate::error::{CodaError, Result};
use crate::output;
use crate::schema_cache;
use crate::trace;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const PROTOCOL_VERSION: &str = "2024-11-05";

pub async fn start() -> Result<()> {
    let token = auth::resolve_token(None)?;
    let client = CodaClient::new(token)?;

    // Fetch tools dynamically from the Coda MCP endpoint
    output::info("[coda mcp] Fetching tools from Coda...\n");
    let mut tools = client.fetch_tools().await?;
    schema_cache::save(&tools)?;

    output::info(&format!(
        "[coda mcp] Starting MCP server over stdio ({} tools)...\n",
        tools.len(),
    ));

    let mut stdin = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();

    while let Ok(Some(line)) = stdin.next_line().await {
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<Value>(&line) {
            Ok(req) => {
                let is_notification = req.get("id").is_none();
                let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
                let params = req.get("params").cloned().unwrap_or(json!({}));

                // On tools/list, refresh if cache is expired
                let result = if method == "tools/list" {
                    match refresh_if_expired(&client, &mut tools).await {
                        Ok(()) => Ok(json!({ "tools": tools })),
                        Err(e) => Err(e),
                    }
                } else {
                    handle_request(method, &params, &client, &tools).await
                };

                if !is_notification {
                    let id = req.get("id").unwrap_or(&Value::Null);
                    let response = match result {
                        Ok(res) => json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": res
                        }),
                        Err(e) => json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": {
                                "code": error_code(&e),
                                "message": e.to_string(),
                                "data": {
                                    "type": e.error_type(),
                                }
                            }
                        }),
                    };

                    let mut out = serde_json::to_string(&response)
                        .unwrap_or_else(|_| r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"Serialization error"}}"#.into());
                    out.push('\n');
                    let _ = stdout.write_all(out.as_bytes()).await;
                    let _ = stdout.flush().await;
                }
            }
            Err(_) => {
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": { "code": -32700, "message": "Parse error" }
                });
                let mut out = serde_json::to_string(&response).unwrap_or_default();
                out.push('\n');
                let _ = stdout.write_all(out.as_bytes()).await;
                let _ = stdout.flush().await;
            }
        }
    }

    Ok(())
}

/// Refresh tools from network if the schema cache is expired.
async fn refresh_if_expired(client: &CodaClient, tools: &mut Vec<Value>) -> Result<()> {
    if schema_cache::load()?.is_none() {
        output::info("[coda mcp] Cache expired, refreshing tools...\n");
        let fresh = client.fetch_tools().await?;
        schema_cache::save(&fresh)?;
        *tools = fresh;
    }
    Ok(())
}

async fn handle_request(
    method: &str,
    params: &Value,
    client: &CodaClient,
    tools: &[Value],
) -> Result<Value> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "serverInfo": {
                "name": "coda-mcp",
                "version": env!("CARGO_PKG_VERSION")
            },
            "capabilities": { "tools": {} }
        })),
        "notifications/initialized" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools })),
        "tools/call" => handle_tool_call(params, client, tools).await,
        _ => Err(CodaError::Other(format!("Method not supported: {method}"))),
    }
}

async fn handle_tool_call(params: &Value, client: &CodaClient, tools: &[Value]) -> Result<Value> {
    let tool_name = params
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or_else(|| CodaError::Validation("Missing 'name' in tools/call".into()))?;
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    // Schema validation from cached tools
    if let Some(tool_schema) = schema_cache::find_tool(tools, tool_name) {
        schema_cache::validate_payload(tool_schema, &args)?;
    }

    trace::emit_request(tool_name, &args);
    let start = std::time::Instant::now();

    let result = client.call_tool(tool_name, args).await;
    let elapsed_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(val) => {
            trace::emit_response(tool_name, &val, elapsed_ms, false);
            Ok(json!({
                "content": [{ "type": "text", "text": serde_json::to_string_pretty(&val).unwrap_or_default() }],
                "isError": false
            }))
        }
        Err(e) => {
            trace::emit_response(
                tool_name,
                &json!({"error": e.to_string()}),
                elapsed_ms,
                true,
            );
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string(&json!({
                        "error": true,
                        "type": e.error_type(),
                        "message": e.to_string(),
                    })).unwrap_or_else(|_| e.to_string())
                }],
                "isError": true
            }))
        }
    }
}

fn error_code(e: &CodaError) -> i32 {
    match e {
        CodaError::Validation(_) => -32602,          // Invalid params
        CodaError::NoToken => -32001,                // Auth required
        CodaError::ContractChanged { .. } => -32002, // Tool changed
        _ => -32603,                                 // Internal error
    }
}
