use crate::error::Result;
use crate::sanitize;
use comfy_table::{Cell, Table};
use serde_json::Value;

/// Whether to sanitize output against prompt injection. Set globally.
static SANITIZE_ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Whether to suppress informational stderr messages.
static QUIET_ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_sanitize(enabled: bool) {
    SANITIZE_ENABLED.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

pub fn set_quiet(enabled: bool) {
    QUIET_ENABLED.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

fn should_sanitize() -> bool {
    SANITIZE_ENABLED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Print an informational message to stderr, suppressed by --quiet.
pub fn info(msg: &str) {
    if !QUIET_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
        eprint!("{msg}");
    }
}

fn maybe_sanitize(value: &Value) -> std::borrow::Cow<'_, Value> {
    if should_sanitize() {
        let mut sanitized = value.clone();
        let count = sanitize::sanitize_value(&mut sanitized);
        if count > 0 {
            eprintln!(
                "[sanitize] Redacted {count} potential prompt injection pattern(s) in response."
            );
        }
        std::borrow::Cow::Owned(sanitized)
    } else {
        std::borrow::Cow::Borrowed(value)
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
            _ => Err(format!(
                "Unknown output format: {s}. Use json, table, or ndjson."
            )),
        }
    }
}

/// Print a single API response in the requested format.
pub fn print_response(value: &Value, format: OutputFormat) -> Result<()> {
    let value = maybe_sanitize(value);
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(value.as_ref())?);
        }
        OutputFormat::Table => {
            print_as_table(value.as_ref());
        }
        OutputFormat::Ndjson => {
            println!("{}", serde_json::to_string(value.as_ref())?);
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

/// Print a picked value: strings without quotes, other types as compact JSON.
pub fn print_picked(value: &Value) -> Result<()> {
    match value {
        Value::String(s) => println!("{s}"),
        _ => println!("{}", serde_json::to_string(value)?),
    }
    Ok(())
}

/// Build a JSON object from picked paths and values.
/// Uses the last segment of each path as key (e.g., "pages.0.title" → "title").
/// When two paths have the same last segment, falls back to full dot-paths for all keys.
pub fn build_picked_object(paths: &[&str], values: &[&Value]) -> Value {
    let keys: Vec<&str> = paths
        .iter()
        .map(|p| p.rsplit('.').next().unwrap_or(p))
        .collect();

    // Detect collisions
    let has_collision = {
        let mut seen = std::collections::HashSet::new();
        keys.iter().any(|k| !seen.insert(k))
    };

    let mut obj = serde_json::Map::new();
    for (i, val) in values.iter().enumerate() {
        let key = if has_collision { paths[i] } else { keys[i] };
        obj.insert(key.to_string(), (*val).clone());
    }
    Value::Object(obj)
}

/// Print multiple picked values as a JSON object keyed by their path's last segment.
/// Example: --pick "docUri,pages" → {"docUri": "...", "pages": [...]}
pub fn print_picked_multi(paths: &[&str], values: &[&Value]) -> Result<()> {
    let obj = build_picked_object(paths, values);
    println!("{}", serde_json::to_string_pretty(&obj)?);
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
