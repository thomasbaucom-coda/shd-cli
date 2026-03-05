use crate::error::Result;
use crate::sanitize;
use comfy_table::{Cell, Table};
use serde_json::Value;

/// Whether to sanitize output against prompt injection. Set globally.
static SANITIZE_ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_sanitize(enabled: bool) {
    SANITIZE_ENABLED.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

fn should_sanitize() -> bool {
    SANITIZE_ENABLED.load(std::sync::atomic::Ordering::Relaxed)
}

fn maybe_sanitize(value: &Value) -> Value {
    if should_sanitize() {
        let mut sanitized = value.clone();
        let count = sanitize::sanitize_value(&mut sanitized);
        if count > 0 {
            eprintln!("[sanitize] Redacted {count} potential prompt injection pattern(s) in response.");
        }
        sanitized
    } else {
        value.clone()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Json,
    Table,
    Ndjson,
}

impl OutputFormat {
    pub fn from_str_opt(s: &str) -> std::result::Result<Self, String> {
        match s.to_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "table" => Ok(Self::Table),
            "ndjson" => Ok(Self::Ndjson),
            _ => Err(format!("Unknown output format: {s}. Use json, table, or ndjson.")),
        }
    }
}

/// Print a single API response in the requested format.
pub fn print_response(value: &Value, format: OutputFormat) -> Result<()> {
    let value = maybe_sanitize(value);
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
        OutputFormat::Table => {
            print_as_table(&value);
        }
        OutputFormat::Ndjson => {
            print_ndjson(&value)?;
        }
    }
    Ok(())
}

/// Print a list response (with `items` array) in the requested format.
pub fn print_list_response(value: &Value, format: OutputFormat) -> Result<()> {
    let value = maybe_sanitize(value);
    let items = value.get("items").and_then(|v| v.as_array());

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
        OutputFormat::Table => {
            if let Some(items) = items {
                print_items_as_table(items);
            } else {
                print_as_table(&value);
            }
        }
        OutputFormat::Ndjson => {
            if let Some(items) = items {
                for item in items {
                    println!("{}", serde_json::to_string(item)?);
                }
            } else {
                println!("{}", serde_json::to_string(&value)?);
            }
        }
    }
    Ok(())
}

fn print_as_table(value: &Value) {
    match value {
        Value::Object(map) => {
            let mut table = Table::new();
            table.set_header(vec!["Field", "Value"]);
            for (key, val) in map {
                let display = format_cell_value(val);
                table.add_row(vec![Cell::new(key), Cell::new(&display)]);
            }
            println!("{table}");
        }
        _ => {
            // Fall back to JSON for non-objects
            if let Ok(s) = serde_json::to_string_pretty(value) {
                println!("{s}");
            }
        }
    }
}

fn print_items_as_table(items: &[Value]) {
    if items.is_empty() {
        println!("(no results)");
        return;
    }

    // Collect all unique keys from all items to build columns
    let mut columns: Vec<String> = Vec::new();
    for item in items {
        if let Value::Object(map) = item {
            for key in map.keys() {
                if !columns.contains(key) {
                    columns.push(key.clone());
                }
            }
        }
    }

    // Prioritize common useful columns first
    let priority = ["id", "name", "title", "type", "href", "browserLink"];
    let mut ordered: Vec<String> = Vec::new();
    for p in &priority {
        if let Some(pos) = columns.iter().position(|c| c == p) {
            ordered.push(columns.remove(pos));
        }
    }
    ordered.extend(columns);

    let mut table = Table::new();
    table.set_header(ordered.iter().map(Cell::new));

    for item in items {
        let row: Vec<Cell> = ordered
            .iter()
            .map(|col| {
                let val = item.get(col).unwrap_or(&Value::Null);
                Cell::new(format_cell_value(val))
            })
            .collect();
        table.add_row(row);
    }

    println!("{table}");
}

fn print_ndjson(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

fn format_cell_value(val: &Value) -> String {
    match val {
        Value::Null => "".to_string(),
        Value::String(s) => {
            if s.chars().count() > 60 {
                let truncated: String = s.chars().take(57).collect();
                format!("{truncated}...")
            } else {
                s.clone()
            }
        }
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Array(arr) => format!("[{} items]", arr.len()),
        Value::Object(_) => "{...}".to_string(),
    }
}
