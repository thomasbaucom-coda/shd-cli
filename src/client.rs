use crate::error::{CodaError, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use serde_json::Value;

const PUBLIC_API_BASE: &str = "https://coda.io/apis/v1";
const TOOL_API_BASE: &str = "https://coda.io/apis/mcp/vbeta";
const CLI_USER_AGENT: &str = concat!("coda-cli/", env!("CARGO_PKG_VERSION"));

pub struct CodaClient {
    http: reqwest::Client,
    base_url: String,
    tool_base_url: String,
    #[allow(dead_code)]
    token: String,
}

/// Represents a request that may or may not be executed (dry-run support).
pub struct ApiRequest {
    pub method: reqwest::Method,
    pub url: String,
    pub body: Option<Value>,
    pub query_params: Vec<(String, String)>,
}

/// Represents a response from the Coda API.
pub struct ApiResponse {
    #[allow(dead_code)]
    pub status: u16,
    pub body: Value,
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
            base_url: PUBLIC_API_BASE.to_string(),
            tool_base_url: TOOL_API_BASE.to_string(),
            token,
        })
    }

    /// Build a public API request without executing it.
    pub fn build_request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
        query_params: Vec<(String, String)>,
    ) -> ApiRequest {
        ApiRequest {
            method,
            url: format!("{}{}", self.base_url, path),
            body,
            query_params,
        }
    }

    /// Execute a public API request.
    pub async fn execute(&self, req: ApiRequest) -> Result<ApiResponse> {
        let mut builder = self.http.request(req.method.clone(), &req.url);

        if !req.query_params.is_empty() {
            builder = builder.query(&req.query_params);
        }

        if let Some(body) = &req.body {
            builder = builder.json(body);
        }

        let response = builder.send().await?;
        let status = response.status().as_u16();
        let body: Value = response.json().await.unwrap_or(Value::Null);

        if status >= 400 {
            let message = extract_error_message(&body);
            return Err(CodaError::Api { status, message });
        }

        Ok(ApiResponse { status, body })
    }

    /// Call an internal Coda agent tool via the direct tool endpoint.
    /// POST /apis/mcp/vbeta/docs/{docId}/tool with {toolName, payload}
    pub async fn call_tool(
        &self,
        doc_id: &str,
        tool_name: &str,
        payload: Value,
    ) -> Result<Value> {
        let url = format!(
            "{}/docs/{}/tool",
            self.tool_base_url,
            crate::validate::encode_path_segment(doc_id),
        );

        let body = serde_json::json!({
            "toolName": tool_name,
            "payload": payload,
        });

        let response = self.http
            .post(&url)
            .json(&body)
            .send()
            .await?;

        let status = response.status().as_u16();
        let resp_body: Value = response.json().await.unwrap_or(Value::Null);

        if status >= 400 {
            // Try to extract a useful error message from various response formats
            let message = resp_body
                .get("result")
                .and_then(|r| r.get("error"))
                .and_then(|e| e.as_str())
                .or_else(|| resp_body.get("message").and_then(|m| m.as_str()))
                .or_else(|| resp_body.get("statusMessage").and_then(|m| m.as_str()))
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    // If no known fields, dump the full body for debugging
                    serde_json::to_string(&resp_body).unwrap_or_else(|_| "Unknown error".into())
                });
            return Err(CodaError::Api { status, message });
        }

        // The tool endpoint wraps results in {toolName, result, executionTime}
        let result = resp_body.get("result").cloned().unwrap_or(resp_body);
        Ok(result)
    }

    /// Build a dry-run representation of a tool call.
    pub fn dry_run_tool(
        &self,
        doc_id: &str,
        tool_name: &str,
        payload: &Value,
    ) -> Value {
        serde_json::json!({
            "method": "POST",
            "url": format!("{}/docs/{}/tool", self.tool_base_url, crate::validate::encode_path_segment(doc_id)),
            "body": {
                "toolName": tool_name,
                "payload": payload,
            }
        })
    }

    #[allow(dead_code)]
    pub fn token(&self) -> &str {
        &self.token
    }
}

fn extract_error_message(body: &Value) -> String {
    extract_error_message_ref(body).to_string()
}

fn extract_error_message_ref(body: &Value) -> &str {
    body.get("message")
        .and_then(|m| m.as_str())
        .unwrap_or_else(|| {
            body.get("statusMessage")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error")
        })
}

impl ApiRequest {
    /// Render this request as a dry-run JSON object for inspection.
    pub fn to_dry_run_json(&self) -> Value {
        let mut obj = serde_json::json!({
            "method": self.method.as_str(),
            "url": self.url,
        });
        if !self.query_params.is_empty() {
            obj["queryParams"] = serde_json::json!(
                self.query_params.iter()
                    .map(|(k, v)| serde_json::json!({k: v}))
                    .collect::<Vec<_>>()
            );
        }
        if let Some(body) = &self.body {
            obj["body"] = body.clone();
        }
        obj
    }
}
