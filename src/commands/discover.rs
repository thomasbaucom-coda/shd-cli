use crate::client::CodaClient;
use crate::error::Result;

/// Known tool names to probe. This list bootstraps discovery —
/// if a tool is added server-side, `coda discover` will find it
/// via tool_guide, and dynamic dispatch handles execution regardless.
const KNOWN_TOOLS: &[&str] = &[
    "whoami",
    "document_create",
    "document_delete",
    "document_read",
    "search",
    "url_decode",
    "tool_guide",
    "page_create",
    "page_read",
    "page_update",
    "page_delete",
    "page_duplicate",
    "table_create",
    "table_add_rows",
    "table_add_columns",
    "table_read_rows",
    "table_delete",
    "table_delete_rows",
    "table_delete_columns",
    "table_update_rows",
    "table_update_columns",
    "table_view_configure",
    "content_modify",
    "content_image_upload",
    "comment_manage",
    "formula_create",
    "formula_execute",
    "formula_update",
    "formula_delete",
];

/// Discover all available tools by probing the endpoint.
pub async fn discover_all(client: &CodaClient) -> Result<()> {
    println!("Probing tool endpoint for available tools...\n");

    let mut available = Vec::new();
    let mut not_found = Vec::new();

    for tool_name in KNOWN_TOOLS {
        let result = client.probe_tool(tool_name).await?;
        let exists = result.get("exists").and_then(|v| v.as_bool()).unwrap_or(false);

        if exists {
            let issues = result
                .get("schema")
                .and_then(|s| s.get("issues"))
                .and_then(|i| i.as_array())
                .cloned()
                .unwrap_or_default();

            let required: Vec<String> = issues
                .iter()
                .filter_map(|i| {
                    let path = i.get("path").and_then(|p| p.as_array())?;
                    let field = path.last()?.as_str()?;
                    if field == "toolName" { return None; }
                    let expected = i.get("expected").and_then(|e| e.as_str()).unwrap_or("any");
                    Some(format!("{field} ({expected})"))
                })
                .collect();

            let req_str = if required.is_empty() {
                "(none)".to_string()
            } else {
                required.join(", ")
            };
            println!("  {tool_name:30} required: {req_str}");
            available.push(tool_name.to_string());
        } else {
            not_found.push(tool_name.to_string());
        }
    }

    println!("\n{} tools available, {} not found", available.len(), not_found.len());
    if !not_found.is_empty() {
        println!("Not found: {}", not_found.join(", "));
    }

    Ok(())
}

/// Discover a single tool's schema by probing it.
pub async fn discover_one(client: &CodaClient, tool_name: &str) -> Result<()> {
    let result = client.probe_tool(tool_name).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
