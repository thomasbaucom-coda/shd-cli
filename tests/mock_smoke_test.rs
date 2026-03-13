mod common;

use coda_cli::client::ToolCaller;
use coda_cli::error::CodaError;
use common::mock_client::MockClient;
use serde_json::json;

#[tokio::test]
async fn mock_client_returns_queued_response() {
    let mock = MockClient::new();
    mock.enqueue_fixture("whoami.json");

    let result = mock.call_tool("whoami", json!({})).await.unwrap();

    assert_eq!(result["name"], "Test User");
    assert_eq!(result["loginId"], "test@example.com");
    mock.assert_tool_called("whoami");
    assert_eq!(mock.call_count(), 1);
}

#[tokio::test]
async fn mock_client_returns_queued_error() {
    let mock = MockClient::new();
    mock.enqueue_err(CodaError::Api {
        status: 404,
        message: "Not found".into(),
    });

    let result = mock.call_tool("nonexistent_tool", json!({})).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        CodaError::Api { status, message } => {
            assert_eq!(status, 404);
            assert_eq!(message, "Not found");
        }
        other => panic!("Expected Api error, got: {:?}", other),
    }
    mock.assert_tool_called("nonexistent_tool");
    assert_eq!(mock.call_count(), 1);
}
