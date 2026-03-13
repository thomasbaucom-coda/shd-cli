mod common;
use common::mock_client::MockClient;
use serde_json::json;

// ---------------------------------------------------------------------------
// Task 7: doc_scaffold fail-fast + best-effort
// ---------------------------------------------------------------------------

#[tokio::test]
async fn doc_scaffold_all_success_returns_complete_true() {
    let mock = MockClient::new();
    // document_create
    mock.enqueue_ok(json!({
        "docUri": "coda://docs/abc",
        "browserLink": "https://coda.io/d/abc",
        "pages": [{"canvasUri": "coda://docs/abc/canvas/c1", "pageUri": "coda://docs/abc/pages/p1"}]
    }));
    // page_update (rename first page)
    mock.enqueue_ok(json!({"uri": "coda://docs/abc/pages/p1"}));
    // content_modify
    mock.enqueue_ok(json!({"uri": "coda://docs/abc/pages/p1"}));

    let payload = json!({
        "title": "Test Doc",
        "pages": [{"title": "Page 1", "content": "# Hello"}]
    });

    let result = coda_cli::commands::compound::execute(&mock, "doc_scaffold", payload)
        .await
        .unwrap();
    assert_eq!(result["complete"], true);
    assert!(result["errors"].as_array().unwrap().is_empty());
    assert!(result["docUri"].as_str().is_some());
}

#[tokio::test]
async fn doc_scaffold_doc_creation_fails_returns_error() {
    let mock = MockClient::new();
    mock.enqueue_err(coda_cli::error::CodaError::Api {
        status: 500,
        message: "Internal error".into(),
    });

    let payload = json!({"title": "Test Doc", "pages": []});
    let result = coda_cli::commands::compound::execute(&mock, "doc_scaffold", payload).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn doc_scaffold_content_fails_returns_partial_with_errors() {
    let mock = MockClient::new();
    // document_create
    mock.enqueue_ok(json!({
        "docUri": "coda://docs/abc",
        "browserLink": "https://coda.io/d/abc",
        "pages": [{"canvasUri": "coda://docs/abc/canvas/c1", "pageUri": "coda://docs/abc/pages/p1"}]
    }));
    // page_update (rename first page)
    mock.enqueue_ok(json!({"uri": "coda://docs/abc/pages/p1"}));
    // content_modify fails (non-retriable 400 so no retries)
    mock.enqueue_err(coda_cli::error::CodaError::Api {
        status: 400,
        message: "Content insert failed".into(),
    });

    let payload = json!({
        "title": "Test Doc",
        "pages": [{"title": "Page 1", "content": "# Hello"}]
    });

    let result = coda_cli::commands::compound::execute(&mock, "doc_scaffold", payload)
        .await
        .unwrap();
    assert_eq!(result["complete"], false);
    assert!(!result["errors"].as_array().unwrap().is_empty());
    assert!(result["docUri"].as_str().is_some());
    // Page should have status "partial" since content failed
    assert_eq!(result["pages"][0]["status"], "partial");
}

#[tokio::test]
async fn doc_scaffold_page2_fails_is_critical_error() {
    let mock = MockClient::new();
    // document_create
    mock.enqueue_ok(json!({
        "docUri": "coda://docs/abc",
        "browserLink": "https://coda.io/d/abc",
        "pages": [{"canvasUri": "coda://docs/abc/canvas/c1", "pageUri": "coda://docs/abc/pages/p1"}]
    }));
    // page_update for page 1
    mock.enqueue_ok(json!({"uri": "coda://docs/abc/pages/p1"}));
    // content_modify for page 1
    mock.enqueue_ok(json!({"uri": "coda://docs/abc/pages/p1"}));
    // page_create for page 2 fails
    mock.enqueue_err(coda_cli::error::CodaError::Api {
        status: 500,
        message: "Page creation failed".into(),
    });

    let payload = json!({
        "title": "Test Doc",
        "pages": [
            {"title": "Page 1", "content": "# One"},
            {"title": "Page 2", "content": "# Two"}
        ]
    });

    let result = coda_cli::commands::compound::execute(&mock, "doc_scaffold", payload).await;
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Task 8: page_create_with_content fail-fast + best-effort
// ---------------------------------------------------------------------------

#[tokio::test]
async fn page_create_with_content_page_fails_returns_error() {
    let mock = MockClient::new();
    mock.enqueue_err(coda_cli::error::CodaError::Api {
        status: 404,
        message: "Doc not found".into(),
    });

    let payload = json!({"uri": "coda://docs/abc", "title": "New Page", "content": "# Hi"});
    let result =
        coda_cli::commands::compound::execute(&mock, "page_create_with_content", payload).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn page_create_with_content_content_fails_returns_partial() {
    let mock = MockClient::new();
    // page_create succeeds
    mock.enqueue_ok(json!({
        "canvasUri": "coda://docs/abc/canvas/c1",
        "pageUri": "coda://docs/abc/pages/p1",
        "uri": "coda://docs/abc/pages/p1"
    }));
    // content_modify fails (non-retriable so no retries)
    mock.enqueue_err(coda_cli::error::CodaError::Api {
        status: 400,
        message: "Content failed".into(),
    });

    let payload = json!({"uri": "coda://docs/abc", "title": "New Page", "content": "# Hi"});
    let result = coda_cli::commands::compound::execute(&mock, "page_create_with_content", payload)
        .await
        .unwrap();
    assert_eq!(result["complete"], false);
    assert!(!result["errors"].as_array().unwrap().is_empty());
    assert!(result["pageUri"].as_str().is_some() || result["uri"].as_str().is_some());
}
