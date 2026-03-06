use crate::error::{CodaError, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use serde_json::Value;

const TOOL_API_BASE: &str = "https://coda.io/apis/mcp/vbeta";
const CLI_USER_AGENT: &str = concat!("coda-cli/", env!("CARGO_PKG_VERSION"));

pub struct CodaClient {
    http: reqwest::Client,
    tool_base_url: String,
}

impl CodaClient {
    pub fn new(token: String) -> Result<Self> {
        let mut headers = HeaderMap::new();
        let auth_value = format!("Bearer {token}");
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth_value)
                .map_err(|_| CodaError::Validation("Invalid token format".into()))?,
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(CLI_USER_AGENT),
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            http,
            tool_base_url: TOOL_API_BASE.to_string(),
        })
    }

    /// Call a Coda tool via the direct tool endpoint.
    /// POST /apis/mcp/vbeta/tool (docless) or /apis/mcp/vbeta/docs/{docId}/tool
    pub async fn call_tool(
        &self,
        tool_name: &str,
        payload: Value,
    ) -> Result<Value> {
        // Route via docId if present in payload, otherwise use docless endpoint
        let url = match payload.get("docId").and_then(|v| v.as_str()) {
            Some(doc_id) if !doc_id.is_empty() => {
                format!(
                    "{}/docs/{}/tool",
                    self.tool_base_url,
                    crate::validate::encode_path_segment(doc_id),
                )
            }
            _ => format!("{}/tool", self.tool_base_url),
        };

        let body = serde_json::json!({
            "toolName": tool_name,
            "payload": payload,
        });

        let response = self.http.post(&url).json(&body).send().await?;
        let status = response.status().as_u16();
        let resp_body: Value = response.json().await.unwrap_or(Value::Null);

        if status >= 400 {
            return Err(Self::parse_tool_error(status, &resp_body, tool_name));
        }

        // The tool endpoint wraps results in {toolName, result, executionTime}
        let result = resp_body.get("result").cloned().unwrap_or(resp_body);
        Ok(result)
    }

    /// Build a dry-run representation of a tool call.
    pub fn dry_run_tool(
        &self,
        tool_name: &str,
        payload: &Value,
    ) -> Value {
        let url = match payload.get("docId").and_then(|v| v.as_str()) {
            Some(doc_id) if !doc_id.is_empty() => {
                format!("{}/docs/{}/tool", self.tool_base_url, crate::validate::encode_path_segment(doc_id))
            }
            _ => format!("{}/tool", self.tool_base_url),
        };
        serde_json::json!({
            "method": "POST",
            "url": url,
            "body": {
                "toolName": tool_name,
                "payload": payload,
            }
        })
    }

    /// Probe a tool with empty payload to discover its required fields.
    #[allow(dead_code)]
    pub async fn probe_tool(&self, tool_name: &str) -> Result<Value> {
        let url = format!("{}/tool", self.tool_base_url);
        let body = serde_json::json!({
            "toolName": tool_name,
            "payload": {},
        });

        let response = self.http.post(&url).json(&body).send().await?;
        let status = response.status().as_u16();
        let resp_body: Value = response.json().await.unwrap_or(Value::Null);

        // Auth failure — propagate as a real error, not "tool not found"
        if status == 401 {
            let message = resp_body.get("message").and_then(|m| m.as_str()).unwrap_or("Unauthorized");
            return Err(CodaError::Api { status, message: message.to_string() });
        }

        if status == 400 {
            // Validation error — extract the schema from issues
            if let Some(detail) = resp_body.get("codaDetail") {
                return Ok(serde_json::json!({
                    "tool": tool_name,
                    "exists": true,
                    "schema": detail,
                }));
            }
        }

        if status == 200 {
            // Tool requires no fields (like whoami)
            return Ok(serde_json::json!({
                "tool": tool_name,
                "exists": true,
                "schema": {"issues": []},
                "note": "No required fields",
            }));
        }

        // Tool not found
        Ok(serde_json::json!({
            "tool": tool_name,
            "exists": false,
            "error": resp_body.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown"),
        }))
    }

    /// Fetch all available tools from the MCP endpoint via tools/list.
    /// Calls the Streamable HTTP MCP endpoint and parses the SSE response.
    pub async fn fetch_tools(&self) -> Result<Vec<Value>> {
        let url = self.tool_base_url.clone();
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {},
        });

        let response = self.http
            .post(&url)
            .header("Accept", "application/json, text/event-stream")
            .json(&body)
            .send()
            .await?;

        let status = response.status().as_u16();
        if status == 401 {
            return Err(CodaError::Api {
                status,
                message: "Invalid token. Generate an MCP-scoped token at https://coda.io/account".into(),
            });
        }

        let text = response.text().await.unwrap_or_default();

        // Parse SSE: look for "data:" lines containing the tools/list result
        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data:") {
                if let Ok(msg) = serde_json::from_str::<Value>(data.trim()) {
                    if let Some(tools) = msg.get("result")
                        .and_then(|r| r.get("tools"))
                        .and_then(|t| t.as_array())
                    {
                        return Ok(tools.clone());
                    }
                }
            }
        }

        Err(CodaError::Other("Failed to fetch tool list from MCP endpoint".into()))
    }

    /// Parse a tool endpoint error into a structured, agent-friendly CodaError.
    fn parse_tool_error(status: u16, body: &Value, tool_name: &str) -> CodaError {
        // Contract validation errors (schema mismatch)
        if let Some(detail) = body.get("codaDetail") {
            if let Some(issues) = detail.get("issues").and_then(|v| v.as_array()) {
                // Check if the tool name itself is invalid (renamed/removed)
                let is_tool_not_found = issues.iter().any(|i| {
                    i.get("discriminator").and_then(|d| d.as_str()) == Some("toolName")
                });

                if is_tool_not_found {
                    return CodaError::ContractChanged {
                        tool: tool_name.to_string(),
                        message: format!(
                            "Tool '{}' not found. It may have been renamed or removed. Run `coda discover` to see available tools.",
                            tool_name
                        ),
                    };
                }

                // Schema validation — missing/wrong fields
                let field_errors: Vec<String> = issues.iter().map(|i| {
                    let path = i.get("path")
                        .and_then(|p| p.as_array())
                        .map(|arr| arr.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join("."))
                        .unwrap_or_default();
                    let msg = i.get("message").and_then(|m| m.as_str()).unwrap_or("invalid");
                    format!("{path}: {msg}")
                }).collect();

                return CodaError::ContractChanged {
                    tool: tool_name.to_string(),
                    message: format!(
                        "Validation error for '{}':\n{}\nRun `coda discover {}` to see current schema.",
                        tool_name,
                        field_errors.join("\n"),
                        tool_name,
                    ),
                };
            }
        }

        // Generic error
        let message = body.get("result")
            .and_then(|r| r.get("error"))
            .and_then(|e| e.as_str())
            .or_else(|| body.get("message").and_then(|m| m.as_str()))
            .or_else(|| body.get("statusMessage").and_then(|m| m.as_str()))
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                serde_json::to_string(body).unwrap_or_else(|_| "Unknown error".into())
            });

        CodaError::Api { status, message }
    }
}
