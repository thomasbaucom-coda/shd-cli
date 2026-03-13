# Agent-Ready Hardening — Design Spec

**Date:** 2026-03-13
**Goal:** Make SHD CLI reliable for real agent workflows. Agents should never get silently truncated data, half-built docs with no indication of failure, or inconsistent filesystem state.

**Approach:** Test-first for complex issues (sync, compound ops, pagination), direct fixes for obvious ones (polish, pick, docs). Fixture-based mocks for fast CI, live integration tests behind `--features integration`.

---

## 1. Test Infrastructure

### 1.1 Fixture System

Record real Coda API responses as JSON fixtures. A `MockClient` replays them deterministically.

**Directory structure:**

```
tests/
├── fixtures/
│   ├── whoami.json
│   ├── tools_list.json
│   ├── document_create.json
│   ├── document_read.json
│   ├── page_create.json
│   ├── page_read.json
│   ├── page_read_with_content.json
│   ├── table_read_rows.json
│   ├── table_read_rows_page2.json
│   └── errors/
│       ├── 401_unauthorized.json
│       ├── 404_not_found.json
│       ├── 429_rate_limited.json
│       └── tool_not_found.json
├── integration/
│   ├── mod.rs
│   ├── sync_test.rs
│   ├── compound_test.rs
│   └── pagination_test.rs
└── common/
    └── mock_client.rs
```

### 1.2 MockClient Design

The `MockClient` wraps a queue of fixture responses. Tests push expected responses, then call the function under test. Assertions verify the right tools were called with the right payloads.

```rust
struct MockClient {
    calls: Vec<(String, Value)>,           // recorded (tool_name, payload) pairs
    responses: VecDeque<Result<Value>>,     // queued responses
}

impl MockClient {
    fn enqueue(&mut self, response: Result<Value>);
    fn assert_called(&self, tool_name: &str, payload_matcher: impl Fn(&Value) -> bool);
}
```

This requires extracting a trait from `CodaClient` so both real and mock implementations can be used:

```rust
#[async_trait]
trait ToolCaller {
    async fn call_tool(&self, tool: &str, payload: Value) -> Result<Value>;
    async fn fetch_tools(&self) -> Result<Vec<Value>>;
}
```

Functions in `commands/` that currently take `&CodaClient` will take `&dyn ToolCaller` instead.

**Note on `dry_run_tool`:** The `dry_run_tool` method is NOT part of the `ToolCaller` trait — it's specific to `CodaClient`. Dry-run is handled at the CLI dispatch layer in `main.rs` before calling into trait-based command functions. This keeps the trait minimal.

### 1.3 Live Integration Tests

Behind `#[cfg(feature = "integration")]`. Require `CODA_API_TOKEN` env var. Use a dedicated test doc (created once, reused). Run with `cargo test --features integration`.

Tests:
- `whoami` returns expected structure
- `discover` returns tools with `inputSchema`
- Create doc, sync it, verify filesystem, delete doc
- Pagination: read a table with 100+ rows, verify all returned
- `doc_scaffold` end-to-end: create doc with pages/tables, verify structure, delete
- `page_create_with_content` end-to-end: add page to existing doc, verify content
- `table_search` against a table with known data
- Error handling: call a non-existent tool, verify structured error response

---

## 2. Auto-Pagination: Never Silently Truncate

**File:** `src/client.rs` — `auto_paginate()`

**Current behavior:** On page N failure, logs to stderr and returns pages 1..N-1. With `--quiet`, the warning is suppressed entirely.

**New behavior:** Enrich the returned `Value` with pagination metadata inline. The `call_tool` return type stays `Result<Value>` — no signature change needed. Internally, `auto_paginate` uses a helper struct to track state, then embeds `_pagination` into the final JSON value before returning.

When pagination is incomplete, the JSON output includes:

```json
{
  "items": [...],
  "_pagination": {
    "complete": false,
    "pagesFetched": 14,
    "error": "Page 15 failed: HTTP 429 rate limited"
  }
}
```

When `--pick` is used on a truncated result, emit a stderr warning:
`[paginate] Warning: result was truncated. Use --pick _pagination to see details.`

**Retry selectivity:** Only retry on retriable errors (429 rate limit, 5xx server errors, network timeouts). Do NOT retry 400 (bad request), 401 (unauthorized), 403 (forbidden), or 404 (not found) — these waste time and confuse agents.

**Tests:**
- Pagination completes normally (3 pages, all succeed)
- Pagination fails on page 2 of 3 (returns page 1 items + truncation metadata)
- Single page (no pagination needed)
- Empty result set
- Non-retriable error (400) is not retried

---

## 3. Compound Operations: Fail-Fast + Best-Effort

**File:** `src/commands/compound.rs`

### 3.1 Failure Classification

Split compound steps into **critical** (fail fast) and **non-critical** (best effort):

| Operation | Step | Classification |
|-----------|------|----------------|
| `doc_scaffold` | Create doc | Critical |
| `doc_scaffold` | Create page | Critical |
| `doc_scaffold` | Insert page content | Non-critical |
| `doc_scaffold` | Create table | Non-critical |
| `doc_scaffold` | Insert rows | Non-critical |
| `page_create_with_content` | Create page | Critical |
| `page_create_with_content` | Insert content | Non-critical |
| `doc_summarize` | Read doc | Critical |
| `doc_summarize` | Read pages | Non-critical |

### 3.2 Return Shape

All compound operations return a consistent shape:

```json
{
  "docUri": "coda://docs/abc",
  "browserLink": "https://coda.io/d/...",
  "pages": [
    {"title": "Goals", "uri": "coda://docs/abc/pages/xyz", "status": "ok"},
    {"title": "Tasks", "uri": "coda://docs/abc/pages/qrs", "status": "ok"},
    {"title": "Notes", "uri": null, "status": "failed", "error": "HTTP 500"}
  ],
  "errors": [
    "Page 'Notes': HTTP 500 Internal Server Error"
  ],
  "complete": false
}
```

The `complete` field is `true` only when `errors` is empty. Agents can check this single field.

The per-page `status` reflects the full outcome for that page (creation + content + tables). If page creation succeeded but content insertion failed, status is `"partial"` with an `error` field explaining what failed. If page creation itself failed, status is `"failed"`.

**Retry selectivity:** Same as pagination — only retry 429/5xx/network errors. 400/401/403/404 fail immediately.

### 3.3 Tests

- `doc_scaffold` all steps succeed → `complete: true`, empty `errors`
- `doc_scaffold` doc creation fails → immediate error return, no partial result
- `doc_scaffold` page 2 of 3 fails → returns doc + page 1, error for page 2, page 3 still attempted
- `doc_scaffold` content insertion fails → page exists but content missing, error recorded
- `doc_scaffold` table row insertion fails → table exists with columns, rows missing, error recorded
- `page_create_with_content` page fails → immediate error
- `page_create_with_content` content fails → page exists, content missing, error recorded

---

## 4. Sync: Atomic Writes + Manifest Consistency

**File:** `src/commands/sync.rs`

### 4.1 Atomic Writes via Temp Directory

Instead of writing directly to `.coda/docs/<slug>/`, write to `.coda/.sync_tmp/<slug>/` first. After all pages and tables for a doc succeed, rename the temp directory to the final location.

```
Sync flow:
1. Create .coda/.sync_tmp/<slug>/
2. Write all pages, tables, rows to temp
3. If all succeed:
   a. Rename old .coda/docs/<slug>/ → .coda/.sync_old/<slug>/  (atomic, preserves old data)
   b. Rename .coda/.sync_tmp/<slug>/ → .coda/docs/<slug>/      (atomic)
   c. Delete .coda/.sync_old/<slug>/                            (cleanup, safe to fail)
   d. Update manifest with status: "complete"
4. If step 3b fails (rename to final location):
   a. Rename .coda/.sync_old/<slug>/ back to .coda/docs/<slug>/ (restore old data)
   b. Clean up .coda/.sync_tmp/<slug>/
   c. Return error
5. If any page/table fetch fails during step 2:
   a. Clean up .coda/.sync_tmp/<slug>/
   b. Update manifest with status: "partial", record which pages/tables succeeded
   c. Return error with details
6. Update INDEX.md
```

This three-phase rename ensures that if the process crashes between removing old and placing new, the old data is recoverable from `.sync_old/`.

### 4.2 Manifest Status Tracking

```json
{
  "docs": {
    "coda://docs/abc": {
      "title": "My Doc",
      "slug": "my-doc-abc",
      "syncedAt": "2026-03-13T14:00:00Z",
      "status": "complete",
      "pages": 5,
      "tables": 2,
      "rows": 150
    }
  }
}
```

A `"status": "partial"` entry means the agent should re-sync with `--force`.

**Backward compatibility:** Existing `__sync.json` files from before this change will not have a `status` field. The code must default missing `status` to `"complete"` so pre-existing syncs are treated as valid.

### 4.3 Corrupted Manifest Recovery

If `__sync.json` or manifest fails to parse, treat as empty and log a warning. Never fail the entire sync because of a corrupt cache file.

### 4.4 Tests

- Full sync succeeds → all files present, manifest says "complete"
- Sync fails on page 3 → temp dir cleaned up, manifest says "partial"
- Sync with corrupted manifest → treated as fresh sync, warning emitted
- Sync with `--force` on "partial" doc → re-syncs everything
- Concurrent sync guard: if `.sync_tmp/<slug>` already exists, abort with message

---

## 5. Quick Fixes

### 5.1 Polish Non-Fatal

**File:** `src/main.rs` (dispatch_tool) and `src/polish.rs`

Change:
```rust
// Before (fatal)
let count = polish::polish_payload(&resolved_name, &mut payload).await?;

// After (non-fatal)
match polish::polish_payload(&resolved_name, &mut payload).await {
    Ok(count) if count > 0 => output::info(&format!("[polish] Polished {count} text field(s).\n")),
    Ok(_) => {},
    Err(e) => output::info(&format!("[polish] Skipped: {e}. Proceeding with original text.\n")),
}
```

### 5.2 Polish Tool Coverage

**File:** `src/polish.rs` — `collect_polish_paths()`

Add support for:
- `page_create` (if it has a `content` field)
- `page_update` (if it has a `content` field)
- Generic fallback: any payload with a top-level `content` or `markdown` string field

### 5.3 Pick Key Collisions

**File:** `src/commands/tools.rs` — multi-pick logic

When two paths resolve to the same key (e.g., `pages.0.title` and `pages.1.title` both → `title`), use the full dot-path as the key:

```json
{"pages.0.title": "Goals", "pages.1.title": "Tasks"}
```

Only triggers when there's an actual collision. Single-segment keys stay as-is.

The collision detection belongs in `output::print_picked_multi` (not in `pick_fields`), since that's where keys are assembled from paths.

### 5.4 Sanitize Pattern Review

**File:** `src/sanitize.rs`

Review needed: the current patterns may already be sufficiently specific (e.g., `"disregard all previous"` not just `"disregard"`). Verify each pattern against the actual code before changing. Only tighten patterns that demonstrably match legitimate text. Keep broad patterns for obvious attacks (`</system>`, `<|im_start|>`).

---

## 6. Documentation Updates

### 6.1 CONTEXT.md Additions

- **Error recovery section**: What to do when `errors[]` is non-empty, when `complete: false`, when sync status is `"partial"`
- **Cache TTL**: Mention 24h TTL, how to force refresh
- **Polish requirements**: Needs `ANTHROPIC_API_KEY`, non-fatal if missing
- **Output format note**: `--output table` is suboptimal for compound results (nested arrays render as `[3 items]`). Recommend `--output json` for compound operations in documentation.
- **`--quiet` and pagination**: When `--quiet` is set and pagination truncates, the `_pagination` metadata in JSON is the only signal. This is correct behavior — document it.

### 6.2 Skills Updates

- `skills/fundamentals/getting-started.md`: Add `--polish` and `--fuzzy` flags
- `skills/workflows/scaffold-doc.md`: Document `complete` field and error recovery
- `skills/workflows/create-then-sync.md`: Document sync status tracking
- New: `skills/fundamentals/error-recovery.md` — how agents should handle partial failures

---

## 7. Implementation Order

1. **Extract `ToolCaller` trait** — enables mock testing without touching behavior
2. **Build fixture system + MockClient** — test infrastructure
3. **Write failing tests** for pagination, compound ops, sync
4. **Fix auto-pagination** — return PaginatedResult
5. **Fix compound operations** — fail-fast/best-effort split
6. **Fix sync** — atomic writes + manifest status
7. **Quick fixes** — polish, pick, sanitize
8. **Documentation** — CONTEXT.md, skills
9. **Live integration tests** — behind feature flag

---

## Phase C Preview (Separate Spec)

After Phase B ships:
- Full JSON Schema validation (`jsonschema` crate)
- Streaming NDJSON writes for large tables (memory optimization)
- MCP server concurrent request handling
- `shd cache clear` command
- Sync file locking for concurrent runs
- Rate limit retry with backoff
