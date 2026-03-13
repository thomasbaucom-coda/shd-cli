use async_trait::async_trait;
use coda_cli::client::ToolCaller;
use coda_cli::error::{CodaError, Result};
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::Mutex;

/// A mock implementation of ToolCaller for testing.
///
/// Queued responses are returned in FIFO order by `call_tool`.
/// All calls are recorded in `calls` for assertion.
pub struct MockClient {
    responses: Mutex<VecDeque<Result<Value>>>,
    tool_responses: Mutex<VecDeque<Result<Vec<Value>>>>,
    pub calls: Mutex<Vec<(String, Value)>>,
}

impl MockClient {
    pub fn new() -> Self {
        Self {
            responses: Mutex::new(VecDeque::new()),
            tool_responses: Mutex::new(VecDeque::new()),
            calls: Mutex::new(Vec::new()),
        }
    }

    /// Enqueue a successful response for the next `call_tool` invocation.
    pub fn enqueue_ok(&self, value: Value) {
        self.responses.lock().unwrap().push_back(Ok(value));
    }

    /// Enqueue an error response for the next `call_tool` invocation.
    pub fn enqueue_err(&self, error: CodaError) {
        self.responses.lock().unwrap().push_back(Err(error));
    }

    /// Enqueue a response loaded from a fixture file (relative to tests/fixtures/).
    pub fn enqueue_fixture(&self, fixture_path: &str) {
        let path = format!(
            "{}/tests/fixtures/{}",
            env!("CARGO_MANIFEST_DIR"),
            fixture_path
        );
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", path, e));
        let value: Value = serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse fixture {}: {}", path, e));
        self.enqueue_ok(value);
    }

    /// Enqueue a successful response for the next `fetch_tools` invocation.
    pub fn enqueue_tools(&self, tools: Vec<Value>) {
        self.tool_responses.lock().unwrap().push_back(Ok(tools));
    }

    /// Assert that a specific tool was called at least once.
    pub fn assert_tool_called(&self, tool_name: &str) {
        let calls = self.calls.lock().unwrap();
        assert!(
            calls.iter().any(|(name, _)| name == tool_name),
            "Expected tool '{}' to be called, but it was not. Calls: {:?}",
            tool_name,
            calls.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>()
        );
    }

    /// Return the total number of `call_tool` invocations.
    pub fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}

#[async_trait]
impl ToolCaller for MockClient {
    async fn call_tool(&self, tool_name: &str, payload: Value) -> Result<Value> {
        self.calls
            .lock()
            .unwrap()
            .push((tool_name.to_string(), payload));

        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| {
                Err(CodaError::Other(format!(
                    "MockClient: no response queued for call_tool('{}')",
                    tool_name
                )))
            })
    }

    async fn fetch_tools(&self) -> Result<Vec<Value>> {
        self.tool_responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| {
                Err(CodaError::Other(
                    "MockClient: no response queued for fetch_tools".into(),
                ))
            })
    }
}
