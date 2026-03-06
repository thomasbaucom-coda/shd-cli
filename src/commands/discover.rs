use crate::client::CodaClient;
use crate::error::Result;
use crate::tool_registry;

/// Discover all available tools by probing the endpoint.
pub async fn discover_all(client: &CodaClient) -> Result<()> {
    println!("Probing tool endpoint for available tools...\n");

    let mut available = 0u32;
    let mut not_found = Vec::new();

    for tool in tool_registry::TOOLS {
        let result = client.probe_tool(tool.name).await?;
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
            println!("  {:30} required: {req_str}", tool.name);
            available += 1;
        } else {
            not_found.push(tool.name.to_string());
        }
    }

    println!("\n{available} tools available, {} not found", not_found.len());
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
