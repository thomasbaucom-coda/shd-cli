use crate::error::{CodaError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

/// Default cache TTL: 24 hours.
const CACHE_TTL_HOURS: i64 = 24;

#[derive(Serialize, Deserialize)]
pub struct CachedTools {
    pub tools: Vec<Value>,
    pub fetched_at: String,
}

pub fn cache_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| CodaError::Other("Could not determine config directory".into()))?;
    Ok(config_dir.join("shd").join("tool_cache.json"))
}

pub fn load() -> Result<Option<CachedTools>> {
    load_from(&cache_path()?)
}

pub fn load_from(path: &std::path::Path) -> Result<Option<CachedTools>> {
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(path)?;
    match serde_json::from_str::<CachedTools>(&contents) {
        Ok(cached) => {
            if is_expired(&cached) {
                Ok(None)
            } else {
                Ok(Some(cached))
            }
        }
        Err(_) => Ok(None), // corrupt file — treat as missing
    }
}

/// Returns true if the cache is older than CACHE_TTL_HOURS.
fn is_expired(cached: &CachedTools) -> bool {
    let Ok(fetched) = chrono::DateTime::parse_from_rfc3339(&cached.fetched_at) else {
        return true; // unparseable timestamp — treat as expired
    };
    let age = chrono::Utc::now() - fetched.to_utc();
    age > chrono::Duration::hours(CACHE_TTL_HOURS)
}

pub fn save(tools: &[Value]) -> Result<()> {
    save_to(&cache_path()?, tools)
}

pub fn save_to(path: &std::path::Path, tools: &[Value]) -> Result<()> {
    let cached = CachedTools {
        tools: tools.to_vec(),
        fetched_at: chrono::Utc::now().to_rfc3339(),
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&cached)?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn find_tool<'a>(tools: &'a [Value], name: &str) -> Option<&'a Value> {
    tools
        .iter()
        .find(|t| t.get("name").and_then(|n| n.as_str()) == Some(name))
}

/// Lightweight validation: check that all required fields from the tool's
/// inputSchema are present in the payload. Not a full JSON Schema validator.
/// On failure, includes type/description hints from the schema for each missing field.
pub fn validate_payload(tool_schema: &Value, payload: &Value) -> Result<()> {
    let input_schema = tool_schema.get("inputSchema");

    let required = input_schema
        .and_then(|s| s.get("required"))
        .and_then(|r| r.as_array());

    let required = match required {
        Some(r) => r,
        None => return Ok(()), // no required fields
    };

    let properties = input_schema
        .and_then(|s| s.get("properties"))
        .and_then(|p| p.as_object());

    let payload_obj = payload.as_object();
    let mut missing: Vec<String> = Vec::new();

    for field in required {
        if let Some(name) = field.as_str() {
            let present = payload_obj
                .map(|obj| obj.contains_key(name))
                .unwrap_or(false);
            if !present {
                let hint = properties
                    .and_then(|props| props.get(name))
                    .map(|prop| {
                        let type_str = prop
                            .get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("unknown");
                        let desc = prop
                            .get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("");
                        if desc.is_empty() {
                            format!("{name} ({type_str})")
                        } else {
                            format!("{name} ({type_str}): {desc}")
                        }
                    })
                    .unwrap_or_else(|| name.to_string());
                missing.push(hint);
            }
        }
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(CodaError::Validation(format!(
            "Missing required field(s):\n  {}",
            missing.join("\n  ")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trip_save_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tool_cache.json");

        let tools = vec![
            json!({"name": "whoami", "description": "Get current user"}),
            json!({"name": "doc_create", "description": "Create a doc", "inputSchema": {"required": ["title"]}}),
        ];

        save_to(&path, &tools).unwrap();
        let loaded = load_from(&path).unwrap().expect("should load");
        assert_eq!(loaded.tools.len(), 2);
        assert_eq!(loaded.tools[0]["name"], "whoami");
        assert!(!loaded.fetched_at.is_empty());
    }

    #[test]
    fn load_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        assert!(load_from(&path).unwrap().is_none());
    }

    #[test]
    fn load_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corrupt.json");
        std::fs::write(&path, "not valid json!!!").unwrap();
        assert!(load_from(&path).unwrap().is_none());
    }

    #[test]
    fn find_tool_by_name() {
        let tools = vec![json!({"name": "whoami"}), json!({"name": "doc_create"})];
        assert!(find_tool(&tools, "whoami").is_some());
        assert!(find_tool(&tools, "nonexistent").is_none());
    }

    #[test]
    fn expired_cache_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tool_cache.json");

        // Write a cache with a timestamp 25 hours in the past
        let old_time = chrono::Utc::now() - chrono::Duration::hours(25);
        let cached = CachedTools {
            tools: vec![json!({"name": "whoami"})],
            fetched_at: old_time.to_rfc3339(),
        };
        let json = serde_json::to_string_pretty(&cached).unwrap();
        std::fs::write(&path, json).unwrap();

        assert!(
            load_from(&path).unwrap().is_none(),
            "expired cache should return None"
        );
    }

    #[test]
    fn fresh_cache_returns_some() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tool_cache.json");

        // Write a cache with a recent timestamp
        save_to(&path, &[json!({"name": "whoami"})]).unwrap();
        assert!(
            load_from(&path).unwrap().is_some(),
            "fresh cache should return Some"
        );
    }

    #[test]
    fn validate_payload_passes_when_all_required_present() {
        let schema = json!({
            "inputSchema": {
                "required": ["docId", "name"]
            }
        });
        let payload = json!({"docId": "abc", "name": "test", "extra": true});
        assert!(validate_payload(&schema, &payload).is_ok());
    }

    #[test]
    fn validate_payload_fails_on_missing_required() {
        let schema = json!({
            "inputSchema": {
                "required": ["docId", "name"]
            }
        });
        let payload = json!({"docId": "abc"});
        let err = validate_payload(&schema, &payload).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("name"),
            "error should mention missing field: {msg}"
        );
    }

    #[test]
    fn validate_payload_includes_schema_hints() {
        let schema = json!({
            "inputSchema": {
                "required": ["docId", "name"],
                "properties": {
                    "docId": {"type": "string", "description": "ID of the doc"},
                    "name": {"type": "string", "description": "Name of the table"}
                }
            }
        });
        let payload = json!({});
        let err = validate_payload(&schema, &payload).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("string"), "should include type hint: {msg}");
        assert!(
            msg.contains("ID of the doc"),
            "should include description: {msg}"
        );
    }

    #[test]
    fn validate_payload_passes_with_no_required() {
        let schema = json!({"inputSchema": {}});
        assert!(validate_payload(&schema, &json!({})).is_ok());

        let schema_no_input = json!({"name": "whoami"});
        assert!(validate_payload(&schema_no_input, &json!({})).is_ok());
    }
}
