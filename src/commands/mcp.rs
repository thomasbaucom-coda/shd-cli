use crate::auth;
use crate::client::CodaClient;
use crate::error::{CodaError, Result};
use crate::validate;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const PROTOCOL_VERSION: &str = "2024-11-05";

pub async fn start() -> Result<()> {
    eprintln!("[coda mcp] Starting MCP server over stdio...");

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
                    "error": {
                        "code": -32700,
                        "message": "Parse error"
                    }
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
            "capabilities": {
                "tools": {}
            }
        })),
        "notifications/initialized" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": build_tools_list() })),
        "tools/call" => handle_tool_call(params, client).await,
        _ => Err(CodaError::Other(format!("Method not supported: {method}"))),
    }
}

fn build_tools_list() -> Vec<Value> {
    vec![
        tool_def("coda_whoami", "Get info about the authenticated user", json!({
            "type": "object", "properties": {}
        })),
        tool_def("coda_docs_list", "List accessible Coda docs", json!({
            "type": "object",
            "properties": {
                "limit": { "type": "integer", "description": "Max results to return" },
                "query": { "type": "string", "description": "Search query to filter docs" }
            }
        })),
        tool_def("coda_docs_get", "Get a doc by ID", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string", "description": "Document ID" }
            },
            "required": ["docId"]
        })),
        tool_def("coda_docs_create", "Create a new doc", json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "description": "Doc title" },
                "folderId": { "type": "string", "description": "Folder to create in" }
            },
            "required": ["title"]
        })),
        tool_def("coda_docs_delete", "Delete a doc (DESTRUCTIVE)", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string", "description": "Document ID" }
            },
            "required": ["docId"]
        })),
        tool_def("coda_pages_list", "List pages in a doc", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string", "description": "Document ID" },
                "limit": { "type": "integer", "description": "Max results" }
            },
            "required": ["docId"]
        })),
        tool_def("coda_pages_get", "Get a page by ID", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string", "description": "Document ID" },
                "pageId": { "type": "string", "description": "Page ID" }
            },
            "required": ["docId", "pageId"]
        })),
        tool_def("coda_pages_create", "Create a new page", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string", "description": "Document ID" },
                "name": { "type": "string", "description": "Page name" },
                "subtitle": { "type": "string", "description": "Page subtitle" }
            },
            "required": ["docId", "name"]
        })),
        tool_def("coda_tables_list", "List tables in a doc", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string", "description": "Document ID" },
                "limit": { "type": "integer", "description": "Max results" }
            },
            "required": ["docId"]
        })),
        tool_def("coda_tables_get", "Get table metadata", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string", "description": "Document ID" },
                "tableId": { "type": "string", "description": "Table ID" }
            },
            "required": ["docId", "tableId"]
        })),
        tool_def("coda_columns_list", "List columns in a table", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string", "description": "Document ID" },
                "tableId": { "type": "string", "description": "Table ID" },
                "limit": { "type": "integer", "description": "Max results" }
            },
            "required": ["docId", "tableId"]
        })),
        tool_def("coda_rows_list", "List rows in a table. ALWAYS use fields to limit columns.", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string", "description": "Document ID" },
                "tableId": { "type": "string", "description": "Table ID" },
                "limit": { "type": "integer", "description": "Max rows to return" },
                "query": { "type": "string", "description": "Filter query (column:value)" },
                "sortBy": { "type": "string", "description": "Sort: natural, createdAt, updatedAt" },
                "fields": { "type": "string", "description": "Comma-separated column names to include" }
            },
            "required": ["docId", "tableId"]
        })),
        tool_def("coda_rows_get", "Get a single row by ID", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string", "description": "Document ID" },
                "tableId": { "type": "string", "description": "Table ID" },
                "rowId": { "type": "string", "description": "Row ID" }
            },
            "required": ["docId", "tableId", "rowId"]
        })),
        tool_def("coda_rows_upsert", "Insert or upsert rows into a table", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string", "description": "Document ID" },
                "tableId": { "type": "string", "description": "Table ID" },
                "rows": { "type": "array", "description": "Array of row objects with cells" },
                "keyColumns": { "type": "array", "items": { "type": "string" }, "description": "Key columns for upsert matching" }
            },
            "required": ["docId", "tableId", "rows"]
        })),
        tool_def("coda_rows_update", "Update a single row", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string", "description": "Document ID" },
                "tableId": { "type": "string", "description": "Table ID" },
                "rowId": { "type": "string", "description": "Row ID" },
                "row": { "type": "object", "description": "Row object with cells to update" }
            },
            "required": ["docId", "tableId", "rowId", "row"]
        })),
        tool_def("coda_rows_delete", "Delete a single row (DESTRUCTIVE)", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string", "description": "Document ID" },
                "tableId": { "type": "string", "description": "Table ID" },
                "rowId": { "type": "string", "description": "Row ID" }
            },
            "required": ["docId", "tableId", "rowId"]
        })),
        tool_def("coda_rows_push_button", "Push a button column on a row", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string", "description": "Document ID" },
                "tableId": { "type": "string", "description": "Table ID" },
                "rowId": { "type": "string", "description": "Row ID" },
                "columnId": { "type": "string", "description": "Button column ID" }
            },
            "required": ["docId", "tableId", "rowId", "columnId"]
        })),
        tool_def("coda_formulas_list", "List named formulas in a doc", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string", "description": "Document ID" }
            },
            "required": ["docId"]
        })),
        tool_def("coda_controls_list", "List controls in a doc", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string", "description": "Document ID" }
            },
            "required": ["docId"]
        })),
        tool_def("coda_folders_list", "List folders", json!({
            "type": "object", "properties": {}
        })),
        tool_def("coda_permissions_list", "List permissions on a doc", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string", "description": "Document ID" }
            },
            "required": ["docId"]
        })),
        tool_def("coda_permissions_add", "Add a permission to a doc", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string", "description": "Document ID" },
                "access": { "type": "string", "description": "Access level: readonly, write, comment" },
                "principal": { "type": "object", "description": "Principal: {type, email}" }
            },
            "required": ["docId", "access", "principal"]
        })),
        tool_def("coda_resolve_url", "Decode a Coda URL into structured IDs", json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "Coda URL to decode" }
            },
            "required": ["url"]
        })),
        tool_def("coda_schema", "Inspect API schema for a resource or method (no network call)", json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Schema path: 'list', '<resource>', or '<resource>.<method>'" }
            },
            "required": ["path"]
        })),

        // --- Internal tool endpoint tools (require MCP-scoped token) ---
        // These are dispatched dynamically via client.call_tool()

        tool_def("coda_tool_table_create", "Create a table with typed columns on a page (requires MCP token)", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string" },
                "canvasId": { "type": "string", "description": "Canvas/page ID for table placement" },
                "name": { "type": "string", "description": "Table name" },
                "columns": { "type": "array", "description": "Column definitions [{name, format, isDisplayColumn}]" },
                "rows": { "type": "array", "description": "Initial rows (values in column order)" }
            },
            "required": ["docId", "canvasId", "name", "columns"]
        })),
        tool_def("coda_tool_table_add_rows", "Add rows to a table in bulk (requires MCP token)", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string" },
                "tableId": { "type": "string" },
                "columns": { "type": "array", "description": "Column IDs in order" },
                "rows": { "type": "array", "description": "Rows as arrays of values in column order" }
            },
            "required": ["docId", "tableId", "columns", "rows"]
        })),
        tool_def("coda_tool_table_add_columns", "Add columns to an existing table (requires MCP token)", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string" },
                "tableId": { "type": "string" },
                "columns": { "type": "array", "description": "Column definitions to add" }
            },
            "required": ["docId", "tableId", "columns"]
        })),
        tool_def("coda_tool_table_delete_rows", "Delete rows from a table (requires MCP token)", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string" },
                "tableId": { "type": "string" },
                "data": { "type": "object", "description": "Delete config: {action, rowNumbersOrIds}" }
            },
            "required": ["docId", "tableId", "data"]
        })),
        tool_def("coda_tool_table_update_rows", "Update rows in a table (requires MCP token)", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string" },
                "tableId": { "type": "string" },
                "rows": { "type": "array", "description": "Row updates [{rowNumberOrId, updateCells}]" }
            },
            "required": ["docId", "tableId", "rows"]
        })),
        tool_def("coda_tool_table_delete", "Delete a table (requires MCP token, DESTRUCTIVE)", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string" },
                "tableId": { "type": "string" }
            },
            "required": ["docId", "tableId"]
        })),
        tool_def("coda_tool_content_modify", "Write page content: markdown, callouts, code blocks, images (requires MCP token)", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string" },
                "canvasId": { "type": "string" },
                "operations": { "type": "array", "description": "Content operations [{operation, blockType, content, ...}]" }
            },
            "required": ["docId", "canvasId", "operations"]
        })),
        tool_def("coda_tool_comment_manage", "Add, reply to, or delete comments (requires MCP token)", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string" },
                "data": { "type": "object", "description": "Comment action: {action, content, pageId/tableId/threadId, ...}" }
            },
            "required": ["docId", "data"]
        })),
        tool_def("coda_tool_formula_create", "Create a named formula on a page (requires MCP token)", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string" },
                "canvasId": { "type": "string" },
                "name": { "type": "string" },
                "formula": { "type": "string", "description": "CFL expression" }
            },
            "required": ["docId", "canvasId", "name", "formula"]
        })),
        tool_def("coda_tool_formula_execute", "Evaluate a Coda Formula Language expression (requires MCP token)", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string" },
                "formula": { "type": "string", "description": "CFL expression to evaluate" }
            },
            "required": ["docId", "formula"]
        })),
        tool_def("coda_tool_formula_update", "Update a named formula (requires MCP token)", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string" },
                "formulaId": { "type": "string" },
                "updatedFields": { "type": "object", "description": "Fields to update: {name, formula, format}" }
            },
            "required": ["docId", "formulaId", "updatedFields"]
        })),
        tool_def("coda_tool_formula_delete", "Delete a named formula (requires MCP token)", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string" },
                "formulaId": { "type": "string" }
            },
            "required": ["docId", "formulaId"]
        })),
        tool_def("coda_tool_table_view_configure", "Configure a table view: filter, layout, name (requires MCP token)", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string" },
                "tableId": { "type": "string" },
                "tableViewId": { "type": "string", "description": "View ID (use 'default' for default view)" },
                "name": { "type": "string" },
                "filterFormula": { "type": "string", "description": "CFL filter expression" },
                "viewLayout": { "type": "string", "description": "grid, card, timeline, calendar" }
            },
            "required": ["docId", "tableId", "tableViewId"]
        })),
        tool_def("coda_tool_page_duplicate", "Duplicate a page with all content (requires MCP token)", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string" },
                "pageId": { "type": "string" },
                "newTitle": { "type": "string" }
            },
            "required": ["docId", "pageId"]
        })),
        tool_def("coda_tool_search", "Search across a doc for pages, tables, and rows (requires MCP token)", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string" },
                "query": { "type": "string", "description": "Search query" }
            },
            "required": ["docId", "query"]
        })),
        tool_def("coda_tool_document_read", "Read full document structure (requires MCP token)", json!({
            "type": "object",
            "properties": {
                "docId": { "type": "string" }
            },
            "required": ["docId"]
        })),
    ]
}

fn tool_def(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
    })
}

async fn handle_tool_call(params: &Value, client: &CodaClient) -> Result<Value> {
    let tool_name = params.get("name").and_then(|n| n.as_str())
        .ok_or_else(|| CodaError::Validation("Missing 'name' in tools/call".into()))?;
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    let result = dispatch_tool(tool_name, &args, client).await;

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

async fn dispatch_tool(name: &str, args: &Value, client: &CodaClient) -> Result<Value> {
    let s = |key: &str| -> Result<&str> {
        args.get(key).and_then(|v| v.as_str())
            .ok_or_else(|| CodaError::Validation(format!("Missing required parameter: {key}")))
    };
    let s_opt = |key: &str| -> Option<&str> {
        args.get(key).and_then(|v| v.as_str())
    };
    let i_opt = |key: &str| -> Option<u32> {
        args.get(key).and_then(|v| v.as_u64()).map(|n| n as u32)
    };

    match name {
        "coda_whoami" => {
            let req = client.build_request(reqwest::Method::GET, "/whoami", None, vec![]);
            Ok(client.execute(req).await?.body)
        }

        "coda_docs_list" => {
            let mut params = Vec::new();
            if let Some(l) = i_opt("limit") { params.push(("limit".into(), l.to_string())); }
            if let Some(q) = s_opt("query") { params.push(("query".into(), q.to_string())); }
            let req = client.build_request(reqwest::Method::GET, "/docs", None, params);
            Ok(client.execute(req).await?.body)
        }

        "coda_docs_get" => {
            let doc_id = s("docId")?;
            validate::validate_resource_id(doc_id, "docId")?;
            let path = format!("/docs/{}", validate::encode_path_segment(doc_id));
            let req = client.build_request(reqwest::Method::GET, &path, None, vec![]);
            Ok(client.execute(req).await?.body)
        }

        "coda_docs_create" => {
            let title = s("title")?;
            let mut body = json!({ "title": title });
            if let Some(fid) = s_opt("folderId") {
                body["folderId"] = json!(fid);
            }
            let req = client.build_request(reqwest::Method::POST, "/docs", Some(body), vec![]);
            Ok(client.execute(req).await?.body)
        }

        "coda_docs_delete" => {
            let doc_id = s("docId")?;
            validate::validate_resource_id(doc_id, "docId")?;
            let path = format!("/docs/{}", validate::encode_path_segment(doc_id));
            let req = client.build_request(reqwest::Method::DELETE, &path, None, vec![]);
            client.execute(req).await?;
            Ok(json!({ "deleted": true, "docId": doc_id }))
        }

        "coda_pages_list" => {
            let doc_id = s("docId")?;
            validate::validate_resource_id(doc_id, "docId")?;
            let path = format!("/docs/{}/pages", validate::encode_path_segment(doc_id));
            let mut params = Vec::new();
            if let Some(l) = i_opt("limit") { params.push(("limit".into(), l.to_string())); }
            let req = client.build_request(reqwest::Method::GET, &path, None, params);
            Ok(client.execute(req).await?.body)
        }

        "coda_pages_get" => {
            let doc_id = s("docId")?;
            let page_id = s("pageId")?;
            validate::validate_resource_id(doc_id, "docId")?;
            validate::validate_resource_id(page_id, "pageId")?;
            let path = format!("/docs/{}/pages/{}", validate::encode_path_segment(doc_id), validate::encode_path_segment(page_id));
            let req = client.build_request(reqwest::Method::GET, &path, None, vec![]);
            Ok(client.execute(req).await?.body)
        }

        "coda_pages_create" => {
            let doc_id = s("docId")?;
            let name = s("name")?;
            validate::validate_resource_id(doc_id, "docId")?;
            let path = format!("/docs/{}/pages", validate::encode_path_segment(doc_id));
            let mut body = json!({ "name": name });
            if let Some(sub) = s_opt("subtitle") { body["subtitle"] = json!(sub); }
            let req = client.build_request(reqwest::Method::POST, &path, Some(body), vec![]);
            Ok(client.execute(req).await?.body)
        }

        "coda_tables_list" => {
            let doc_id = s("docId")?;
            validate::validate_resource_id(doc_id, "docId")?;
            let path = format!("/docs/{}/tables", validate::encode_path_segment(doc_id));
            let mut params = Vec::new();
            if let Some(l) = i_opt("limit") { params.push(("limit".into(), l.to_string())); }
            let req = client.build_request(reqwest::Method::GET, &path, None, params);
            Ok(client.execute(req).await?.body)
        }

        "coda_tables_get" => {
            let doc_id = s("docId")?;
            let table_id = s("tableId")?;
            validate::validate_resource_id(doc_id, "docId")?;
            validate::validate_resource_id(table_id, "tableId")?;
            let path = format!("/docs/{}/tables/{}", validate::encode_path_segment(doc_id), validate::encode_path_segment(table_id));
            let req = client.build_request(reqwest::Method::GET, &path, None, vec![]);
            Ok(client.execute(req).await?.body)
        }

        "coda_columns_list" => {
            let doc_id = s("docId")?;
            let table_id = s("tableId")?;
            validate::validate_resource_id(doc_id, "docId")?;
            validate::validate_resource_id(table_id, "tableId")?;
            let path = format!("/docs/{}/tables/{}/columns", validate::encode_path_segment(doc_id), validate::encode_path_segment(table_id));
            let mut params = Vec::new();
            if let Some(l) = i_opt("limit") { params.push(("limit".into(), l.to_string())); }
            let req = client.build_request(reqwest::Method::GET, &path, None, params);
            Ok(client.execute(req).await?.body)
        }

        "coda_rows_list" => {
            let doc_id = s("docId")?;
            let table_id = s("tableId")?;
            validate::validate_resource_id(doc_id, "docId")?;
            validate::validate_resource_id(table_id, "tableId")?;
            let path = format!("/docs/{}/tables/{}/rows", validate::encode_path_segment(doc_id), validate::encode_path_segment(table_id));
            let mut params = vec![
                ("useColumnNames".into(), "true".into()),
                ("valueFormat".into(), "simpleWithArrays".into()),
            ];
            if let Some(l) = i_opt("limit") { params.push(("limit".into(), l.to_string())); }
            if let Some(q) = s_opt("query") { params.push(("query".into(), q.to_string())); }
            if let Some(sb) = s_opt("sortBy") { params.push(("sortBy".into(), sb.to_string())); }
            let req = client.build_request(reqwest::Method::GET, &path, None, params);
            let mut resp = client.execute(req).await?.body;
            // Apply field filtering
            if let Some(fields) = s_opt("fields") {
                filter_row_fields(&mut resp, fields);
            }
            Ok(resp)
        }

        "coda_rows_get" => {
            let doc_id = s("docId")?;
            let table_id = s("tableId")?;
            let row_id = s("rowId")?;
            validate::validate_resource_id(doc_id, "docId")?;
            validate::validate_resource_id(table_id, "tableId")?;
            validate::validate_resource_id(row_id, "rowId")?;
            let path = format!("/docs/{}/tables/{}/rows/{}", validate::encode_path_segment(doc_id), validate::encode_path_segment(table_id), validate::encode_path_segment(row_id));
            let params = vec![
                ("useColumnNames".into(), "true".into()),
                ("valueFormat".into(), "simpleWithArrays".into()),
            ];
            let req = client.build_request(reqwest::Method::GET, &path, None, params);
            Ok(client.execute(req).await?.body)
        }

        "coda_rows_upsert" => {
            let doc_id = s("docId")?;
            let table_id = s("tableId")?;
            validate::validate_resource_id(doc_id, "docId")?;
            validate::validate_resource_id(table_id, "tableId")?;
            let path = format!("/docs/{}/tables/{}/rows", validate::encode_path_segment(doc_id), validate::encode_path_segment(table_id));
            let mut body = json!({});
            if let Some(rows) = args.get("rows") { body["rows"] = rows.clone(); }
            if let Some(keys) = args.get("keyColumns") { body["keyColumns"] = keys.clone(); }
            let req = client.build_request(reqwest::Method::POST, &path, Some(body), vec![]);
            Ok(client.execute(req).await?.body)
        }

        "coda_rows_update" => {
            let doc_id = s("docId")?;
            let table_id = s("tableId")?;
            let row_id = s("rowId")?;
            validate::validate_resource_id(doc_id, "docId")?;
            validate::validate_resource_id(table_id, "tableId")?;
            validate::validate_resource_id(row_id, "rowId")?;
            let path = format!("/docs/{}/tables/{}/rows/{}", validate::encode_path_segment(doc_id), validate::encode_path_segment(table_id), validate::encode_path_segment(row_id));
            let body = args.get("row").cloned().unwrap_or(json!({}));
            let req = client.build_request(reqwest::Method::PUT, &path, Some(body), vec![]);
            Ok(client.execute(req).await?.body)
        }

        "coda_rows_delete" => {
            let doc_id = s("docId")?;
            let table_id = s("tableId")?;
            let row_id = s("rowId")?;
            validate::validate_resource_id(doc_id, "docId")?;
            validate::validate_resource_id(table_id, "tableId")?;
            validate::validate_resource_id(row_id, "rowId")?;
            let path = format!("/docs/{}/tables/{}/rows/{}", validate::encode_path_segment(doc_id), validate::encode_path_segment(table_id), validate::encode_path_segment(row_id));
            let req = client.build_request(reqwest::Method::DELETE, &path, None, vec![]);
            client.execute(req).await?;
            Ok(json!({ "deleted": true, "rowId": row_id }))
        }

        "coda_rows_push_button" => {
            let doc_id = s("docId")?;
            let table_id = s("tableId")?;
            let row_id = s("rowId")?;
            let column_id = s("columnId")?;
            validate::validate_resource_id(doc_id, "docId")?;
            validate::validate_resource_id(table_id, "tableId")?;
            validate::validate_resource_id(row_id, "rowId")?;
            validate::validate_resource_id(column_id, "columnId")?;
            let path = format!("/docs/{}/tables/{}/rows/{}/buttons/{}", validate::encode_path_segment(doc_id), validate::encode_path_segment(table_id), validate::encode_path_segment(row_id), validate::encode_path_segment(column_id));
            let req = client.build_request(reqwest::Method::POST, &path, None, vec![]);
            Ok(client.execute(req).await?.body)
        }

        "coda_formulas_list" => {
            let doc_id = s("docId")?;
            validate::validate_resource_id(doc_id, "docId")?;
            let path = format!("/docs/{}/formulas", validate::encode_path_segment(doc_id));
            let req = client.build_request(reqwest::Method::GET, &path, None, vec![]);
            Ok(client.execute(req).await?.body)
        }

        "coda_controls_list" => {
            let doc_id = s("docId")?;
            validate::validate_resource_id(doc_id, "docId")?;
            let path = format!("/docs/{}/controls", validate::encode_path_segment(doc_id));
            let req = client.build_request(reqwest::Method::GET, &path, None, vec![]);
            Ok(client.execute(req).await?.body)
        }

        "coda_folders_list" => {
            let req = client.build_request(reqwest::Method::GET, "/folders", None, vec![]);
            Ok(client.execute(req).await?.body)
        }

        "coda_permissions_list" => {
            let doc_id = s("docId")?;
            validate::validate_resource_id(doc_id, "docId")?;
            let path = format!("/docs/{}/acl/permissions", validate::encode_path_segment(doc_id));
            let req = client.build_request(reqwest::Method::GET, &path, None, vec![]);
            Ok(client.execute(req).await?.body)
        }

        "coda_permissions_add" => {
            let doc_id = s("docId")?;
            let access = s("access")?;
            validate::validate_resource_id(doc_id, "docId")?;
            let path = format!("/docs/{}/acl/permissions", validate::encode_path_segment(doc_id));
            let mut body = json!({ "access": access });
            if let Some(principal) = args.get("principal") { body["principal"] = principal.clone(); }
            let req = client.build_request(reqwest::Method::POST, &path, Some(body), vec![]);
            Ok(client.execute(req).await?.body)
        }

        "coda_resolve_url" => {
            let url = s("url")?;
            let params = vec![("url".into(), url.to_string())];
            let req = client.build_request(reqwest::Method::GET, "/resolveBrowserLink", None, params);
            Ok(client.execute(req).await?.body)
        }

        "coda_schema" => {
            let path = s("path")?;
            // Capture schema output by redirecting to a string
            let output = capture_schema_output(path)?;
            Ok(json!({ "schema": serde_json::from_str::<Value>(&output).unwrap_or(json!(output)) }))
        }

        // Dynamic dispatch: any coda_tool_* name routes to the internal tool endpoint
        _ if name.starts_with("coda_tool_") => {
            let tool_name = &name["coda_tool_".len()..];
            let doc_id = s("docId")?;
            validate::validate_resource_id(doc_id, "docId")?;
            Ok(client.call_tool(doc_id, tool_name, args.clone()).await?)
        }

        _ => Err(CodaError::Other(format!("Unknown tool: {name}"))),
    }
}

fn filter_row_fields(response: &mut Value, field_list: &str) {
    let fields: Vec<&str> = field_list.split(',').map(|s| s.trim()).collect();
    if let Some(items) = response.get_mut("items").and_then(|v| v.as_array_mut()) {
        for item in items {
            if let Some(values) = item.get_mut("values").and_then(|v| v.as_object_mut()) {
                let keys: Vec<String> = values.keys().cloned().collect();
                for key in keys {
                    if !fields.iter().any(|f| f.eq_ignore_ascii_case(&key)) {
                        values.remove(&key);
                    }
                }
            }
        }
    }
}

fn capture_schema_output(path: &str) -> Result<String> {
    // Reuse the schema module's lookup logic via its public handle function,
    // but we can't capture stdout easily. Instead, do a direct lookup.
    let spec_str = include_str!("../../openapi/v1.json");
    let spec: Value = serde_json::from_str(spec_str)
        .map_err(|e| CodaError::Other(format!("Failed to parse OpenAPI spec: {e}")))?;

    let parts: Vec<&str> = path.split('.').collect();
    if parts.len() == 2 {
        let resource = parts[0];
        let method_name = parts[1];
        let target_op = super::schema::build_operation_id(resource, method_name);

        if let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) {
            for (_path, path_item) in paths {
                if let Some(obj) = path_item.as_object() {
                    for (_, operation) in obj {
                        let op_id = operation.get("operationId").and_then(|v| v.as_str()).unwrap_or("");
                        if op_id.eq_ignore_ascii_case(&target_op) {
                            return Ok(serde_json::to_string_pretty(operation).unwrap_or_default());
                        }
                    }
                }
            }
        }
    }

    Ok(format!("{{\"error\": \"Schema not found for '{}'\"}}", path))
}
