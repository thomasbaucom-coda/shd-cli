use crate::client::CodaClient;
use crate::error::{CodaError, Result};
use crate::output::{self, OutputFormat};
use crate::validate;
use serde_json::{json, Value};

/// Call any tool by name with a JSON payload.
/// This is the core dispatch — all commands go through here.
pub async fn call(
    client: &CodaClient,
    tool_name: &str,
    payload: Value,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        output::print_response(
            &client.dry_run_tool(tool_name, &payload),
            OutputFormat::Json,
        )?;
        return Ok(());
    }

    let result = client.call_tool(tool_name, payload).await?;
    output::print_response(&result, OutputFormat::Json)?;
    Ok(())
}

/// Import rows from stdin, auto-batched.
/// Reads NDJSON or JSON array, chunks into batches of batch_size.
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

    let mut input = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)
        .map_err(CodaError::Io)?;
    let input = input.trim();

    if input.is_empty() {
        return Err(CodaError::Validation(
            "No input on stdin. Pipe rows as JSON array of arrays.".into(),
        ));
    }

    let all_rows: Vec<Value> = if input.starts_with('[') {
        let parsed: Value = serde_json::from_str(input).map_err(|e| {
            CodaError::Validation(format!("Invalid JSON array: {e}"))
        })?;
        match parsed {
            Value::Array(arr) => arr,
            _ => vec![parsed],
        }
    } else {
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
                "toolName": "table_add_rows",
            });
            println!("{}", serde_json::to_string(&dry)?);
            continue;
        }

        let result = client.call_tool("table_add_rows", payload).await?;
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
