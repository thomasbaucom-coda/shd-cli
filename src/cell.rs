//! Cell value unwrapping and row flattening for Coda table data.
//!
//! Coda's API returns row values keyed by column IDs with rich wrapper objects.
//! This module provides utilities to flatten those into simple, agent-friendly
//! JSON objects with human-readable column names as keys.

use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// Unwrap a Coda cell value to its underlying JSON primitive.
///
/// - `"hello"` → `"hello"` (string passthrough)
/// - `42` → `42` (number passthrough)
/// - `true` → `true` (bool passthrough)
/// - `null` → `null`
/// - `{"content": "Alice", "@type": "CodaText"}` → `"Alice"`
/// - `{"url": "https://..."}` → `"https://..."`
/// - `[{"content": "A"}, {"content": "B"}]` → `["A", "B"]`
/// - Unknown objects → preserved as-is
pub fn unwrap_cell_value(cell: &Value) -> Value {
    match cell {
        Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null => cell.clone(),
        Value::Object(map) => {
            if let Some(content) = map.get("content") {
                content.clone()
            } else if let Some(url) = map.get("url") {
                url.clone()
            } else {
                cell.clone()
            }
        }
        Value::Array(arr) => {
            let unwrapped: Vec<Value> = arr.iter().map(unwrap_cell_value).collect();
            Value::Array(unwrapped)
        }
    }
}

/// Build a column ID → display name map from the columns metadata array.
///
/// Handles duplicate column names by appending ` (c-xyz)` to the second occurrence.
/// Tries `columnId` first, then `id` as fallback for the column identifier.
pub fn build_column_map(columns: &[Value]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut name_counts: HashMap<String, usize> = HashMap::new();

    for col in columns {
        let id = col
            .get("columnId")
            .or_else(|| col.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if id.is_empty() {
            continue;
        }

        let raw_name = col
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled")
            .to_string();

        let count = name_counts.entry(raw_name.clone()).or_insert(0);
        *count += 1;

        let display_name = if *count > 1 {
            format!("{} ({})", raw_name, id)
        } else {
            raw_name
        };

        map.insert(id.to_string(), display_name);
    }

    map
}

/// Flatten a raw API row into a simple object with human-readable column names.
///
/// Input:  `{"rowId":"r-abc","values":{"c-1":{"content":"Alice"},"c-2":42},"browserLink":"..."}`
/// Output: `{"_rowId":"r-abc","Name":"Alice","Score":42}`
///
/// - Internal fields are prefixed with `_`
/// - Column IDs are resolved to display names via `column_map`
/// - Unknown column IDs fall back to the raw ID as the key
/// - Metadata fields (browserLink, createdAt, updatedAt) are dropped
pub fn flatten_row(row: &Value, column_map: &HashMap<String, String>) -> Value {
    let mut flat = Map::new();

    // Extract row ID (try both "rowId" and "id")
    if let Some(row_id) = row
        .get("rowId")
        .or_else(|| row.get("id"))
        .and_then(|v| v.as_str())
    {
        flat.insert("_rowId".to_string(), json!(row_id));
    }

    // Flatten values using column name mapping
    if let Some(values) = row.get("values").and_then(|v| v.as_object()) {
        for (col_id, cell) in values {
            let col_name = column_map
                .get(col_id)
                .cloned()
                .unwrap_or_else(|| col_id.clone());
            flat.insert(col_name, unwrap_cell_value(cell));
        }
    }

    Value::Object(flat)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- unwrap_cell_value tests --

    #[test]
    fn unwrap_string_passthrough() {
        assert_eq!(unwrap_cell_value(&json!("hello")), json!("hello"));
    }

    #[test]
    fn unwrap_number_passthrough() {
        assert_eq!(unwrap_cell_value(&json!(42)), json!(42));
    }

    #[test]
    fn unwrap_bool_passthrough() {
        assert_eq!(unwrap_cell_value(&json!(true)), json!(true));
    }

    #[test]
    fn unwrap_null_passthrough() {
        assert_eq!(unwrap_cell_value(&Value::Null), Value::Null);
    }

    #[test]
    fn unwrap_content_wrapper() {
        let cell = json!({"content": "Alice", "@type": "CodaText"});
        assert_eq!(unwrap_cell_value(&cell), json!("Alice"));
    }

    #[test]
    fn unwrap_content_preserves_number() {
        let cell = json!({"content": 42});
        assert_eq!(unwrap_cell_value(&cell), json!(42));
    }

    #[test]
    fn unwrap_url_object() {
        let cell = json!({"url": "https://example.com", "name": "Example"});
        // url takes precedence when no content field
        assert_eq!(unwrap_cell_value(&cell), json!("https://example.com"));
    }

    #[test]
    fn unwrap_unknown_object_preserved() {
        let cell = json!({"custom": "data", "nested": true});
        assert_eq!(unwrap_cell_value(&cell), cell);
    }

    #[test]
    fn unwrap_array_of_rich_objects() {
        let cell = json!([{"content": "A"}, {"content": "B"}]);
        assert_eq!(unwrap_cell_value(&cell), json!(["A", "B"]));
    }

    #[test]
    fn unwrap_array_of_primitives() {
        let cell = json!(["a", "b", "c"]);
        assert_eq!(unwrap_cell_value(&cell), json!(["a", "b", "c"]));
    }

    // -- build_column_map tests --

    #[test]
    fn column_map_basic() {
        let cols = vec![
            json!({"columnId": "c-1", "name": "Name"}),
            json!({"columnId": "c-2", "name": "Score"}),
        ];
        let map = build_column_map(&cols);
        assert_eq!(map.get("c-1"), Some(&"Name".to_string()));
        assert_eq!(map.get("c-2"), Some(&"Score".to_string()));
    }

    #[test]
    fn column_map_duplicate_names() {
        let cols = vec![
            json!({"columnId": "c-1", "name": "Status"}),
            json!({"columnId": "c-2", "name": "Status"}),
        ];
        let map = build_column_map(&cols);
        assert_eq!(map.get("c-1"), Some(&"Status".to_string()));
        assert!(map.get("c-2").unwrap().contains("(c-2)"));
    }

    #[test]
    fn column_map_uses_id_fallback() {
        let cols = vec![json!({"id": "c-99", "name": "Foo"})];
        let map = build_column_map(&cols);
        assert_eq!(map.get("c-99"), Some(&"Foo".to_string()));
    }

    #[test]
    fn column_map_skips_empty_id() {
        let cols = vec![json!({"name": "NoId"})];
        let map = build_column_map(&cols);
        assert!(map.is_empty());
    }

    #[test]
    fn column_map_untitled_fallback() {
        let cols = vec![json!({"columnId": "c-1"})];
        let map = build_column_map(&cols);
        assert_eq!(map.get("c-1"), Some(&"Untitled".to_string()));
    }

    // -- flatten_row tests --

    #[test]
    fn flatten_row_basic() {
        let cols = vec![
            json!({"columnId": "c-1", "name": "Name"}),
            json!({"columnId": "c-2", "name": "Score"}),
        ];
        let col_map = build_column_map(&cols);
        let row = json!({
            "rowId": "r-abc",
            "values": {"c-1": {"content": "Alice"}, "c-2": 42},
            "browserLink": "https://coda.io/...",
            "createdAt": "2026-01-01"
        });
        let flat = flatten_row(&row, &col_map);
        assert_eq!(flat["_rowId"], "r-abc");
        assert_eq!(flat["Name"], "Alice");
        assert_eq!(flat["Score"], 42);
        // Metadata should NOT be in output
        assert!(flat.get("browserLink").is_none());
        assert!(flat.get("createdAt").is_none());
    }

    #[test]
    fn flatten_row_unknown_column_uses_id() {
        let col_map = HashMap::new();
        let row = json!({"rowId": "r-1", "values": {"c-unknown": "val"}});
        let flat = flatten_row(&row, &col_map);
        assert_eq!(flat["c-unknown"], "val");
    }

    #[test]
    fn flatten_row_uses_id_field() {
        let col_map = HashMap::new();
        let row = json!({"id": "r-xyz", "values": {"c-1": "val"}});
        let flat = flatten_row(&row, &col_map);
        assert_eq!(flat["_rowId"], "r-xyz");
    }

    #[test]
    fn flatten_row_no_values() {
        let col_map = HashMap::new();
        let row = json!({"rowId": "r-1"});
        let flat = flatten_row(&row, &col_map);
        assert_eq!(flat["_rowId"], "r-1");
        // Should just have _rowId, nothing else
        assert_eq!(flat.as_object().unwrap().len(), 1);
    }

    #[test]
    fn flatten_row_null_cell() {
        let cols = vec![json!({"columnId": "c-1", "name": "Val"})];
        let col_map = build_column_map(&cols);
        let row = json!({"rowId": "r-1", "values": {"c-1": null}});
        let flat = flatten_row(&row, &col_map);
        assert_eq!(flat["Val"], Value::Null);
    }
}
