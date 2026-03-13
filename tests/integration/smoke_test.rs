use coda_cli::auth;
use coda_cli::client::{CodaClient, ToolCaller};
use serde_json::json;

fn get_client() -> CodaClient {
    let token = auth::resolve_token(None)
        .expect("No API token found. Set CODA_API_TOKEN or run `shd auth login`.");
    CodaClient::new(token).unwrap()
}

#[tokio::test]
async fn whoami_returns_name() {
    let client = get_client();
    let result = client.call_tool("whoami", json!({})).await.unwrap();
    assert!(
        result.get("name").is_some(),
        "whoami should return a name field"
    );
}

#[tokio::test]
async fn discover_returns_tools() {
    let client = get_client();
    let tools = client.fetch_tools().await.unwrap();
    assert!(
        !tools.is_empty(),
        "discover should return at least one tool"
    );
}

#[tokio::test]
async fn nonexistent_tool_returns_error() {
    let client = get_client();
    let result = client
        .call_tool("definitely_not_a_real_tool_12345", json!({}))
        .await;
    assert!(result.is_err());
}
