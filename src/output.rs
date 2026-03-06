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

/// Print multiple picked values as tab-separated on one line.
/// Strings print without quotes, other types as compact JSON.
pub fn print_picked_multi(values: &[&Value]) -> Result<()> {
    let parts: Vec<String> = values
        .iter()
        .map(|v| match v {
            Value::String(s) => s.clone(),
            _ => serde_json::to_string(v).unwrap_or_default(),
        })
        .collect();
    println!("{}", parts.join("\t"));
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
