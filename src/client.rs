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
        headers.insert(USER_AGENT, HeaderValue::from_static(CLI_USER_AGENT));

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self {
            http,
            tool_base_url: TOOL_API_BASE.to_string(),
        })
    }

    /// Build the tool endpoint URL, validating the docId if present.
    fn build_tool_url(&self, payload: &Value) -> Result<String> {
        match payload.get("docId").and_then(|v| v.as_str()) {
            Some(doc_id) if !doc_id.is_empty() => {
                crate::validate::validate_resource_id(doc_id, "docId")?;
                Ok(format!(
                    "{}/docs/{}/tool",
                    self.tool_base_url,
                    crate::validate::encode_path_segment(doc_id),
                ))
            }
            _ => Ok(format!("{}/tool", self.tool_base_url)),
        }
    }

    /// Call a Coda tool via the direct tool endpoint.
    /// POST /apis/mcp/vbeta/tool (docless) or /apis/mcp/vbeta/docs/{docId}/tool
    pub async fn call_tool(&self, tool_name: &str, payload: Value) -> Result<Value> {
        let url = self.build_tool_url(&payload)?;

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
        let mut result = resp_body.get("result").cloned().unwrap_or(resp_body);

        // Auto-paginate: if result has items + nextPageToken, follow pages
        self.auto_paginate(tool_name, &payload, &mut result).await;

        Ok(result)
    }

    /// Build a dry-run representation of a tool call.
    pub fn dry_run_tool(&self, tool_name: &str, payload: &Value) -> Result<Value> {
        let url = self.build_tool_url(payload)?;
        Ok(serde_json::json!({
            "method": "POST",
            "url": url,
            "body": {
                "toolName": tool_name,
                "payload": payload,
            }
        }))
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
            let message = resp_body
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unauthorized");
            return Err(CodaError::Api {
                status,
                message: message.to_string(),
            });
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

        let response = self
            .http
            .post(&url)
            .header("Accept", "application/json, text/event-stream")
            .json(&body)
            .send()
            .await?;

        let status = response.status().as_u16();
        if status == 401 {
            return Err(CodaError::Api {
                status,
                message: "Invalid token. Generate an MCP-scoped token at https://coda.io/account"
                    .into(),
            });
        }

        let text = response.text().await.unwrap_or_default();

        // Parse SSE: look for "data:" lines containing the tools/list result
        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data:") {
                if let Ok(msg) = serde_json::from_str::<Value>(data.trim()) {
                    if let Some(tools) = msg
                        .get("result")
                        .and_then(|r| r.get("tools"))
                        .and_then(|t| t.as_array())
                    {
                        return Ok(tools.clone());
                    }
                }
            }
        }

        Err(CodaError::Other(
            "Failed to fetch tool list from MCP endpoint".into(),
        ))
    }

    /// Follow pagination tokens to collect all pages into a single result.
    /// Merges `items` arrays from subsequent pages into the first result.
    /// Stops after 50 pages as a safety limit.
    async fn auto_paginate(&self, tool_name: &str, original_payload: &Value, result: &mut Value) {
        const MAX_PAGES: usize = 50;

        let mut page_count = 0usize;
        loop {
            let token = result
                .get("nextPageToken")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string());

            let token = match token {
                Some(t) if !t.is_empty() => t,
                _ => break,
            };

            page_count += 1;
            if page_count >= MAX_PAGES {
                crate::output::info(&format!(
                    "[paginate] Stopped after {MAX_PAGES} pages. Results may be partial.\n"
                ));
                break;
            }

            // Build next-page payload
            let mut next_payload = original_payload.clone();
            if let Some(obj) = next_payload.as_object_mut() {
                obj.insert("pageToken".to_string(), serde_json::json!(token));
            }

            let next_result = match self.call_tool_single(tool_name, next_payload).await {
                Ok(r) => r,
                Err(_) => {
                    crate::output::info(
                        "[paginate] Error fetching next page. Results may be partial.\n",
                    );
                    break;
                }
            };

            // Merge items arrays
            if let Some(next_items) = next_result.get("items").and_then(|v| v.as_array()) {
                if let Some(items) = result.get_mut("items").and_then(|v| v.as_array_mut()) {
                    items.extend(next_items.iter().cloned());
                }
            }

            // Update nextPageToken from this page's result.
            // Use as_str() to handle null tokens (JSON null → None → break).
            match next_result
                .get("nextPageToken")
                .and_then(|t| t.as_str())
                .filter(|s| !s.is_empty())
            {
                Some(t) => result["nextPageToken"] = serde_json::json!(t),
                None => {
                    if let Some(obj) = result.as_object_mut() {
                        obj.remove("nextPageToken");
                    }
                    break;
                }
            }
        }

        // Clean up the token from final result — agent doesn't need it
        if page_count > 0 {
            if let Some(obj) = result.as_object_mut() {
                obj.remove("nextPageToken");
            }
        }
    }

    /// Single-page tool call (no auto-pagination). Used internally by auto_paginate.
    async fn call_tool_single(&self, tool_name: &str, payload: Value) -> Result<Value> {
        let url = self.build_tool_url(&payload)?;

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

        let result = resp_body.get("result").cloned().unwrap_or(resp_body);
        Ok(result)
    }

    /// Parse a tool endpoint error into a structured, agent-friendly CodaError.
    fn parse_tool_error(status: u16, body: &Value, tool_name: &str) -> CodaError {
        // Contract validation errors (schema mismatch)
        if let Some(detail) = body.get("codaDetail") {
            if let Some(issues) = detail.get("issues").and_then(|v| v.as_array()) {
                // Check if the tool name itself is invalid (renamed/removed)
                let is_tool_not_found = issues
                    .iter()
                    .any(|i| i.get("discriminator").and_then(|d| d.as_str()) == Some("toolName"));

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
                let field_errors: Vec<String> = issues
                    .iter()
                    .map(|i| {
                        let path = i
                            .get("path")
                            .and_then(|p| p.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str())
                                    .collect::<Vec<_>>()
                                    .join(".")
                            })
                            .unwrap_or_default();
                        let msg = i
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("invalid");
                        format!("{path}: {msg}")
                    })
                    .collect();

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
        let message = body
            .get("result")
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_tool_not_found_error() {
        let body = json!({
            "codaDetail": {
                "issues": [{"discriminator": "toolName", "message": "not found"}]
            }
        });
        let err = CodaClient::parse_tool_error(400, &body, "bad_tool");
        match err {
            CodaError::ContractChanged { tool, message } => {
                assert_eq!(tool, "bad_tool");
                assert!(message.contains("not found"), "{message}");
            }
            _ => panic!("Expected ContractChanged, got: {err:?}"),
        }
    }

    #[test]
    fn parse_schema_validation_error() {
        let body = json!({
            "codaDetail": {
                "issues": [{
                    "path": ["payload", "docId"],
                    "message": "Field is required"
                }]
            }
        });
        let err = CodaClient::parse_tool_error(400, &body, "table_create");
        match err {
            CodaError::ContractChanged { tool, message } => {
                assert_eq!(tool, "table_create");
                assert!(message.contains("payload.docId"), "{message}");
                assert!(message.contains("Field is required"), "{message}");
            }
            _ => panic!("Expected ContractChanged, got: {err:?}"),
        }
    }

    #[test]
    fn parse_generic_api_error() {
        let body = json!({"message": "Rate limited"});
        let err = CodaClient::parse_tool_error(429, &body, "whoami");
        match err {
            CodaError::Api { status, message } => {
                assert_eq!(status, 429);
                assert_eq!(message, "Rate limited");
            }
            _ => panic!("Expected Api error, got: {err:?}"),
        }
    }

    #[test]
    fn parse_error_with_result_error_field() {
        let body = json!({"result": {"error": "Doc not found"}});
        let err = CodaClient::parse_tool_error(404, &body, "doc_get");
        match err {
            CodaError::Api { message, .. } => {
                assert_eq!(message, "Doc not found");
            }
            _ => panic!("Expected Api error, got: {err:?}"),
        }
    }

    #[test]
    fn parse_error_fallback_to_json_body() {
        let body = json!({"unexpected": "format"});
        let err = CodaClient::parse_tool_error(500, &body, "whoami");
        match err {
            CodaError::Api { message, .. } => {
                assert!(message.contains("unexpected"), "{message}");
            }
            _ => panic!("Expected Api error, got: {err:?}"),
        }
    }

    /// Verify that the pagination token extraction handles null/missing/empty correctly.
    /// This tests the logic pattern used in auto_paginate without needing a live client.
    #[test]
    fn pagination_token_extraction() {
        // Normal token
        let result = json!({"items": [], "nextPageToken": "abc123"});
        let token = result
            .get("nextPageToken")
            .and_then(|t| t.as_str())
            .filter(|s| !s.is_empty());
        assert_eq!(token, Some("abc123"));

        // Null token (should be treated as no more pages)
        let result = json!({"items": [], "nextPageToken": null});
        let token = result
            .get("nextPageToken")
            .and_then(|t| t.as_str())
            .filter(|s| !s.is_empty());
        assert!(token.is_none(), "null token should be None");

        // Empty string token
        let result = json!({"items": [], "nextPageToken": ""});
        let token = result
            .get("nextPageToken")
            .and_then(|t| t.as_str())
            .filter(|s| !s.is_empty());
        assert!(token.is_none(), "empty token should be None");

        // Missing token entirely
        let result = json!({"items": []});
        let token = result
            .get("nextPageToken")
            .and_then(|t| t.as_str())
            .filter(|s| !s.is_empty());
        assert!(token.is_none(), "missing token should be None");
    }
}
