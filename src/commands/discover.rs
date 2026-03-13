use crate::client::{CodaClient, ToolCaller};
use crate::error::Result;
use crate::output;
use crate::schema_cache;

/// Fetch tools (cache-aware). Shared by discover_all and discover_one.
async fn fetch_tools(client: &CodaClient, refresh: bool) -> Result<Vec<serde_json::Value>> {
    if !refresh {
        if let Some(cached) = schema_cache::load()? {
            output::info(&format!(
                "Using cached tools (fetched at {}). Use --refresh to update.\n",
                cached.fetched_at
            ));
            return Ok(cached.tools);
        }
    }
    output::info("Fetching tools from Coda MCP endpoint...\n");
    let tools = client.fetch_tools().await?;
    schema_cache::save(&tools)?;
    Ok(tools)
}

/// Discover all available tools by fetching from the MCP endpoint.
/// If refresh is false, tries the local cache first.
/// If filter is Some, only shows tools matching the filter in name or description.
pub async fn discover_all(client: &CodaClient, refresh: bool, filter: Option<&str>) -> Result<()> {
    let mut tools = fetch_tools(client, refresh).await?;

    // Append synthetic compound tools
    tools.extend(super::compound::synthetic_tool_schemas());

    let filter_lower = filter.map(|f| f.to_lowercase());
    let mut count = 0usize;

    for tool in &tools {
        let name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("?");
        let desc = tool
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("");

        // Apply filter: match name or description (case-insensitive)
        if let Some(ref f) = filter_lower {
            let name_lower = name.to_lowercase();
            let desc_lower = desc.to_lowercase();
            if !name_lower.contains(f) && !desc_lower.contains(f) {
                continue;
            }
        }

        count += 1;

        let required = tool
            .get("inputSchema")
            .and_then(|s| s.get("required"))
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_else(|| "(none)".into());

        let desc_short = if desc.chars().count() > 50 {
            let truncated: String = desc.chars().take(47).collect();
            format!("{truncated}...")
        } else {
            desc.to_string()
        };

        println!("  {name:30} {desc_short:52} required: [{required}]");
    }

    if filter.is_some() {
        output::info(&format!(
            "\n{count} tools matched (of {} total).",
            tools.len()
        ));
    } else {
        output::info(&format!("\n{count} tools available."));
    }
    Ok(())
}

/// Discover a single tool's schema. Tries cache first, falls back to network.
/// If compact is true, shows a condensed agent-friendly view instead of full JSON.
pub async fn discover_one(
    client: &CodaClient,
    tool_name: &str,
    refresh: bool,
    compact: bool,
) -> Result<()> {
    let mut tools = fetch_tools(client, refresh).await?;

    // Append synthetic compound tools (same as discover_all)
    tools.extend(super::compound::synthetic_tool_schemas());

    let tool = tools
        .iter()
        .find(|t| t.get("name").and_then(|n| n.as_str()) == Some(tool_name));

    match tool {
        Some(t) => {
            if compact {
                print_compact_schema(t);
            } else {
                println!("{}", serde_json::to_string_pretty(t)?);
            }
        }
        None => {
            // Tool not in list — might have been removed
            let available: Vec<&str> = tools
                .iter()
                .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
                .collect();

            output::info(&format!("Tool '{}' not found.\n", tool_name));
            output::info(&format!("Available tools: {}\n", available.join(", ")));
        }
    }

    Ok(())
}

/// Print a compact, agent-friendly schema: name, description, required fields with types.
fn print_compact_schema(tool: &serde_json::Value) {
    let name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("?");
    let desc = tool
        .get("description")
        .and_then(|d| d.as_str())
        .unwrap_or("");

    println!("{name}");
    println!("  {desc}");

    let schema = match tool.get("inputSchema") {
        Some(s) => s,
        None => {
            println!("  (no input schema)");
            return;
        }
    };

    let required: Vec<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let properties = schema.get("properties").and_then(|p| p.as_object());

    if required.is_empty() {
        println!("  required: (none)");
    } else {
        println!("  required:");
        if let Some(props) = properties {
            for field in &required {
                if let Some(prop) = props.get(*field) {
                    let type_str = compact_type(prop);
                    let field_desc = prop
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("");
                    let desc_short = if field_desc.chars().count() > 60 {
                        let truncated: String = field_desc.chars().take(57).collect();
                        format!("{truncated}...")
                    } else {
                        field_desc.to_string()
                    };
                    if desc_short.is_empty() {
                        println!("    {field}: {type_str}");
                    } else {
                        println!("    {field}: {type_str} — {desc_short}");
                    }
                } else {
                    println!("    {field}");
                }
            }
        } else {
            println!("    {}", required.join(", "));
        }
    }

    // Show optional fields briefly
    if let Some(props) = properties {
        let optional: Vec<&String> = props
            .keys()
            .filter(|k| !required.contains(&k.as_str()))
            .collect();
        if !optional.is_empty() {
            let opt_strs: Vec<String> = optional
                .iter()
                .map(|k| {
                    let type_str = props.get(k.as_str()).map(compact_type).unwrap_or_default();
                    format!("{k}({type_str})")
                })
                .collect();
            println!("  optional: {}", opt_strs.join(", "));
        }
    }

    // Hint about tool_guide if description mentions it
    if desc.contains("tool_guide") || desc.contains("Call tool_guide") {
        let topic = if name.starts_with("table") || name.starts_with("view") {
            "table"
        } else if name.starts_with("page") {
            "page"
        } else if name.starts_with("content") {
            "content"
        } else if name.starts_with("formula") {
            "formula"
        } else if name.starts_with("comment") {
            "comment"
        } else {
            "document"
        };
        println!(
            "  tip: run `shd tool_guide --json '{{\"topic\":\"{topic}\"}}'` for usage examples"
        );
    }
}

/// Summarize a JSON Schema property into a short type string.
/// Surfaces semantic hints (e.g. "ID") from the description when the
/// structural type alone would be ambiguous (e.g. `[string]` vs `[id]`).
fn compact_type(prop: &serde_json::Value) -> String {
    let semantic_hint = prop
        .get("description")
        .and_then(|d| d.as_str())
        .and_then(|d| infer_semantic_type(d));
    // Handle anyOf / oneOf (union types)
    if let Some(any_of) = prop.get("anyOf").or_else(|| prop.get("oneOf")) {
        if let Some(variants) = any_of.as_array() {
            let types: Vec<String> = variants.iter().take(4).map(compact_type).collect();
            let joined = types.join("|");
            if variants.len() > 4 {
                return format!("{joined}|...");
            }
            return joined;
        }
    }

    // Handle const
    if let Some(c) = prop.get("const").and_then(|c| c.as_str()) {
        return format!("\"{}\"", c);
    }

    // Handle enum
    if let Some(vals) = prop.get("enum").and_then(|e| e.as_array()) {
        let strs: Vec<&str> = vals.iter().take(5).filter_map(|v| v.as_str()).collect();
        if vals.len() > 5 {
            return format!("enum({}|...)", strs.join("|"));
        }
        return format!("enum({})", strs.join("|"));
    }

    // Handle type
    match prop.get("type").and_then(|t| t.as_str()) {
        Some("array") => {
            let items_type = prop
                .get("items")
                .map(compact_type)
                .unwrap_or_else(|| "any".into());
            // Surface semantic hint on array items when items are plain "string"
            if items_type == "string" {
                if let Some(hint) = &semantic_hint {
                    return format!("[{hint}]");
                }
            }
            format!("[{items_type}]")
        }
        Some("object") => {
            // Show required keys if available
            if let Some(required) = prop.get("required").and_then(|r| r.as_array()) {
                let keys: Vec<&str> = required.iter().take(4).filter_map(|v| v.as_str()).collect();
                format!("{{{}}}", keys.join(", "))
            } else {
                "object".into()
            }
        }
        Some("string") => {
            // Surface semantic hint for plain strings (e.g. "uri", "id")
            if let Some(hint) = &semantic_hint {
                hint.clone()
            } else {
                "string".into()
            }
        }
        Some(t) => t.to_string(),
        None => "any".into(),
    }
}

/// Infer a more descriptive type label from a field's description.
/// Returns None if no strong semantic signal is found.
fn infer_semantic_type(desc: &str) -> Option<String> {
    let lower = desc.to_lowercase();

    // URI / URL patterns
    if lower.contains("uri") || lower.starts_with("url") {
        return Some("uri".into());
    }

    // ID patterns: "Column IDs", "Row ID", "element ID", etc.
    // Match " ID" or " IDs" as whole words (preceded by space or start of string)
    if lower.contains(" id ")
        || lower.contains(" ids ")
        || lower.contains(" id(")
        || lower.ends_with(" id")
        || lower.ends_with(" ids")
        || lower.starts_with("id ")
        || lower.starts_with("ids ")
    {
        return Some("id".into());
    }

    None
}
