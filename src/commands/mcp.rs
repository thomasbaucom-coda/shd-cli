use crate::auth;
use crate::client::CodaClient;
use crate::error::{CodaError, Result};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const PROTOCOL_VERSION: &str = "2024-11-05";

/// All tool names exposed via the MCP server.
/// These map directly to the tool endpoint — any new tool added here
/// is immediately available to MCP clients.
const MCP_TOOLS: &[(&str, &str, &str)] = &[
    // (tool_name, description, required_fields_hint)
    ("whoami", "Get info about the authenticated user", ""),
    ("document_create", "Create a new Coda doc", "title"),
    ("document_delete", "Delete a doc (DESTRUCTIVE)", "docId"),
    ("document_read", "Read full document structure", "docId"),
    ("search", "Search across docs for pages, tables, and rows", "query"),
    ("url_decode", "Decode a Coda URL into resource IDs", "url"),
    ("tool_guide", "Get usage guidance for a topic", "topic"),
    ("page_create", "Create a new page", "docId, title"),
    ("page_read", "Read page content and metadata", "docId"),
    ("page_update", "Update page properties", "docId, pageId, updateFields"),
    ("page_delete", "Delete a page (DESTRUCTIVE)", "docId, pageId"),
    ("page_duplicate", "Duplicate a page with all content", "docId, pageId"),
    ("table_create", "Create a table with typed columns", "docId, canvasId, name, columns"),
    ("table_add_rows", "Add rows to a table (bulk)", "docId, tableId, columns, rows"),
    ("table_add_columns", "Add columns to a table", "docId, tableId, columns"),
    ("table_read_rows", "Read rows from a table", "docId, tableId"),
    ("table_delete", "Delete a table (DESTRUCTIVE)", "docId, tableId"),
    ("table_delete_rows", "Delete rows from a table", "docId, tableId, data"),
    ("table_delete_columns", "Delete columns from a table", "docId, tableId, columnIds"),
    ("table_update_rows", "Update rows in a table", "docId, tableId, rows"),
    ("table_update_columns", "Update column properties", "docId, tableId, columns"),
    ("table_view_configure", "Configure view: filter, layout, name", "docId, tableId, tableViewId"),
    ("content_modify", "Write page content: markdown, callouts, code", "docId, canvasId, operations"),
    ("content_image_upload", "Upload an image to a page", "docId, blobId, imageUrl"),
    ("comment_manage", "Add, reply to, or delete comments", "docId, data"),
    ("formula_create", "Create a named formula", "docId, canvasId, formula"),
    ("formula_execute", "Evaluate a CFL expression", "docId, formula"),
    ("formula_update", "Update a formula", "docId, formulaId, updatedFields"),
    ("formula_delete", "Delete a formula", "docId, formulaId"),
];

pub async fn start() -> Result<()> {
    eprintln!("[coda mcp] Starting MCP server over stdio ({} tools)...", MCP_TOOLS.len());

    let token = auth::resolve_token(None)?;
    let client = CodaClient::new(token)?;

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

                let result = handle_request(method, &params, &client).await;

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

async fn handle_request(method: &str, params: &Value, client: &CodaClient) -> Result<Value> {
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
        "tools/list" => Ok(json!({ "tools": build_tools_list() })),
        "tools/call" => handle_tool_call(params, client).await,
        _ => Err(CodaError::Other(format!("Method not supported: {method}"))),
    }
}

fn build_tools_list() -> Vec<Value> {
    MCP_TOOLS
        .iter()
        .map(|(name, desc, required)| {
            let req_fields: Vec<&str> = if required.is_empty() {
                vec![]
            } else {
                required.split(", ").collect()
            };

            let mut properties = json!({});
            let mut required_arr = vec![];
            for field in &req_fields {
                properties[field] = json!({"type": "string"});
                required_arr.push(json!(field));
            }

            json!({
                "name": name,
                "description": desc,
                "inputSchema": {
                    "type": "object",
                    "properties": properties,
                    "required": required_arr,
                    "additionalProperties": true,
                }
            })
        })
        .collect()
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
