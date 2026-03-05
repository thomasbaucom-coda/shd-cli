use crate::client::CodaClient;
use crate::error::Result;
use crate::output::{self, OutputFormat};
use crate::validate;
use serde_json::Value;

#[allow(clippy::too_many_arguments)]
pub async fn list(
    client: &CodaClient,
    doc_id: &str,
    table_id: &str,
    format: OutputFormat,
    limit: Option<u32>,
    query: Option<&str>,
    sort_by: Option<&str>,
    fields: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(table_id, "tableId")?;
    let path = format!(
        "/docs/{}/tables/{}/rows",
        validate::encode_path_segment(doc_id),
        validate::encode_path_segment(table_id),
    );

    let mut params = Vec::new();
    if let Some(l) = limit {
        params.push(("limit".to_string(), l.to_string()));
    }
    if let Some(q) = query {
        params.push(("query".to_string(), q.to_string()));
    }
    if let Some(s) = sort_by {
        params.push(("sortBy".to_string(), s.to_string()));
    }
    // useColumnNames makes the output more readable for agents
    params.push(("useColumnNames".to_string(), "true".to_string()));
    // valueFormat=simpleWithArrays gives cleaner values
    params.push(("valueFormat".to_string(), "simpleWithArrays".to_string()));

    let req = client.build_request(reqwest::Method::GET, &path, None, params);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    let resp = client.execute(req).await?;

    // If --fields is specified, filter the row values to only those columns
    if let Some(field_list) = fields {
        let filtered = filter_row_fields(&resp.body, field_list);
        output::print_list_response(&filtered, format)?;
    } else {
        output::print_list_response(&resp.body, format)?;
    }

    Ok(())
}

pub async fn get(
    client: &CodaClient,
    doc_id: &str,
    table_id: &str,
    row_id: &str,
    format: OutputFormat,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(table_id, "tableId")?;
    validate::validate_resource_id(row_id, "rowId")?;
    let path = format!(
        "/docs/{}/tables/{}/rows/{}",
        validate::encode_path_segment(doc_id),
        validate::encode_path_segment(table_id),
        validate::encode_path_segment(row_id),
    );
    let params = vec![
        ("useColumnNames".to_string(), "true".to_string()),
        ("valueFormat".to_string(), "simpleWithArrays".to_string()),
    ];

    let req = client.build_request(reqwest::Method::GET, &path, None, params);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    let resp = client.execute(req).await?;
    output::print_response(&resp.body, format)?;
    Ok(())
}

pub async fn upsert(
    client: &CodaClient,
    doc_id: &str,
    table_id: &str,
    json_payload: &str,
    format: OutputFormat,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(table_id, "tableId")?;
    let path = format!(
        "/docs/{}/tables/{}/rows",
        validate::encode_path_segment(doc_id),
        validate::encode_path_segment(table_id),
    );
    let body = validate::resolve_json_payload(json_payload)?;
    let req = client.build_request(reqwest::Method::POST, &path, Some(body), vec![]);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    let resp = client.execute(req).await?;
    output::print_response(&resp.body, format)?;
    Ok(())
}

pub async fn update(
    client: &CodaClient,
    doc_id: &str,
    table_id: &str,
    row_id: &str,
    json_payload: &str,
    format: OutputFormat,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(table_id, "tableId")?;
    validate::validate_resource_id(row_id, "rowId")?;
    let path = format!(
        "/docs/{}/tables/{}/rows/{}",
        validate::encode_path_segment(doc_id),
        validate::encode_path_segment(table_id),
        validate::encode_path_segment(row_id),
    );
    let body = validate::resolve_json_payload(json_payload)?;
    let req = client.build_request(reqwest::Method::PUT, &path, Some(body), vec![]);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    let resp = client.execute(req).await?;
    output::print_response(&resp.body, format)?;
    Ok(())
}

pub async fn delete(
    client: &CodaClient,
    doc_id: &str,
    table_id: &str,
    row_id: &str,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(table_id, "tableId")?;
    validate::validate_resource_id(row_id, "rowId")?;
    let path = format!(
        "/docs/{}/tables/{}/rows/{}",
        validate::encode_path_segment(doc_id),
        validate::encode_path_segment(table_id),
        validate::encode_path_segment(row_id),
    );
    let req = client.build_request(reqwest::Method::DELETE, &path, None, vec![]);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    client.execute(req).await?;
    eprintln!("Row {row_id} deleted.");
    Ok(())
}

pub async fn delete_rows(
    client: &CodaClient,
    doc_id: &str,
    table_id: &str,
    json_payload: &str,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(table_id, "tableId")?;
    let path = format!(
        "/docs/{}/tables/{}/rows",
        validate::encode_path_segment(doc_id),
        validate::encode_path_segment(table_id),
    );
    let body = validate::resolve_json_payload(json_payload)?;
    let req = client.build_request(reqwest::Method::DELETE, &path, Some(body), vec![]);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    client.execute(req).await?;
    eprintln!("Rows deleted.");
    Ok(())
}

pub async fn push_button(
    client: &CodaClient,
    doc_id: &str,
    table_id: &str,
    row_id: &str,
    column_id: &str,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(table_id, "tableId")?;
    validate::validate_resource_id(row_id, "rowId")?;
    validate::validate_resource_id(column_id, "columnId")?;
    let path = format!(
        "/docs/{}/tables/{}/rows/{}/buttons/{}",
        validate::encode_path_segment(doc_id),
        validate::encode_path_segment(table_id),
        validate::encode_path_segment(row_id),
        validate::encode_path_segment(column_id),
    );
    let req = client.build_request(reqwest::Method::POST, &path, None, vec![]);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    let resp = client.execute(req).await?;
    output::print_response(&resp.body, OutputFormat::Json)?;
    Ok(())
}

/// Import rows from stdin (NDJSON or JSON array), auto-batching into chunks.
/// Each line of NDJSON should be a row object with a "cells" array, or a flat
/// key-value object where keys are column names.
///
/// Supports two input formats:
/// 1. NDJSON: one JSON object per line (flat key-value or Coda row format)
/// 2. JSON array: a single JSON array of objects (read all at once)
pub async fn import(
    client: &CodaClient,
    doc_id: &str,
    table_id: &str,
    key_columns: Option<&str>,
    batch_size: usize,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(table_id, "tableId")?;
    let path = format!(
        "/docs/{}/tables/{}/rows",
        validate::encode_path_segment(doc_id),
        validate::encode_path_segment(table_id),
    );

    // Read all input from stdin
    let mut input = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)
        .map_err(crate::error::CodaError::Io)?;
    let input = input.trim();

    if input.is_empty() {
        return Err(crate::error::CodaError::Validation(
            "No input provided on stdin. Pipe NDJSON rows or a JSON array.".into(),
        ));
    }

    // Parse rows: try JSON array first, then NDJSON
    let raw_rows: Vec<Value> = if input.starts_with('[') {
        serde_json::from_str(input).map_err(|e| {
            crate::error::CodaError::Validation(format!("Invalid JSON array: {e}"))
        })?
    } else {
        input
            .lines()
            .filter(|line| !line.trim().is_empty())
            .enumerate()
            .map(|(i, line)| {
                serde_json::from_str(line).map_err(|e| {
                    crate::error::CodaError::Validation(format!("Invalid JSON on line {}: {e}", i + 1))
                })
            })
            .collect::<Result<Vec<Value>>>()?
    };

    if raw_rows.is_empty() {
        eprintln!("[import] No rows to import.");
        return Ok(());
    }

    // Convert flat objects to Coda row format if needed
    let rows: Vec<Value> = raw_rows
        .into_iter()
        .map(normalize_row)
        .collect();

    let total_rows = rows.len();
    let mut total_added = 0u32;
    let mut total_updated = 0u32;
    let mut batch_num = 0u32;

    // Chunk and send
    for chunk in rows.chunks(batch_size) {
        batch_num += 1;
        let mut body = serde_json::json!({
            "rows": chunk,
        });
        if let Some(keys) = key_columns {
            let key_list: Vec<&str> = keys.split(',').map(|s| s.trim()).collect();
            body["keyColumns"] = serde_json::json!(key_list);
        }

        let req = client.build_request(reqwest::Method::POST, &path, Some(body.clone()), vec![]);

        if dry_run {
            let dry = serde_json::json!({
                "batch": batch_num,
                "rowCount": chunk.len(),
                "method": "POST",
                "url": format!("https://coda.io/apis/v1{}", path),
                "bodyPreview": {
                    "rows": format!("[{} rows]", chunk.len()),
                    "keyColumns": key_columns,
                },
            });
            println!("{}", serde_json::to_string(&dry)?);
            continue;
        }

        let resp = client.execute(req).await?;

        // The API returns either addedRowIds/updatedRowIds (arrays) or
        // addedRowCount/updatedRowCount (numbers) depending on the endpoint version
        let added = resp.body.get("addedRowIds")
            .and_then(|v| v.as_array())
            .map(|a| a.len() as u32)
            .or_else(|| resp.body.get("addedRowCount").and_then(|v| v.as_u64()).map(|n| n as u32))
            .unwrap_or(0);
        let updated = resp.body.get("updatedRowIds")
            .and_then(|v| v.as_array())
            .map(|a| a.len() as u32)
            .or_else(|| resp.body.get("updatedRowCount").and_then(|v| v.as_u64()).map(|n| n as u32))
            .unwrap_or(0);
        total_added += added;
        total_updated += updated;

        eprintln!(
            "[import] Batch {batch_num}: sent {} rows (added: {added}, updated: {updated})",
            chunk.len()
        );
    }

    if dry_run {
        eprintln!(
            "[import] Dry run: {total_rows} rows in {batch_num} batches of up to {batch_size}."
        );
    } else {
        let result = serde_json::json!({
            "totalRows": total_rows,
            "batches": batch_num,
            "addedRowCount": total_added,
            "updatedRowCount": total_updated,
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
    }

    Ok(())
}

/// Normalize a row object into Coda's expected format.
/// If the row is already in Coda format (has "cells" key), pass through.
/// If it's a flat key-value object, convert to cells format.
fn normalize_row(row: Value) -> Value {
    if row.get("cells").is_some() {
        // Already in Coda format
        return row;
    }

    // Flat key-value → convert to cells
    if let Value::Object(map) = &row {
        let cells: Vec<Value> = map
            .iter()
            .map(|(key, val)| {
                serde_json::json!({
                    "column": key,
                    "value": val,
                })
            })
            .collect();
        return serde_json::json!({ "cells": cells });
    }

    // Fallback: wrap as-is
    row
}

/// Filter row response to only include specified column names in values.
fn filter_row_fields(response: &Value, field_list: &str) -> Value {
    let fields: Vec<&str> = field_list.split(',').map(|s| s.trim()).collect();

    let mut filtered = response.clone();
    if let Some(items) = filtered.get_mut("items").and_then(|v| v.as_array_mut()) {
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
    filtered
}
