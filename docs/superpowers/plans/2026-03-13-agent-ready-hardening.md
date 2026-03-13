# Agent-Ready Hardening Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make SHD CLI reliable for real agent workflows — no silent data loss, no inconsistent filesystem state, no fatal errors for optional features.

**Architecture:** Extract a `ToolCaller` trait from `CodaClient` to enable mock testing. Fix error handling in pagination, compound ops, and sync. Add fixture-based integration tests. Make retry logic selective (only retriable errors).

**Tech Stack:** Rust, async-trait crate (for async trait), serde_json fixtures, tempdir for test isolation.

**Spec:** `docs/superpowers/specs/2026-03-13-agent-ready-hardening-design.md`

---

## Chunk 1: Test Infrastructure + ToolCaller Trait

### Task 1: Add async-trait dependency

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add async-trait to Cargo.toml**

Add under `[dependencies]`:
```toml
async-trait = "0.1"
```

And add under `[dev-dependencies]` (create section if needed):
```toml
tempfile = "3"
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles with no new errors.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "Add async-trait and tempfile dependencies for test infrastructure"
```

---

### Task 2: Extract ToolCaller trait from CodaClient

**Files:**
- Modify: `src/client.rs:1-55`

The trait covers only `call_tool` and `fetch_tools`. `dry_run_tool`, `build_tool_url`, and `probe_tool` stay as `CodaClient`-only methods — dry-run is handled at the dispatch layer in `main.rs`.

- [ ] **Step 1: Define the ToolCaller trait**

Add at the top of `src/client.rs`, after the imports:

```rust
use async_trait::async_trait;

#[async_trait]
pub trait ToolCaller: Send + Sync {
    async fn call_tool(&self, tool_name: &str, payload: serde_json::Value) -> crate::error::Result<serde_json::Value>;
    async fn fetch_tools(&self) -> crate::error::Result<Vec<serde_json::Value>>;
}
```

- [ ] **Step 2: Implement ToolCaller for CodaClient**

Rename the existing `call_tool` and `fetch_tools` methods: keep the logic in private methods (`call_tool_impl`, `fetch_tools_impl`), then implement the trait by delegating:

```rust
#[async_trait]
impl ToolCaller for CodaClient {
    async fn call_tool(&self, tool_name: &str, payload: serde_json::Value) -> crate::error::Result<serde_json::Value> {
        self.call_tool_impl(tool_name, payload).await
    }

    async fn fetch_tools(&self) -> crate::error::Result<Vec<serde_json::Value>> {
        self.fetch_tools_impl().await
    }
}
```

- [ ] **Step 3: Verify all 117 tests still pass**

Run: `cargo test`
Expected: `test result: ok. 117 passed`

- [ ] **Step 4: Commit**

```bash
git add src/client.rs
git commit -m "Extract ToolCaller trait from CodaClient"
```

---

### Task 3: Update command functions to accept dyn ToolCaller

**Files:**
- Modify: `src/commands/tools.rs:10` — `call()` signature
- Modify: `src/commands/compound.rs:26,53,217,699` — `dispatch()`, `execute()`, `doc_scaffold()`, `call_with_retry()`
- Modify: `src/commands/sync.rs:49,196,347,460,806` — `run()`, `sync_document()`, `sync_page()`, `sync_table()`, `call_with_retry()`
- Modify: `src/main.rs:345` — `dispatch_tool()` call sites

- [ ] **Step 1: Update tools.rs**

Change `call()` signature from `client: &CodaClient` to `client: &dyn crate::client::ToolCaller`:

```rust
pub async fn call(
    client: &dyn crate::client::ToolCaller,
    tool_name: &str,
    payload: Value,
    dry_run: bool,
    pick: Option<&str>,
    format: OutputFormat,
) -> Result<Option<Value>> {
```

Note: `dry_run_tool` is called from `tools.rs:18`. Since dry-run uses `CodaClient` directly, guard this: if `dry_run` is true, the caller in `main.rs` should handle it before calling `tools::call()`. For now, remove the dry-run branch from `tools::call()` and move it to the dispatch layer in `main.rs`.

- [ ] **Step 2: Update compound.rs**

Change all functions taking `client: &CodaClient` to `client: &dyn crate::client::ToolCaller`. This includes: `dispatch()`, `execute()`, `doc_scaffold()`, `page_create_with_content()`, `doc_summarize()`, `table_search()`, `call_with_retry()`.

- [ ] **Step 3: Update sync.rs**

Change all functions taking `client: &CodaClient` to `client: &dyn crate::client::ToolCaller`. This includes: `run()`, `sync_document()`, `sync_page()`, `sync_table()`, `call_with_retry()`.

- [ ] **Step 4: Update main.rs dispatch**

In `dispatch_tool()` and `run()`, the `client` is already a `CodaClient` which implements `ToolCaller`. Pass it as `&client as &dyn ToolCaller` or let Rust coerce it. Handle dry-run in `dispatch_tool()` before calling `tools::call()`:

```rust
if dry_run {
    client.dry_run_tool(&resolved_name, &payload)?;
    return Ok(());
}
```

Then remove the dry-run branch from `tools::call()`.

- [ ] **Step 5: Verify all 117 tests still pass**

Run: `cargo test`
Expected: `test result: ok. 117 passed`

- [ ] **Step 6: Commit**

```bash
git add src/commands/tools.rs src/commands/compound.rs src/commands/sync.rs src/main.rs
git commit -m "Update command functions to accept dyn ToolCaller trait"
```

---

### Task 4: Build MockClient and fixture loader

**Files:**
- Create: `tests/common/mod.rs`
- Create: `tests/common/mock_client.rs`
- Create: `tests/fixtures/whoami.json`
- Create: `tests/fixtures/tools_list.json`
- Create: `tests/fixtures/errors/404_not_found.json`

- [ ] **Step 1: Create fixture files**

Record real responses by running:
```bash
shd whoami 2>/dev/null > tests/fixtures/whoami.json
shd discover --output json 2>/dev/null | head -1 > tests/fixtures/tools_list.json
```

For error fixtures, create manually:
```json
// tests/fixtures/errors/404_not_found.json
{
  "error": true,
  "type": "api_error",
  "message": "Not found",
  "statusCode": 404
}
```

- [ ] **Step 2: Write MockClient**

```rust
// tests/common/mock_client.rs
use async_trait::async_trait;
use coda_cli::client::ToolCaller;
use coda_cli::error::{CodaError, Result};
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::Mutex;

pub struct MockClient {
    responses: Mutex<VecDeque<Result<Value>>>,
    pub calls: Mutex<Vec<(String, Value)>>,
}

impl MockClient {
    pub fn new() -> Self {
        Self {
            responses: Mutex::new(VecDeque::new()),
            calls: Mutex::new(Vec::new()),
        }
    }

    pub fn enqueue_ok(&self, value: Value) {
        self.responses.lock().unwrap().push_back(Ok(value));
    }

    pub fn enqueue_err(&self, err: CodaError) {
        self.responses.lock().unwrap().push_back(Err(err));
    }

    pub fn enqueue_fixture(&self, fixture_name: &str) {
        let path = format!("tests/fixtures/{}", fixture_name);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("Fixture not found: {}", path));
        let value: Value = serde_json::from_str(&content)
            .unwrap_or_else(|_| panic!("Invalid JSON in fixture: {}", path));
        self.enqueue_ok(value);
    }

    pub fn assert_tool_called(&self, tool_name: &str) {
        let calls = self.calls.lock().unwrap();
        assert!(
            calls.iter().any(|(name, _)| name == tool_name),
            "Expected tool '{}' to be called. Calls: {:?}",
            tool_name,
            calls.iter().map(|(n, _)| n).collect::<Vec<_>>()
        );
    }

    pub fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}

#[async_trait]
impl ToolCaller for MockClient {
    async fn call_tool(&self, tool_name: &str, payload: Value) -> Result<Value> {
        self.calls.lock().unwrap().push((tool_name.to_string(), payload));
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Err(CodaError::Other("MockClient: no more responses queued".into())))
    }

    async fn fetch_tools(&self) -> Result<Vec<Value>> {
        let result = self.call_tool("__fetch_tools", serde_json::json!({})).await?;
        Ok(result.as_array().cloned().unwrap_or_default())
    }
}
```

- [ ] **Step 3: Create tests/common/mod.rs**

```rust
pub mod mock_client;
pub use mock_client::MockClient;
```

- [ ] **Step 4: Create src/lib.rs and update main.rs**

This is critical: `src/lib.rs` must exist before integration tests can import `coda_cli::*`. Additionally, `main.rs` must import from the library crate instead of re-declaring modules with `mod`, otherwise the binary and library have separate types and the `ToolCaller` trait won't be compatible across them.

Create `src/lib.rs`:
```rust
pub mod auth;
pub mod cell;
pub mod client;
pub mod commands;
pub mod error;
pub mod fuzzy;
pub mod output;
pub mod polish;
pub mod sanitize;
pub mod schema_cache;
pub mod slug;
pub mod trace;
pub mod validate;
```

Add to `Cargo.toml`:
```toml
[lib]
name = "coda_cli"
path = "src/lib.rs"
```

Then update `src/main.rs`: replace all `mod` declarations at the top with `use` imports from the library:
```rust
// Before:
// mod auth;
// mod cell;
// mod client;
// ...

// After:
use coda_cli::auth;
use coda_cli::client;
use coda_cli::commands;
use coda_cli::error;
use coda_cli::fuzzy;
use coda_cli::output;
use coda_cli::polish;
use coda_cli::sanitize;
use coda_cli::schema_cache;
use coda_cli::slug;
use coda_cli::trace;
use coda_cli::validate;
```

Also update any `crate::` references in `main.rs` to use the module directly (e.g., `crate::output::info` → `output::info`). Since `main.rs` now imports the library modules, `crate::` refers to the binary crate which no longer has those modules.

- [ ] **Step 5: Verify all 117 tests still pass after lib.rs refactor**

Run: `cargo test`
Expected: `test result: ok. 117 passed`

- [ ] **Step 6: Write a smoke test using MockClient**

Create `tests/mock_smoke_test.rs`:
```rust
mod common;
use common::MockClient;
use coda_cli::client::ToolCaller;
use serde_json::json;

#[tokio::test]
async fn mock_client_returns_queued_response() {
    let mock = MockClient::new();
    mock.enqueue_ok(json!({"name": "Test User"}));

    let result = mock.call_tool("whoami", json!({})).await.unwrap();
    assert_eq!(result["name"], "Test User");
    mock.assert_tool_called("whoami");
    assert_eq!(mock.call_count(), 1);
}

#[tokio::test]
async fn mock_client_returns_error() {
    let mock = MockClient::new();
    mock.enqueue_err(coda_cli::error::CodaError::Api {
        status: 404,
        message: "Not found".into(),
    });

    let result = mock.call_tool("whoami", json!({})).await;
    assert!(result.is_err());
}
```

- [ ] **Step 7: Run tests**

Run: `cargo test mock_smoke`
Expected: Both tests pass.

- [ ] **Step 8: Verify all tests pass (original 117 + 2 new)**

Run: `cargo test`
Expected: 119 passed.

- [ ] **Step 9: Commit**

```bash
git add src/lib.rs src/main.rs tests/ Cargo.toml
git commit -m "Add MockClient test infrastructure with fixture support and lib.rs"
```

---

## Chunk 2: Selective Retry + Auto-Pagination Fix

### Task 5: Make retry logic selective (only retriable errors)

**Files:**
- Modify: `src/error.rs` — add `is_retriable()` method
- Modify: `src/commands/compound.rs:699-719` — `call_with_retry()`
- Modify: `src/commands/sync.rs:806-838` — `call_with_retry()`

- [ ] **Step 1: Write test for is_retriable**

Add to `src/error.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retriable_errors() {
        assert!(CodaError::Api { status: 429, message: "rate limited".into() }.is_retriable());
        assert!(CodaError::Api { status: 500, message: "internal".into() }.is_retriable());
        assert!(CodaError::Api { status: 502, message: "bad gateway".into() }.is_retriable());
        assert!(CodaError::Api { status: 503, message: "unavailable".into() }.is_retriable());
        assert!(CodaError::Api { status: 409, message: "conflict".into() }.is_retriable());
    }

    #[test]
    fn non_retriable_errors() {
        assert!(!CodaError::Api { status: 400, message: "bad request".into() }.is_retriable());
        assert!(!CodaError::Api { status: 401, message: "unauthorized".into() }.is_retriable());
        assert!(!CodaError::Api { status: 403, message: "forbidden".into() }.is_retriable());
        assert!(!CodaError::Api { status: 404, message: "not found".into() }.is_retriable());
        assert!(!CodaError::Validation("bad input".into()).is_retriable());
        assert!(!CodaError::Other("generic error".into()).is_retriable());
    }
}
```

Note: `error.rs` doesn't currently have a `#[cfg(test)]` block. Create one. The test for `Http` variant is omitted because constructing a `reqwest::Error` in tests requires extra setup — verify Http retriability via integration tests instead.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test retriable`
Expected: FAIL — `is_retriable` not defined.

- [ ] **Step 3: Implement is_retriable on CodaError**

Add `is_retriable` to the existing `impl CodaError` block in `src/error.rs` (which already has `error_type()`):
```rust
pub fn is_retriable(&self) -> bool {
    match self {
        CodaError::Api { status, .. } => matches!(status, 409 | 429 | 500..=599),
        CodaError::Http(_) => true,  // network/connection errors are transient
        _ => false,
    }
}
```

Note: `CodaError::Http(reqwest::Error)` is the actual network error variant (timeouts, connection refused). `CodaError::Other(String)` is a generic catch-all and should NOT be retriable.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test retriable`
Expected: PASS.

- [ ] **Step 5: Update compound.rs call_with_retry**

In `src/commands/compound.rs`, update `call_with_retry` (line ~699) to check `is_retriable()`:

```rust
async fn call_with_retry(
    client: &dyn crate::client::ToolCaller,
    tool_name: &str,
    payload: Value,
    max_retries: u32,
) -> crate::error::Result<Value> {
    let mut last_err = None;
    let effective_retries = max_retries;

    for attempt in 0..=effective_retries {
        match client.call_tool(tool_name, payload.clone()).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if !e.is_retriable() || attempt == effective_retries {
                    return Err(e);
                }
                let delay = 1000 * (attempt as u64 + 1);
                crate::output::info(&format!(
                    "[retry] {tool_name} attempt {}/{effective_retries}: {e}. Retrying in {delay}ms...\n",
                    attempt + 1
                ));
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                last_err = Some(e);
            }
        }
    }

    Err(last_err.unwrap())
}
```

- [ ] **Step 6: Update sync.rs call_with_retry identically**

Apply the same `is_retriable()` check to `src/commands/sync.rs` `call_with_retry` (line ~806). Keep the 409-specific extra retry logic (doc readiness), but use `is_retriable()` for the gate.

- [ ] **Step 7: Verify all tests pass**

Run: `cargo test`
Expected: All pass (original + new retriable tests).

- [ ] **Step 8: Commit**

```bash
git add src/error.rs src/commands/compound.rs src/commands/sync.rs
git commit -m "Make retry logic selective: only retry 429/5xx/network errors"
```

---

### Task 6: Fix auto-pagination to never silently truncate

**Files:**
- Modify: `src/client.rs:207-276` — `auto_paginate()`
- Create: `tests/pagination_test.rs`

Since `auto_paginate` is a private method on `CodaClient`, we test the `add_pagination_metadata` helper from within `src/client.rs`'s existing test module.

- [ ] **Step 1: Add pagination metadata test inside client.rs**

Add to `src/client.rs` `#[cfg(test)] mod tests`:
```rust
#[test]
fn pagination_metadata_added_on_truncation() {
    let mut result = json!({
        "items": [{"id": 1}, {"id": 2}],
        "nextPageToken": "abc123"
    });
    // Simulate truncation by adding _pagination metadata
    add_pagination_metadata(&mut result, 1, false, Some("Network error on page 2".into()));

    assert_eq!(result["_pagination"]["complete"], false);
    assert_eq!(result["_pagination"]["pagesFetched"], 1);
    assert!(result["_pagination"]["error"].as_str().unwrap().contains("page 2"));
}

#[test]
fn pagination_metadata_not_added_when_complete() {
    let mut result = json!({
        "items": [{"id": 1}, {"id": 2}]
    });
    add_pagination_metadata(&mut result, 3, true, None);

    // No _pagination field when complete (clean output)
    assert!(result.get("_pagination").is_none());
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test pagination_metadata`
Expected: FAIL — `add_pagination_metadata` not defined.

- [ ] **Step 4: Implement add_pagination_metadata helper**

Add to `src/client.rs`:
```rust
fn add_pagination_metadata(
    result: &mut Value,
    pages_fetched: usize,
    is_complete: bool,
    error: Option<String>,
) {
    if is_complete {
        return; // Don't add metadata for complete results
    }
    let mut meta = serde_json::json!({
        "complete": false,
        "pagesFetched": pages_fetched,
    });
    if let Some(err_msg) = error {
        meta["error"] = Value::String(err_msg);
    }
    if let Some(obj) = result.as_object_mut() {
        obj.insert("_pagination".to_string(), meta);
    }
}
```

- [ ] **Step 5: Update auto_paginate to use the helper**

Modify `auto_paginate` (line ~207). Replace the silent `break` on error with tracking:

```rust
async fn auto_paginate(&self, tool_name: &str, original_payload: &Value, result: &mut Value) {
    // ... existing setup ...
    let mut pages_fetched = 1; // first page already in result
    let mut is_complete = true;
    let mut truncation_error = None;

    loop {
        // ... existing token extraction ...
        pages_fetched += 1;

        match self.call_tool_single(tool_name, next_payload).await {
            Ok(r) => {
                // ... existing merge logic ...
            }
            Err(e) => {
                if e.is_retriable() && pages_fetched <= 3 {
                    // Could retry, but for now just mark as truncated
                }
                is_complete = false;
                truncation_error = Some(format!("Page {} failed: {}", pages_fetched, e));
                crate::output::info(&format!(
                    "[paginate] Error on page {}. Results may be partial: {}\n",
                    pages_fetched, e
                ));
                break;
            }
        }
        // ... existing loop bounds check ...
    }

    // Clean up nextPageToken from final result
    if let Some(obj) = result.as_object_mut() {
        obj.remove("nextPageToken");
    }

    add_pagination_metadata(result, pages_fetched, is_complete, truncation_error);
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test pagination_metadata`
Expected: PASS.

- [ ] **Step 7: Add stderr warning when --pick is used on truncated result**

In `src/commands/tools.rs`, after pick extraction, check for `_pagination`:

```rust
// After picking fields, warn if result was truncated
if value.get("_pagination").is_some() {
    crate::output::info("[paginate] Warning: result was truncated. Use --pick _pagination to see details.\n");
}
```

- [ ] **Step 8: Verify all tests pass**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 9: Commit**

```bash
git add src/client.rs src/commands/tools.rs
git commit -m "Fix auto-pagination: never silently truncate, add _pagination metadata"
```

---

## Chunk 3: Compound Operations — Fail-Fast + Best-Effort

### Task 7: Refactor doc_scaffold error handling

**Files:**
- Modify: `src/commands/compound.rs:217-446`
- Create: `tests/compound_test.rs`

- [ ] **Step 1: Write failing tests for compound error scenarios**

Create `tests/compound_test.rs`:
```rust
mod common;
use common::MockClient;
use serde_json::json;

#[tokio::test]
async fn doc_scaffold_all_success_returns_complete_true() {
    let mock = MockClient::new();
    // Queue: document_create, page_create, content_modify
    mock.enqueue_ok(json!({"docUri": "coda://docs/abc", "browserLink": "https://coda.io/d/abc"}));
    mock.enqueue_ok(json!({"uri": "coda://docs/abc/pages/p1"}));
    mock.enqueue_ok(json!({"uri": "coda://docs/abc/pages/p1"})); // content_modify

    let payload = json!({
        "title": "Test Doc",
        "pages": [{"title": "Page 1", "content": "# Hello"}]
    });

    let result = coda_cli::commands::compound::execute(&mock, "doc_scaffold", payload).await.unwrap();
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
    assert!(result.is_err()); // Critical failure — no partial result
}

#[tokio::test]
async fn doc_scaffold_content_fails_returns_partial_with_errors() {
    let mock = MockClient::new();
    // document_create succeeds
    mock.enqueue_ok(json!({"docUri": "coda://docs/abc", "browserLink": "https://coda.io/d/abc"}));
    // page_create succeeds
    mock.enqueue_ok(json!({"uri": "coda://docs/abc/pages/p1"}));
    // content_modify fails (non-critical)
    mock.enqueue_err(coda_cli::error::CodaError::Api {
        status: 500,
        message: "Content insert failed".into(),
    });

    let payload = json!({
        "title": "Test Doc",
        "pages": [{"title": "Page 1", "content": "# Hello"}]
    });

    let result = coda_cli::commands::compound::execute(&mock, "doc_scaffold", payload).await.unwrap();
    assert_eq!(result["complete"], false);
    assert!(!result["errors"].as_array().unwrap().is_empty());
    assert!(result["docUri"].as_str().is_some()); // Doc was created
}
```

Also add a test for page creation failure mid-sequence:
```rust
#[tokio::test]
async fn doc_scaffold_page2_fails_is_critical_error() {
    let mock = MockClient::new();
    // document_create succeeds
    mock.enqueue_ok(json!({"docUri": "coda://docs/abc", "browserLink": "https://coda.io/d/abc"}));
    // page 1 create succeeds
    mock.enqueue_ok(json!({"uri": "coda://docs/abc/pages/p1"}));
    // page 1 content succeeds
    mock.enqueue_ok(json!({"uri": "coda://docs/abc/pages/p1"}));
    // page 2 create FAILS (critical)
    mock.enqueue_err(coda_cli::error::CodaError::Api {
        status: 500,
        message: "Page creation failed".into(),
    });

    let payload = json!({
        "title": "Test Doc",
        "pages": [
            {"title": "Page 1", "content": "# One"},
            {"title": "Page 2", "content": "# Two"},
            {"title": "Page 3", "content": "# Three"}
        ]
    });

    let result = coda_cli::commands::compound::execute(&mock, "doc_scaffold", payload).await;
    assert!(result.is_err()); // Page creation is critical — fail fast
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test doc_scaffold`
Expected: FAIL — return shape doesn't include `complete` field yet.

- [ ] **Step 3: Refactor doc_scaffold**

Modify `src/commands/compound.rs` `doc_scaffold()` function:

1. Doc creation failure → return `Err()` immediately (critical)
2. Page creation failure → return `Err()` immediately (critical — doc without pages is useless)
3. Content insertion failure → collect in `errors`, continue (non-critical)
4. Table creation failure → collect in `errors`, continue (non-critical)
5. Row insertion failure → collect in `errors`, continue (non-critical)
6. Add `"complete"` field to return value: `true` if errors is empty

The return value must always include `docUri`, `browserLink`, `pages` array (with per-page status), and `errors` array.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test doc_scaffold`
Expected: All 3 tests pass.

- [ ] **Step 5: Add per-page status to return shape**

Each page in the `pages` array gets a `status` field:
- `"ok"` — page created, content inserted, tables created
- `"partial"` — page created but content or tables failed
- `"failed"` — page creation itself failed (shouldn't happen given fail-fast, but defensive)

- [ ] **Step 6: Verify all tests pass**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 7: Commit**

```bash
git add src/commands/compound.rs tests/compound_test.rs
git commit -m "Refactor doc_scaffold: fail-fast for critical steps, best-effort for content/tables"
```

---

### Task 8: Apply same pattern to page_create_with_content

**Files:**
- Modify: `src/commands/compound.rs` — `page_create_with_content()` function

- [ ] **Step 1: Write failing test**

Add to `tests/compound_test.rs`:
```rust
#[tokio::test]
async fn page_create_with_content_page_fails_returns_error() {
    let mock = MockClient::new();
    mock.enqueue_err(coda_cli::error::CodaError::Api {
        status: 404,
        message: "Doc not found".into(),
    });

    let payload = json!({"uri": "coda://docs/abc", "title": "New Page", "content": "# Hi"});
    let result = coda_cli::commands::compound::execute(&mock, "page_create_with_content", payload).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn page_create_with_content_content_fails_returns_partial() {
    let mock = MockClient::new();
    mock.enqueue_ok(json!({"uri": "coda://docs/abc/pages/p1"})); // page_create
    mock.enqueue_err(coda_cli::error::CodaError::Api {
        status: 500,
        message: "Content failed".into(),
    });

    let payload = json!({"uri": "coda://docs/abc", "title": "New Page", "content": "# Hi"});
    let result = coda_cli::commands::compound::execute(&mock, "page_create_with_content", payload).await.unwrap();
    assert_eq!(result["complete"], false);
    assert!(result["uri"].as_str().is_some()); // Page was created
}
```

- [ ] **Step 2: Run test, verify failure, implement, verify pass**

Same pattern as Task 7.

- [ ] **Step 3: Commit**

```bash
git add src/commands/compound.rs tests/compound_test.rs
git commit -m "Apply fail-fast/best-effort to page_create_with_content"
```

---

## Chunk 4: Sync Atomic Writes + Manifest Consistency

### Task 9: Add manifest status field with backward compat

**Files:**
- Modify: `src/commands/sync.rs:133-163` — manifest types

- [ ] **Step 1: Write test for backward compat**

The actual `ManifestDocEntry` fields are `slug`, `title`, `synced_at`, `page_count`, `table_count` (snake_case, no `rows` field). The test must match the real struct.

Add to `src/commands/sync.rs` `#[cfg(test)] mod tests`:
```rust
#[test]
fn manifest_missing_status_defaults_to_complete() {
    let json = r#"{"version":1,"synced_at":"2026-01-01T00:00:00Z","docs":{"coda://docs/abc":{"slug":"test","title":"Test","synced_at":"2026-01-01T00:00:00Z","page_count":1,"table_count":0}}}"#;
    let manifest: SyncManifest = serde_json::from_str(json).unwrap();
    let entry = &manifest.docs["coda://docs/abc"];
    assert_eq!(entry.status, "complete"); // default when field is missing
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test manifest_missing_status`
Expected: FAIL — `status` field not on `ManifestDocEntry`.

- [ ] **Step 3: Add status field to ManifestDocEntry**

Add to the existing struct (preserving all current fields):
```rust
#[derive(Serialize, Deserialize, Clone)]
struct ManifestDocEntry {
    slug: String,
    title: String,
    synced_at: String,
    page_count: usize,
    table_count: usize,
    #[serde(default = "default_status")]
    status: String,
}

fn default_status() -> String {
    "complete".to_string()
}
```

Update all places that construct `ManifestDocEntry` to include `status: "complete".to_string()`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test manifest_missing_status`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/commands/sync.rs
git commit -m "Add status field to sync manifest with backward compat default"
```

---

### Task 10: Implement atomic sync writes

**Files:**
- Modify: `src/commands/sync.rs:196-342` — `sync_document()`

- [ ] **Step 1: Write test for atomic write behavior**

Add to sync.rs tests:
```rust
#[test]
fn sync_tmp_dir_cleaned_on_success() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let sync_tmp = root.join(".sync_tmp").join("test-doc");
    let docs_dir = root.join("docs").join("test-doc");

    // Simulate: tmp exists, docs doesn't
    std::fs::create_dir_all(&sync_tmp).unwrap();
    std::fs::write(sync_tmp.join("test.md"), "hello").unwrap();

    // Promote should move tmp → docs
    promote_sync_dir(&sync_tmp, &docs_dir).unwrap();

    assert!(!sync_tmp.exists());
    assert!(docs_dir.join("test.md").exists());
}
```

- [ ] **Step 2: Implement promote_sync_dir helper**

```rust
fn promote_sync_dir(tmp_dir: &Path, final_dir: &Path) -> Result<()> {
    // Phase 1: Move existing to backup
    let backup_dir = tmp_dir.parent().unwrap().join(
        format!("{}_old", tmp_dir.file_name().unwrap().to_string_lossy())
    );
    if final_dir.exists() {
        std::fs::rename(final_dir, &backup_dir).map_err(|e| {
            CodaError::Other(format!("Failed to backup existing dir: {e}"))
        })?;
    }

    // Phase 2: Move tmp to final
    std::fs::create_dir_all(final_dir.parent().unwrap())?;
    if let Err(e) = std::fs::rename(tmp_dir, final_dir) {
        // Restore backup on failure
        if backup_dir.exists() {
            let _ = std::fs::rename(&backup_dir, final_dir);
        }
        return Err(CodaError::Other(format!("Failed to promote sync dir: {e}")));
    }

    // Phase 3: Clean up backup
    if backup_dir.exists() {
        let _ = std::fs::remove_dir_all(&backup_dir);
    }

    Ok(())
}
```

- [ ] **Step 3: Add concurrent sync guard**

Before creating `.sync_tmp/<slug>/`, check if it already exists (another sync is in progress):
```rust
if sync_tmp_dir.exists() {
    return Err(CodaError::Other(format!(
        "Sync already in progress for '{}'. If this is stale, delete {} and retry.",
        slug, sync_tmp_dir.display()
    )));
}
```

- [ ] **Step 4: Add corrupted manifest recovery**

In `load_manifest()`, if JSON parsing fails, log a warning and return an empty manifest instead of propagating the error:
```rust
fn load_manifest(root: &Path) -> Result<SyncManifest> {
    let path = root.join("__sync_manifest.json");
    if !path.exists() {
        return Ok(SyncManifest::default());
    }
    match std::fs::read_to_string(&path).and_then(|s| Ok(serde_json::from_str(&s)?)) {
        Ok(m) => Ok(m),
        Err(e) => {
            crate::output::info(&format!(
                "[sync] Warning: manifest corrupted ({}). Treating as fresh sync.\n", e
            ));
            Ok(SyncManifest::default())
        }
    }
}
```

- [ ] **Step 5: Refactor sync_document to write to temp first**

Modify `sync_document()`:
1. Check concurrent sync guard (step 3)
2. Create `.coda/.sync_tmp/<slug>/` instead of `.coda/docs/<slug>/`
3. Write all pages, tables, rows to temp dir
4. On success: call `promote_sync_dir()`, set manifest status to `"complete"`
5. On failure: clean up temp dir, set manifest status to `"partial"`

- [ ] **Step 6: Write test for corrupted manifest recovery**

Add to sync.rs tests:
```rust
#[test]
fn corrupted_manifest_treated_as_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest_path = tmp.path().join("__sync_manifest.json");
    std::fs::write(&manifest_path, "not valid json{{{").unwrap();

    let manifest = load_manifest(tmp.path()).unwrap();
    assert!(manifest.docs.is_empty());
}
```

- [ ] **Step 7: Run all tests**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 8: Commit**

```bash
git add src/commands/sync.rs
git commit -m "Implement atomic sync writes with three-phase promotion, concurrent guard, and manifest recovery"
```

---

## Chunk 5: Quick Fixes

### Task 11: Make --polish non-fatal

**Files:**
- Modify: `src/main.rs:426-432`

- [ ] **Step 1: Change polish error handling**

Replace in `dispatch_tool()`:
```rust
// Before
if polish {
    let count = polish::polish_payload(&resolved_name, &mut payload).await?;
    if count > 0 {
        output::info(&format!("[polish] Polished {count} text field(s).\n"));
    }
}

// After
if polish {
    match polish::polish_payload(&resolved_name, &mut payload).await {
        Ok(count) if count > 0 => {
            output::info(&format!("[polish] Polished {count} text field(s).\n"));
        }
        Ok(_) => {}
        Err(e) => {
            output::info(&format!("[polish] Skipped: {e}. Proceeding with original text.\n"));
        }
    }
}
```

- [ ] **Step 2: Verify tests pass**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "Make --polish non-fatal: warn and proceed with original text on error"
```

---

### Task 12: Expand polish tool coverage

**Files:**
- Modify: `src/polish.rs:67-80` — `collect_polish_paths()`

- [ ] **Step 1: Write test for new tool coverage**

Add to `src/polish.rs` tests:
```rust
#[test]
fn paths_page_create_top_level_content() {
    let payload = json!({"uri": "coda://docs/abc", "title": "Test", "content": "# Hello world with enough text to polish"});
    let paths = collect_polish_paths("page_create", &payload);
    assert_eq!(paths, vec!["/content"]);
}

#[test]
fn paths_generic_content_field() {
    let payload = json!({"uri": "coda://docs/abc", "content": "Some long enough content to be polished here"});
    let paths = collect_polish_paths("some_unknown_tool", &payload);
    assert_eq!(paths, vec!["/content"]);
}

#[test]
fn paths_generic_no_content_field() {
    let payload = json!({"uri": "coda://docs/abc", "title": "Short"});
    let paths = collect_polish_paths("some_unknown_tool", &payload);
    assert!(paths.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test paths_page_create_top paths_generic`
Expected: FAIL.

- [ ] **Step 3: Add generic fallback to collect_polish_paths**

Update the match in `collect_polish_paths`:
```rust
fn collect_polish_paths(tool_name: &str, payload: &Value) -> Vec<String> {
    match tool_name {
        "content_modify" => collect_content_modify_paths(payload),
        "page_create_with_content" => {
            if payload.get("content").and_then(|v| v.as_str()).is_some() {
                vec!["/content".to_string()]
            } else {
                vec![]
            }
        }
        "doc_scaffold" => collect_doc_scaffold_paths(payload),
        // Generic fallback: any tool with a top-level "content" string field
        _ => {
            if payload.get("content").and_then(|v| v.as_str()).map_or(false, |s| s.len() >= 20) {
                vec!["/content".to_string()]
            } else {
                vec![]
            }
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test paths_`
Expected: All polish path tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/polish.rs
git commit -m "Expand polish to page_create and generic content field fallback"
```

---

### Task 13: Fix --pick key collisions

**Files:**
- Modify: `src/output.rs:113-120` — `print_picked_multi()`

- [ ] **Step 1: Write test for key collision**

Add to a new `tests/pick_test.rs`:
```rust
use serde_json::json;

#[test]
fn pick_multi_no_collision_uses_short_keys() {
    let value = json!({"name": "Alice", "email": "alice@example.com"});
    let paths = ["name", "email"];
    let resolved: Vec<&serde_json::Value> = paths.iter().map(|p| value.get(*p).unwrap()).collect();

    let result = coda_cli::output::build_picked_object(&paths, &resolved);
    assert_eq!(result["name"], "Alice");
    assert_eq!(result["email"], "alice@example.com");
}

#[test]
fn pick_multi_collision_uses_full_paths() {
    let value = json!({
        "pages": [
            {"title": "Goals"},
            {"title": "Tasks"}
        ]
    });
    let paths = ["pages.0.title", "pages.1.title"];
    let vals = [
        json!("Goals"),
        json!("Tasks"),
    ];
    let refs: Vec<&serde_json::Value> = vals.iter().collect();

    let result = coda_cli::output::build_picked_object(&paths, &refs);
    // Collision on "title" → use full paths
    assert_eq!(result["pages.0.title"], "Goals");
    assert_eq!(result["pages.1.title"], "Tasks");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test pick_multi`
Expected: FAIL — `build_picked_object` not defined.

- [ ] **Step 3: Extract build_picked_object from print_picked_multi**

In `src/output.rs`, extract the object construction into a public function:

```rust
pub fn build_picked_object(paths: &[&str], values: &[&Value]) -> Value {
    let keys: Vec<&str> = paths.iter().map(|p| p.rsplit('.').next().unwrap_or(p)).collect();

    // Detect collisions
    let has_collision = {
        let mut seen = std::collections::HashSet::new();
        keys.iter().any(|k| !seen.insert(k))
    };

    let mut obj = serde_json::Map::new();
    for (i, val) in values.iter().enumerate() {
        let key = if has_collision { paths[i] } else { keys[i] };
        obj.insert(key.to_string(), (*val).clone());
    }
    Value::Object(obj)
}

pub fn print_picked_multi(paths: &[&str], values: &[&Value]) -> crate::error::Result<()> {
    let obj = build_picked_object(paths, values);
    println!("{}", serde_json::to_string_pretty(&obj)?);
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test pick_multi`
Expected: PASS.

- [ ] **Step 5: Verify all tests pass**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 6: Commit**

```bash
git add src/output.rs tests/pick_test.rs
git commit -m "Fix --pick key collisions: use full dot-path when keys conflict"
```

---

## Chunk 6: Documentation Updates

### Task 14: Update CONTEXT.md with error recovery

**Files:**
- Modify: `CONTEXT.md`

- [ ] **Step 1: Add error recovery section**

Add after the existing content:
```markdown
## Error Recovery

### Compound Operations (doc_scaffold, page_create_with_content)

Results include a `complete` field and an `errors` array:
- `"complete": true` — all steps succeeded
- `"complete": false` — some non-critical steps failed, check `errors[]`

Each page in the result has a `status`: `"ok"`, `"partial"`, or `"failed"`.

If errors occurred, the doc/page still exists. Retry only the failed parts:
1. Read `errors[]` to identify what failed
2. Use the individual tool (e.g., `content_modify`) to retry

### Sync Status

After sync, check `.coda/docs/<slug>/__sync.json` for status:
- `"status": "complete"` — all pages and tables synced
- `"status": "partial"` — some items failed, re-sync with `--force`

### Pagination

Large results include `_pagination` metadata when truncated:
```json
{"items": [...], "_pagination": {"complete": false, "pagesFetched": 14, "error": "..."}}
```

### Cache

Tool schemas are cached for 24 hours. Force refresh: `shd discover --refresh`

### Polish

Requires `ANTHROPIC_API_KEY` env var. Non-fatal: if missing or API fails, original text is used with a stderr warning.
```

- [ ] **Step 2: Update skills files**

Update `skills/fundamentals/getting-started.md` to mention `--polish` and `--fuzzy`.
Update `skills/workflows/scaffold-doc.md` to document the `complete` field.
Create `skills/fundamentals/error-recovery.md` with the patterns above.

- [ ] **Step 3: Commit**

```bash
git add CONTEXT.md skills/
git commit -m "Update docs with error recovery patterns, polish, and cache TTL"
```

---

## Chunk 7: Live Integration Tests

### Task 15: Set up integration test feature flag

**Files:**
- Modify: `Cargo.toml` — add `[features]` section
- Create: `tests/integration/mod.rs`
- Create: `tests/integration/smoke_test.rs`

- [ ] **Step 1: Add feature flag to Cargo.toml**

```toml
[features]
default = []
integration = []
```

- [ ] **Step 2: Create integration test entry point**

Create `tests/integration_tests.rs`:
```rust
#![cfg(feature = "integration")]

mod common;

mod integration;
```

Create `tests/integration/mod.rs`:
```rust
mod smoke_test;
```

- [ ] **Step 3: Write smoke test**

Create `tests/integration/smoke_test.rs`:
```rust
use coda_cli::client::{CodaClient, ToolCaller};
use serde_json::json;

fn get_client() -> CodaClient {
    let token = std::env::var("CODA_API_TOKEN")
        .expect("CODA_API_TOKEN must be set for integration tests");
    CodaClient::new(token).unwrap()
}

#[tokio::test]
async fn whoami_returns_name() {
    let client = get_client();
    let result = client.call_tool("whoami", json!({})).await.unwrap();
    assert!(result.get("name").is_some(), "whoami should return a name field");
}

#[tokio::test]
async fn discover_returns_tools() {
    let client = get_client();
    let tools = client.fetch_tools().await.unwrap();
    assert!(!tools.is_empty(), "discover should return at least one tool");
}

#[tokio::test]
async fn nonexistent_tool_returns_error() {
    let client = get_client();
    let result = client.call_tool("definitely_not_a_real_tool_12345", json!({})).await;
    assert!(result.is_err());
}
```

- [ ] **Step 4: Run integration tests**

Run: `cargo test --features integration`
Expected: 3 integration tests pass (requires `CODA_API_TOKEN` set).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml tests/integration_tests.rs tests/integration/
git commit -m "Add live integration test suite behind --features integration flag"
```

---

### Task 16: Final verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass (original 117 + new fixture/mock/unit tests).

- [ ] **Step 2: Run clippy**

Run: `cargo clippy`
Expected: No new warnings related to our changes.

- [ ] **Step 3: Run integration tests**

Run: `cargo test --features integration`
Expected: All integration tests pass.

- [ ] **Step 4: Manual smoke test**

```bash
shd
shd --help
shd whoami --pick name
shd discover --compact | head -20
```

Expected: Clean output, no regressions.

- [ ] **Step 5: Final commit and push**

```bash
git push origin main
```
