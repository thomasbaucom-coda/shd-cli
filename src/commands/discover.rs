use crate::client::CodaClient;
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
    let tools = fetch_tools(client, refresh).await?;

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
pub async fn discover_one(client: &CodaClient, tool_name: &str, refresh: bool) -> Result<()> {
    let tools = fetch_tools(client, refresh).await?;

    let tool = tools
        .iter()
        .find(|t| t.get("name").and_then(|n| n.as_str()) == Some(tool_name));

    match tool {
        Some(t) => {
            println!("{}", serde_json::to_string_pretty(t)?);
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
