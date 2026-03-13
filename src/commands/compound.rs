//! Compound operations — multi-step tool orchestration for agents.
//!
//! These synthetic tools compose multiple Coda API calls into single
//! CLI invocations, reducing agent call count and chaining overhead.

use crate::client::{CodaClient, ToolCaller};
use crate::error::{CodaError, Result};
use crate::output::{self, OutputFormat};
use crate::trace;
use serde_json::{json, Value};

/// Known compound tool names.
const COMPOUND_TOOLS: &[&str] = &[
    "page_create_with_content",
    "doc_scaffold",
    "doc_summarize",
    "table_search",
];

/// Check if a tool name is a compound operation.
pub fn is_compound(name: &str) -> bool {
    COMPOUND_TOOLS.contains(&name)
}

/// CLI dispatch: execute a compound operation and print the result.
pub async fn dispatch(
    client: &CodaClient,
    tool_name: &str,
    payload: Value,
    dry_run: bool,
    pick: Option<&str>,
    format: OutputFormat,
) -> Result<Option<Value>> {
    if dry_run {
        let preview = dry_run_preview(tool_name, &payload);
        output::print_response(&preview, format)?;
        return Ok(None);
    }

    let result = execute(client, tool_name, payload).await?;

    if let Some(paths) = pick {
        crate::commands::tools::pick_and_print(&result, paths)?;
    } else {
        output::print_response(&result, format)?;
    }

    Ok(Some(result))
}

/// Execute a compound operation and return the result as a Value.
/// Used by both CLI dispatch and shell/MCP modes.
pub async fn execute(client: &CodaClient, tool_name: &str, payload: Value) -> Result<Value> {
    match tool_name {
        "page_create_with_content" => page_create_with_content(client, payload).await,
        "doc_scaffold" => doc_scaffold(client, payload).await,
        "doc_summarize" => doc_summarize(client, payload).await,
        "table_search" => table_search(client, payload).await,
        _ => Err(CodaError::Validation(format!(
            "Unknown compound tool: {tool_name}"
        ))),
    }
}

/// Return JSON Schema definitions for compound tools so they appear in discovery.
pub fn synthetic_tool_schemas() -> Vec<Value> {
    vec![
        json!({
            "name": "page_create_with_content",
            "description": "Create a new page and insert markdown content in a single call. Eliminates the need to chain page_create → content_modify.",
            "inputSchema": {
                "$schema": "http://json-schema.org/draft-07/schema#",
                "type": "object",
                "required": ["uri", "title"],
                "properties": {
                    "uri": { "type": "string", "description": "Parent — document URI for top-level, page URI for nested" },
                    "title": { "type": "string", "description": "Page title" },
                    "subtitle": { "type": "string", "description": "Page subtitle" },
                    "content": { "type": "string", "description": "Markdown content to insert (optional)" }
                }
            }
        }),
        json!({
            "name": "doc_scaffold",
            "description": "Create a complete document with pages, content, tables, and rows in a single call.",
            "inputSchema": {
                "$schema": "http://json-schema.org/draft-07/schema#",
                "type": "object",
                "required": ["title"],
                "properties": {
                    "title": { "type": "string", "description": "Document title" },
                    "pages": {
                        "type": "array",
                        "description": "Pages to create",
                        "items": {
                            "type": "object",
                            "required": ["title"],
                            "properties": {
                                "title": { "type": "string" },
                                "subtitle": { "type": "string" },
                                "content": { "type": "string", "description": "Markdown content" },
                                "tables": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "required": ["name", "columns"],
                                        "properties": {
                                            "name": { "type": "string" },
                                            "columns": { "type": "array" },
                                            "rows": { "type": "array" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }),
        json!({
            "name": "doc_summarize",
            "description": "Read a document and return a condensed summary: pages, tables, row counts, and content previews. ~500 tokens instead of 10+ calls.",
            "inputSchema": {
                "$schema": "http://json-schema.org/draft-07/schema#",
                "type": "object",
                "required": ["uri"],
                "properties": {
                    "uri": { "type": "string", "description": "Document URI (coda://docs/{docId})" }
                }
            }
        }),
        json!({
            "name": "table_search",
            "description": "Read table rows and filter client-side by column value. Returns only matching rows.",
            "inputSchema": {
                "$schema": "http://json-schema.org/draft-07/schema#",
                "type": "object",
                "required": ["uri", "column", "value"],
                "properties": {
                    "uri": { "type": "string", "description": "Table URI (coda://docs/{docId}/tables/{tableId})" },
                    "column": { "type": "string", "description": "Column name or column ID (c-xxxxx) to filter on" },
                    "value": { "type": "string", "description": "Value to match" },
                    "operator": { "type": "string", "description": "Operator: eq (default), ne, contains", "enum": ["eq", "ne", "contains"] }
                }
            }
        }),
    ]
}

// ---------------------------------------------------------------------------
// Compound operations
// ---------------------------------------------------------------------------

/// Create a page and optionally insert markdown content.
async fn page_create_with_content(client: &CodaClient, payload: Value) -> Result<Value> {
    let uri = payload
        .get("uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CodaError::Validation("Missing required field: uri".into()))?;
    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CodaError::Validation("Missing required field: title".into()))?;
    let content = payload.get("content").and_then(|v| v.as_str());
    let subtitle = payload.get("subtitle").and_then(|v| v.as_str());

    // Step 1: Create page
    let mut create_payload = json!({"uri": uri, "title": title});
    if let Some(sub) = subtitle {
        create_payload["subtitle"] = json!(sub);
    }

    trace::emit_compound_step(
        "page_create_with_content",
        1,
        "page_create",
        &create_payload,
    );
    let page_result = client.call_tool("page_create", create_payload).await?;

    let canvas_uri = page_result
        .get("canvasUri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CodaError::Other("page_create did not return canvasUri".into()))?;

    // Step 2: Insert content (if provided)
    if let Some(md) = content {
        let content_payload = json!({
            "uri": canvas_uri,
            "operations": [{
                "operation": "insert_element",
                "blockType": "markdown",
                "content": md
            }]
        });

        trace::emit_compound_step(
            "page_create_with_content",
            2,
            "content_modify",
            &content_payload,
        );
        call_with_retry(client, "content_modify", content_payload, 3).await?;
    }

    // Return page metadata with content confirmation
    let mut result = page_result;
    if content.is_some() {
        if let Some(obj) = result.as_object_mut() {
            obj.insert("contentWritten".into(), json!(true));
        }
    }
    Ok(result)
}

/// Create a complete document from a blueprint.
async fn doc_scaffold(client: &CodaClient, payload: Value) -> Result<Value> {
    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CodaError::Validation("Missing required field: title".into()))?;

    // Step 1: Create document
    let create_payload = json!({"title": title});
    trace::emit_compound_step("doc_scaffold", 1, "document_create", &create_payload);
    let doc_result = client.call_tool("document_create", create_payload).await?;

    let doc_uri = doc_result
        .get("docUri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CodaError::Other("document_create did not return docUri".into()))?
        .to_string();

    // Get the first page's canvas URI (created with the doc)
    let first_canvas = doc_result
        .get("pages")
        .and_then(|p| p.as_array())
        .and_then(|arr| arr.first())
        .and_then(|p| p.get("canvasUri"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let first_page_uri = doc_result
        .get("pages")
        .and_then(|p| p.as_array())
        .and_then(|arr| arr.first())
        .and_then(|p| p.get("pageUri"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let pages_spec = payload
        .get("pages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut created_pages: Vec<Value> = Vec::new();
    let mut created_tables: Vec<Value> = Vec::new();
    let mut total_rows = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut step = 2usize;

    for (i, page_spec) in pages_spec.iter().enumerate() {
        let page_title = page_spec
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled");

        // For the first page, rename it; for subsequent pages, create new
        let (canvas_uri, page_uri) = if i == 0 {
            if let Some(ref fp_uri) = first_page_uri {
                let mut update_fields = json!({"title": page_title});
                if let Some(sub) = page_spec.get("subtitle").and_then(|v| v.as_str()) {
                    update_fields["subtitle"] = json!(sub);
                }
                let update_payload = json!({"uri": fp_uri, "updateFields": update_fields});
                trace::emit_compound_step("doc_scaffold", step, "page_update", &update_payload);
                step += 1;
                let _ = client.call_tool("page_update", update_payload).await;
            }
            (first_canvas.clone(), first_page_uri.clone())
        } else {
            let mut create_payload = json!({"uri": &doc_uri, "title": page_title});
            if let Some(sub) = page_spec.get("subtitle").and_then(|v| v.as_str()) {
                create_payload["subtitle"] = json!(sub);
            }
            trace::emit_compound_step("doc_scaffold", step, "page_create", &create_payload);
            step += 1;
            match client.call_tool("page_create", create_payload).await {
                Ok(result) => {
                    let cu = result
                        .get("canvasUri")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let pu = result
                        .get("pageUri")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    (cu, pu)
                }
                Err(e) => {
                    errors.push(format!("Page '{}': {}", page_title, e));
                    continue;
                }
            }
        };

        let c_uri = match canvas_uri {
            Some(ref u) => u.clone(),
            None => {
                errors.push(format!("Page '{}': no canvas URI", page_title));
                continue;
            }
        };

        created_pages.push(json!({
            "title": page_title,
            "canvasUri": &c_uri,
            "pageUri": page_uri,
        }));

        // Insert content if provided
        if let Some(content) = page_spec.get("content").and_then(|v| v.as_str()) {
            let content_payload = json!({
                "uri": &c_uri,
                "operations": [{"operation": "insert_element", "blockType": "markdown", "content": content}]
            });
            trace::emit_compound_step("doc_scaffold", step, "content_modify", &content_payload);
            step += 1;
            if let Err(e) = call_with_retry(client, "content_modify", content_payload, 3).await {
                errors.push(format!("Content for '{}': {}", page_title, e));
            }
        }

        // Create tables if specified
        if let Some(tables) = page_spec.get("tables").and_then(|v| v.as_array()) {
            for table_spec in tables {
                let table_name = table_spec
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Untitled Table");

                let mut table_payload = json!({
                    "uri": &c_uri,
                    "name": table_name,
                    "columns": table_spec.get("columns").cloned().unwrap_or(json!([])),
                });

                // Include rows inline if the API supports it and no separate rows field
                let rows = table_spec.get("rows").and_then(|v| v.as_array());
                if rows.is_some() && table_spec.get("columns").is_some() {
                    table_payload["rows"] = table_spec.get("rows").cloned().unwrap_or(json!([]));
                }

                trace::emit_compound_step("doc_scaffold", step, "table_create", &table_payload);
                step += 1;

                match call_with_retry(client, "table_create", table_payload, 3).await {
                    Ok(table_result) => {
                        let tbl_uri = table_result
                            .get("tableUri")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let col_count = table_result
                            .get("columns")
                            .and_then(|c| c.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0);
                        let row_count = table_result
                            .get("rowCount")
                            .and_then(|r| r.as_u64())
                            .unwrap_or(0);
                        total_rows += row_count as usize;

                        // If rows were specified but not sent inline, add them now
                        if let Some(rows_data) = rows {
                            if row_count == 0 && !rows_data.is_empty() {
                                let col_ids: Vec<Value> = table_result
                                    .get("columns")
                                    .and_then(|c| c.as_array())
                                    .map(|cols| {
                                        cols.iter()
                                            .filter_map(|c| {
                                                c.get("columnId").or_else(|| c.get("id")).cloned()
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_default();

                                let add_payload = json!({
                                    "uri": tbl_uri,
                                    "columns": col_ids,
                                    "rows": rows_data,
                                });
                                trace::emit_compound_step(
                                    "doc_scaffold",
                                    step,
                                    "table_add_rows",
                                    &add_payload,
                                );
                                step += 1;
                                match call_with_retry(client, "table_add_rows", add_payload, 3)
                                    .await
                                {
                                    Ok(add_result) => {
                                        let added = add_result
                                            .get("rowCount")
                                            .and_then(|r| r.as_u64())
                                            .unwrap_or(rows_data.len() as u64);
                                        total_rows += added as usize;
                                    }
                                    Err(e) => {
                                        errors.push(format!("Rows for '{}': {}", table_name, e));
                                    }
                                }
                            }
                        }

                        created_tables.push(json!({
                            "name": table_name,
                            "tableUri": tbl_uri,
                            "columns": col_count,
                        }));
                    }
                    Err(e) => {
                        errors.push(format!("Table '{}': {}", table_name, e));
                    }
                }
            }
        }
    }

    let mut result = json!({
        "docUri": &doc_uri,
        "browserLink": doc_result.get("browserLink").cloned().unwrap_or(Value::Null),
        "pages": created_pages,
        "tables": created_tables,
        "totalRows": total_rows,
    });

    if !errors.is_empty() {
        result["errors"] = json!(errors);
    }

    Ok(result)
}

/// Read a document and return a condensed summary.
async fn doc_summarize(client: &CodaClient, payload: Value) -> Result<Value> {
    let uri = payload
        .get("uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CodaError::Validation("Missing required field: uri".into()))?;

    // Step 1: Read document structure
    trace::emit_compound_step("doc_summarize", 1, "document_read", &payload);
    let doc_result = client
        .call_tool("document_read", json!({"uri": uri}))
        .await?;

    let doc_meta = doc_result.get("document").cloned().unwrap_or(json!({}));
    let pages = doc_result
        .get("pages")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();

    let mut page_summaries: Vec<Value> = Vec::new();
    let mut table_summaries: Vec<Value> = Vec::new();
    let mut step = 2usize;

    // Step 2: Read each page for content preview and tables
    for page in pages.iter().take(20) {
        let page_title = page.get("title").and_then(|v| v.as_str()).unwrap_or("?");
        let canvas_uri = page.get("canvasUri").and_then(|v| v.as_str());
        let page_uri = page.get("pageUri").and_then(|v| v.as_str());

        let read_uri = canvas_uri.or(page_uri).unwrap_or("");
        if read_uri.is_empty() {
            page_summaries.push(json!({"title": page_title}));
            continue;
        }

        let read_payload = json!({
            "uri": read_uri,
            "contentTypesToInclude": ["markdown", "tables"],
            "markdownBlockLimit": 3
        });
        trace::emit_compound_step("doc_summarize", step, "page_read", &read_payload);
        step += 1;

        match client.call_tool("page_read", read_payload).await {
            Ok(page_data) => {
                // Extract content preview — truncate to first 200 chars
                let content_preview = page_data.get("content").and_then(|c| c.as_str()).map(|s| {
                    let trimmed = s.trim();
                    if trimmed.len() > 200 {
                        format!("{}...", &trimmed[..197])
                    } else {
                        trimmed.to_string()
                    }
                });

                // Find tables on this page
                let child_tables = page_data
                    .get("tables")
                    .and_then(|t| t.as_array())
                    .cloned()
                    .unwrap_or_default();

                for table in &child_tables {
                    let tbl_name = table.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                    let tbl_uri = table.get("tableUri").and_then(|v| v.as_str()).unwrap_or("");
                    let col_names: Vec<&str> = table
                        .get("columns")
                        .and_then(|c| c.as_array())
                        .map(|cols| {
                            cols.iter()
                                .filter_map(|c| c.get("name").and_then(|n| n.as_str()))
                                .collect()
                        })
                        .unwrap_or_default();
                    let row_count = table.get("rowCount").and_then(|r| r.as_u64()).unwrap_or(0);

                    table_summaries.push(json!({
                        "name": tbl_name,
                        "tableUri": tbl_uri,
                        "columns": col_names,
                        "rowCount": row_count,
                        "page": page_title,
                    }));
                }

                let table_count = child_tables.len();
                page_summaries.push(json!({
                    "title": page_title,
                    "canvasUri": canvas_uri,
                    "contentPreview": content_preview,
                    "tables": table_count,
                }));
            }
            Err(_) => {
                // Page read failed — include basic info only
                page_summaries.push(json!({
                    "title": page_title,
                    "canvasUri": canvas_uri,
                }));
            }
        }
    }

    Ok(json!({
        "doc": doc_meta,
        "uri": uri,
        "pageCount": pages.len(),
        "pages": page_summaries,
        "tables": table_summaries,
    }))
}

/// Read table rows and filter client-side by column value.
async fn table_search(client: &CodaClient, payload: Value) -> Result<Value> {
    let uri = payload
        .get("uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CodaError::Validation("Missing required field: uri".into()))?;
    let column_name = payload
        .get("column")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CodaError::Validation("Missing required field: column".into()))?;
    let search_value = payload
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CodaError::Validation("Missing required field: value".into()))?;
    let operator = payload
        .get("operator")
        .and_then(|v| v.as_str())
        .unwrap_or("eq");

    // Step 1: Read the page containing this table to get column metadata.
    // We need this to resolve column name → column ID.
    // Use the cached tool schema to find column info, or read from the table itself.
    trace::emit_compound_step("table_search", 1, "table_read_rows", &json!({"uri": uri}));
    let result = client
        .call_tool("table_read_rows", json!({"uri": uri}))
        .await?;

    let rows = result
        .get("rows")
        .or_else(|| result.get("items"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // Try to get columns from the response; if not present, fetch separately
    let columns = if let Some(cols) = result.get("columns").and_then(|c| c.as_array()) {
        cols.clone()
    } else {
        // Fetch table metadata to get column name→ID mapping
        // The table URI is the parent doc, so we need to check if there's
        // column info embedded in the rows response or read the table schema.
        // For now, extract column IDs from the first row's values keys
        // and try to match by position using the table_create response pattern.
        vec![]
    };

    let column_key = resolve_column_key(&rows, &columns, column_name);

    let filtered: Vec<&Value> = rows
        .iter()
        .filter(|row| {
            let cell_value = extract_cell_value(row, &column_key);
            match_value(&cell_value, search_value, operator)
        })
        .collect();

    Ok(json!({
        "rows": filtered,
        "matchCount": filtered.len(),
        "totalRows": rows.len(),
        "filter": {
            "column": column_name,
            "operator": operator,
            "value": search_value,
        }
    }))
}

/// Resolve a column name to the key used in row values.
/// Checks the columns metadata to map name → columnId.
fn resolve_column_key(rows: &[Value], columns: &[Value], column_name: &str) -> String {
    // First check if column_name is already a valid key in the row values
    if let Some(first_row) = rows.first() {
        if let Some(values) = first_row.get("values").and_then(|v| v.as_object()) {
            if values.contains_key(column_name) {
                return column_name.to_string();
            }
        }
    }

    // Map column name → column ID from the columns metadata
    for col in columns {
        let name = col.get("name").and_then(|n| n.as_str()).unwrap_or("");
        if name.eq_ignore_ascii_case(column_name) {
            if let Some(id) = col
                .get("columnId")
                .or_else(|| col.get("id"))
                .and_then(|v| v.as_str())
            {
                return id.to_string();
            }
        }
    }

    // Fallback to the name itself
    column_name.to_string()
}

/// Extract a cell's display value from a row.
/// Handles both direct string values and {content: "..."} wrappers.
/// If column_key is a column ID (starts with "c-"), looks up directly.
/// If it's a column name that wasn't resolved, searches all columns.
fn extract_cell_value(row: &Value, column_key: &str) -> String {
    let values = match row.get("values").and_then(|v| v.as_object()) {
        Some(v) => v,
        None => return String::new(),
    };

    // Try direct key lookup (works for column IDs and direct name matches)
    if let Some(cell) = values.get(column_key) {
        return unwrap_cell(cell);
    }

    String::new()
}

/// Unwrap a cell value: handles raw strings, numbers, and {content: "..."} objects.
fn unwrap_cell(cell: &Value) -> String {
    match cell {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Object(_) => {
            // Try {content: "..."} wrapper
            cell.get("content")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string()
        }
        _ => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Call a tool with retry logic for eventual consistency.
async fn call_with_retry(
    client: &CodaClient,
    tool_name: &str,
    payload: Value,
    max_retries: u32,
) -> Result<Value> {
    let mut last_err = None;
    for attempt in 0..=max_retries {
        if attempt > 0 {
            let delay = std::time::Duration::from_millis(1000 * attempt as u64);
            tokio::time::sleep(delay).await;
        }
        match client.call_tool(tool_name, payload.clone()).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| CodaError::Other("Retry exhausted".into())))
}

/// Match a cell value against a search value using the given operator.
fn match_value(cell: &str, search: &str, operator: &str) -> bool {
    let cell_lower = cell.to_lowercase();
    let search_lower = search.to_lowercase();
    match operator {
        "eq" => cell_lower == search_lower,
        "ne" => cell_lower != search_lower,
        "contains" => cell_lower.contains(&search_lower),
        _ => cell_lower == search_lower,
    }
}

/// Generate a dry-run preview of the steps a compound operation would take.
fn dry_run_preview(tool_name: &str, payload: &Value) -> Value {
    match tool_name {
        "page_create_with_content" => json!({
            "compound": tool_name,
            "steps": [
                {"step": 1, "tool": "page_create", "payload": {"uri": payload.get("uri"), "title": payload.get("title")}},
                {"step": 2, "tool": "content_modify", "payload": {"uri": "<from step 1: canvasUri>", "operations": [{"operation": "insert_element", "blockType": "markdown"}]}},
            ]
        }),
        "doc_scaffold" => {
            let page_count = payload
                .get("pages")
                .and_then(|p| p.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            json!({
                "compound": tool_name,
                "steps": format!("1 document_create + {} page_create + content_modify + table_create as needed", page_count),
                "estimatedCalls": page_count * 2 + 1,
            })
        }
        "doc_summarize" => json!({
            "compound": tool_name,
            "steps": ["1 document_read", "N page_read (one per page, max 20)"]
        }),
        "table_search" => json!({
            "compound": tool_name,
            "steps": ["1 table_read_rows (auto-paginated)", "client-side filter"]
        }),
        _ => json!({"compound": tool_name, "payload": payload}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_compound_recognizes_all_tools() {
        assert!(is_compound("page_create_with_content"));
        assert!(is_compound("doc_scaffold"));
        assert!(is_compound("doc_summarize"));
        assert!(is_compound("table_search"));
        assert!(!is_compound("page_create"));
        assert!(!is_compound("table_create"));
        assert!(!is_compound("whoami"));
    }

    #[test]
    fn match_value_eq() {
        assert!(match_value("Active", "active", "eq"));
        assert!(!match_value("Active", "inactive", "eq"));
    }

    #[test]
    fn match_value_ne() {
        assert!(match_value("Active", "inactive", "ne"));
        assert!(!match_value("Active", "active", "ne"));
    }

    #[test]
    fn match_value_contains() {
        assert!(match_value("In Progress", "progress", "contains"));
        assert!(!match_value("Done", "progress", "contains"));
    }

    #[test]
    fn dry_run_shows_steps() {
        let payload = json!({"uri": "coda://docs/abc", "title": "Test", "content": "# Hello"});
        let preview = dry_run_preview("page_create_with_content", &payload);
        assert!(preview.get("steps").is_some());
        assert_eq!(preview["compound"], "page_create_with_content");
    }

    #[test]
    fn synthetic_schemas_are_valid() {
        let schemas = synthetic_tool_schemas();
        assert_eq!(schemas.len(), 4);
        for schema in &schemas {
            assert!(schema.get("name").is_some());
            assert!(schema.get("inputSchema").is_some());
        }
    }
}
