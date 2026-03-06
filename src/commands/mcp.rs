use crate::auth;
use crate::client::CodaClient;
use crate::error::{CodaError, Result};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const PROTOCOL_VERSION: &str = "2024-11-05";

pub async fn start() -> Result<()> {
    let token = auth::resolve_token(None)?;
    let client = CodaClient::new(token)?;

    // Fetch tools dynamically from the Coda MCP endpoint
    eprintln!("[coda mcp] Fetching tools from Coda...");
    let tools = client.fetch_tools().await?;
    eprintln!("[coda mcp] Starting MCP server over stdio ({} tools)...", tools.len());

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

                let result = handle_request(method, &params, &client, &tools).await;

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
                                "code": -32603,
                                "message": e.to_string()
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

async fn handle_request(method: &str, params: &Value, client: &CodaClient, tools: &[Value]) -> Result<Value> {
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
        "tools/call" => handle_tool_call(params, client).await,
        _ => Err(CodaError::Other(format!("Method not supported: {method}"))),
    }
}

async fn handle_tool_call(params: &Value, client: &CodaClient) -> Result<Value> {
    let tool_name = params.get("name").and_then(|n| n.as_str())
        .ok_or_else(|| CodaError::Validation("Missing 'name' in tools/call".into()))?;
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    // All tools dispatch through the same endpoint
    let result = client.call_tool(tool_name, args).await;

    match result {
        Ok(val) => Ok(json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&val).unwrap_or_default() }],
            "isError": false
        })),
        Err(e) => Ok(json!({
            "content": [{ "type": "text", "text": e.to_string() }],
            "isError": true
        })),
    }
}
