use crate::client::CodaClient;
use crate::error::{CodaError, Result};
use crate::output::{self, OutputFormat};
use crate::validate;
use serde_json::{json, Value};

/// Discover available tools from the server via tool_guide.
pub async fn list_tools(
    client: &CodaClient,
    doc_id: &str,
    topic: Option<&str>,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;

    let topic = topic.unwrap_or("getting_started");
    let mut payload = json!({ "topic": topic });
    // tool_guide doesn't need docId in payload but the endpoint routes by it
    payload["docId"] = json!(doc_id);

    let result = client.call_tool(doc_id, "tool_guide", payload).await?;
    output::print_response(&result, OutputFormat::Json)?;
    Ok(())
}

/// Dynamic dispatch: parse args as <tool_name> <doc_id> [--json <payload>]
/// and call the tool endpoint directly.
pub async fn dynamic_dispatch(
    client: &CodaClient,
    args: &[String],
    dry_run: bool,
) -> Result<()> {
    if args.is_empty() {
        return Err(CodaError::Validation(
            "Dynamic tool call requires: <tool_name> <doc_id> --json '{...}'".into(),
        ));
    }

    let tool_name = &args[0];

    // Find doc_id (first non-flag arg after tool_name)
    let doc_id = args.iter().skip(1)
        .find(|a| !a.starts_with('-'))
        .ok_or_else(|| CodaError::Validation(
            format!("Missing doc_id. Usage: coda tool {tool_name} <doc_id> --json '{{...}}'")
        ))?;
    validate::validate_resource_id(doc_id, "docId")?;

    // Find --json value
    let json_payload = args.windows(2)
        .find(|w| w[0] == "--json")
        .map(|w| w[1].as_str())
        .ok_or_else(|| CodaError::Validation(
            format!("Missing --json flag. Usage: coda tool {tool_name} {doc_id} --json '{{...}}'")
        ))?;

    let payload = validate::resolve_json_payload(json_payload)?;

    if dry_run {
        output::print_response(
            &client.dry_run_tool(doc_id, tool_name, &payload),
            OutputFormat::Json,
        )?;
        return Ok(());
    }

    let result = client.call_tool(doc_id, tool_name, payload).await?;
    output::print_response(&result, OutputFormat::Json)?;
    Ok(())
}

/// Create a table with typed columns on a page.
pub async fn table_create(
    client: &CodaClient,
    doc_id: &str,
    canvas_id: &str,
    name: &str,
    columns_json: &str,
    rows_json: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(canvas_id, "canvasId")?;

    let columns: Value = validate::resolve_json_payload(columns_json)?;

    let mut payload = json!({
        "docId": doc_id,
        "canvasId": canvas_id,
        "name": name,
        "columns": columns,
    });

    if let Some(rows_str) = rows_json {
        let rows: Value = validate::resolve_json_payload(rows_str)?;
        payload["rows"] = rows;
    }

    if dry_run {
        output::print_response(
            &client.dry_run_tool(doc_id, "table_create", &payload),
            OutputFormat::Json,
        )?;
        return Ok(());
    }

    let result = client.call_tool(doc_id, "table_create", payload).await?;
    output::print_response(&result, OutputFormat::Json)?;
    Ok(())
}

/// Add rows to a table (bulk, typed).
pub async fn table_add_rows(
    client: &CodaClient,
    doc_id: &str,
    table_id: &str,
    columns_json: &str,
    rows_json: &str,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(table_id, "tableId")?;

    let columns: Value = validate::resolve_json_payload(columns_json)?;
    let rows: Value = validate::resolve_json_payload(rows_json)?;

    let payload = json!({
        "docId": doc_id,
        "tableId": table_id,
        "columns": columns,
        "rows": rows,
    });

    if dry_run {
        output::print_response(
            &client.dry_run_tool(doc_id, "table_add_rows", &payload),
            OutputFormat::Json,
        )?;
        return Ok(());
    }

    let result = client.call_tool(doc_id, "table_add_rows", payload).await?;
    output::print_response(&result, OutputFormat::Json)?;
    Ok(())
}

/// Delete rows from a table via the internal tool endpoint.
pub async fn table_delete_rows(
    client: &CodaClient,
    doc_id: &str,
    table_id: &str,
    json_payload: &str,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(table_id, "tableId")?;

    let mut payload = validate::resolve_json_payload(json_payload)?;
    payload["docId"] = json!(doc_id);
    payload["tableId"] = json!(table_id);

    if dry_run {
        output::print_response(
            &client.dry_run_tool(doc_id, "table_delete_rows", &payload),
            OutputFormat::Json,
        )?;
        return Ok(());
    }

    let result = client.call_tool(doc_id, "table_delete_rows", payload).await?;
    output::print_response(&result, OutputFormat::Json)?;
    Ok(())
}

/// Update rows in a table via the internal tool endpoint.
pub async fn table_update_rows(
    client: &CodaClient,
    doc_id: &str,
    table_id: &str,
    json_payload: &str,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(table_id, "tableId")?;

    let mut payload = validate::resolve_json_payload(json_payload)?;
    payload["docId"] = json!(doc_id);
    payload["tableId"] = json!(table_id);

    if dry_run {
        output::print_response(
            &client.dry_run_tool(doc_id, "table_update_rows", &payload),
            OutputFormat::Json,
        )?;
        return Ok(());
    }

    let result = client.call_tool(doc_id, "table_update_rows", payload).await?;
    output::print_response(&result, OutputFormat::Json)?;
    Ok(())
}

/// Modify page content (add text, headings, lists, etc.)
pub async fn content_modify(
    client: &CodaClient,
    doc_id: &str,
    canvas_id: &str,
    operations_json: &str,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(canvas_id, "canvasId")?;

    let operations: Value = validate::resolve_json_payload(operations_json)?;

    let payload = json!({
        "docId": doc_id,
        "canvasId": canvas_id,
        "operations": operations,
    });

    if dry_run {
        output::print_response(
            &client.dry_run_tool(doc_id, "content_modify", &payload),
            OutputFormat::Json,
        )?;
        return Ok(());
    }

    let result = client.call_tool(doc_id, "content_modify", payload).await?;
    output::print_response(&result, OutputFormat::Json)?;
    Ok(())
}

/// Manage comments: add to page, add to row, add to cell, reply, delete.
pub async fn comment_manage(
    client: &CodaClient,
    doc_id: &str,
    json_payload: &str,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;

    let mut payload = validate::resolve_json_payload(json_payload)?;
    payload["docId"] = json!(doc_id);

    if dry_run {
        output::print_response(
            &client.dry_run_tool(doc_id, "comment_manage", &payload),
            OutputFormat::Json,
        )?;
        return Ok(());
    }

    let result = client.call_tool(doc_id, "comment_manage", payload).await?;
    output::print_response(&result, OutputFormat::Json)?;
    Ok(())
}

/// Create a named formula on a page.
pub async fn formula_create(
    client: &CodaClient,
    doc_id: &str,
    canvas_id: &str,
    name: &str,
    formula: &str,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(canvas_id, "canvasId")?;

    let payload = json!({
        "docId": doc_id,
        "canvasId": canvas_id,
        "name": name,
        "formula": formula,
    });

    if dry_run {
        output::print_response(
            &client.dry_run_tool(doc_id, "formula_create", &payload),
            OutputFormat::Json,
        )?;
        return Ok(());
    }

    let result = client.call_tool(doc_id, "formula_create", payload).await?;
    output::print_response(&result, OutputFormat::Json)?;
    Ok(())
}

/// Execute a Coda Formula Language expression.
pub async fn formula_execute(
    client: &CodaClient,
    doc_id: &str,
    formula: &str,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;

    let payload = json!({
        "docId": doc_id,
        "formula": formula,
    });

    if dry_run {
        output::print_response(
            &client.dry_run_tool(doc_id, "formula_execute", &payload),
            OutputFormat::Json,
        )?;
        return Ok(());
    }

    let result = client.call_tool(doc_id, "formula_execute", payload).await?;
    output::print_response(&result, OutputFormat::Json)?;
    Ok(())
}

/// Configure a table view (rename, filter, change layout).
#[allow(clippy::too_many_arguments)]
pub async fn view_configure(
    client: &CodaClient,
    doc_id: &str,
    table_id: &str,
    view_id: &str,
    name: Option<&str>,
    layout: Option<&str>,
    filter_formula: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(table_id, "tableId")?;

    let mut payload = json!({
        "docId": doc_id,
        "tableId": table_id,
        "tableViewId": view_id,
    });

    if let Some(n) = name {
        payload["name"] = json!(n);
    }
    if let Some(l) = layout {
        payload["viewLayout"] = json!(l);
    }
    if let Some(f) = filter_formula {
        if f == "none" {
            payload["filterFormula"] = json!(null);
        } else {
            payload["filterFormula"] = json!(f);
        }
    }

    if dry_run {
        output::print_response(
            &client.dry_run_tool(doc_id, "table_view_configure", &payload),
            OutputFormat::Json,
        )?;
        return Ok(());
    }

    let result = client.call_tool(doc_id, "table_view_configure", payload).await?;
    output::print_response(&result, OutputFormat::Json)?;
    Ok(())
}

/// Add columns to an existing table.
pub async fn table_add_columns(
    client: &CodaClient,
    doc_id: &str,
    table_id: &str,
    columns_json: &str,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(table_id, "tableId")?;

    let columns: Value = validate::resolve_json_payload(columns_json)?;

    let payload = json!({
        "docId": doc_id,
        "tableId": table_id,
        "columns": columns,
    });

    if dry_run {
        output::print_response(
            &client.dry_run_tool(doc_id, "table_add_columns", &payload),
            OutputFormat::Json,
        )?;
        return Ok(());
    }

    let result = client.call_tool(doc_id, "table_add_columns", payload).await?;
    output::print_response(&result, OutputFormat::Json)?;
    Ok(())
}

/// Import rows from stdin via the internal tool endpoint (table_add_rows).
/// Reads NDJSON or JSON array, auto-batches into chunks of up to batch_size.
pub async fn import_rows(
    client: &CodaClient,
    doc_id: &str,
    table_id: &str,
    columns_json: &str,
    batch_size: usize,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(table_id, "tableId")?;

    let columns: Value = validate::resolve_json_payload(columns_json)?;

    // Read all input from stdin
    let mut input = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)
        .map_err(CodaError::Io)?;
    let input = input.trim();

    if input.is_empty() {
        return Err(CodaError::Validation(
            "No input on stdin. Pipe rows as JSON array of arrays (values in column order).".into(),
        ));
    }

    let all_rows: Vec<Value> = if input.starts_with('[') {
        // Could be array of arrays (rows) or a single array wrapping
        let parsed: Value = serde_json::from_str(input).map_err(|e| {
            CodaError::Validation(format!("Invalid JSON array: {e}"))
        })?;
        match parsed {
            Value::Array(arr) => arr,
            _ => vec![parsed],
        }
    } else {
        // NDJSON: one row per line (each line is an array of cell values)
        input
            .lines()
            .filter(|line| !line.trim().is_empty())
            .enumerate()
            .map(|(i, line)| {
                serde_json::from_str(line).map_err(|e| {
                    CodaError::Validation(format!("Invalid JSON on line {}: {e}", i + 1))
                })
            })
            .collect::<Result<Vec<Value>>>()?
    };

    if all_rows.is_empty() {
        eprintln!("[import] No rows to import.");
        return Ok(());
    }

    let total_rows = all_rows.len();
    let mut batch_num = 0u32;

    for chunk in all_rows.chunks(batch_size) {
        batch_num += 1;

        let payload = json!({
            "docId": doc_id,
            "tableId": table_id,
            "columns": columns,
            "rows": chunk,
        });

        if dry_run {
            let dry = json!({
                "batch": batch_num,
                "rowCount": chunk.len(),
                "method": "POST",
                "url": format!("https://coda.io/apis/mcp/vbeta/docs/{}/tool", doc_id),
                "toolName": "table_add_rows",
            });
            println!("{}", serde_json::to_string(&dry)?);
            continue;
        }

        let result = client.call_tool(doc_id, "table_add_rows", payload).await?;
        let row_count = result.get("rowCount").and_then(|v| v.as_u64()).unwrap_or(0);
        eprintln!(
            "[import] Batch {batch_num}: sent {} rows (table now has {row_count} total)",
            chunk.len()
        );
    }

    if dry_run {
        eprintln!("[import] Dry run: {total_rows} rows in {batch_num} batches of up to {batch_size}.");
    } else {
        let result = json!({
            "totalRows": total_rows,
            "batches": batch_num,
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
    }

    Ok(())
}

/// Call any internal tool by name with a raw JSON payload.
pub async fn raw(
    client: &CodaClient,
    doc_id: &str,
    tool_name: &str,
    payload_json: &str,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    let payload: Value = validate::resolve_json_payload(payload_json)?;

    if dry_run {
        output::print_response(
            &client.dry_run_tool(doc_id, tool_name, &payload),
            OutputFormat::Json,
        )?;
        return Ok(());
    }

    let result = client.call_tool(doc_id, tool_name, payload).await?;
    output::print_response(&result, OutputFormat::Json)?;
    Ok(())
}
