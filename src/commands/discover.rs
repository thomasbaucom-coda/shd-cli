use crate::client::CodaClient;
use crate::error::Result;

/// Discover all available tools by fetching from the MCP endpoint.
/// This is fully dynamic — new tools appear without a CLI rebuild.
pub async fn discover_all(client: &CodaClient) -> Result<()> {
    eprintln!("Fetching tools from Coda MCP endpoint...\n");

    let tools = client.fetch_tools().await?;

    for tool in &tools {
        let name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("?");
        let desc = tool.get("description").and_then(|d| d.as_str()).unwrap_or("");

        let required = tool.get("inputSchema")
            .and_then(|s| s.get("required"))
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_else(|| "(none)".into());

        let desc_short = if desc.len() > 50 {
            format!("{}...", &desc[..47])
        } else {
            desc.to_string()
        };

        println!("  {name:30} {desc_short:52} required: [{required}]");
    }

    eprintln!("\n{} tools available.", tools.len());
    Ok(())
}

/// Discover a single tool's schema by fetching the full list and filtering.
pub async fn discover_one(client: &CodaClient, tool_name: &str) -> Result<()> {
    let tools = client.fetch_tools().await?;

    let tool = tools.iter().find(|t| {
        t.get("name").and_then(|n| n.as_str()) == Some(tool_name)
    });

    match tool {
        Some(t) => {
            println!("{}", serde_json::to_string_pretty(t)?);
        }
        None => {
            // Tool not in list — might have been removed
            let available: Vec<&str> = tools.iter()
                .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
                .collect();

            eprintln!("Tool '{}' not found.", tool_name);
            eprintln!("Available tools: {}", available.join(", "));
        }
    }

    Ok(())
}
