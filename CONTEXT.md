# shd CLI — Agent Context

This file is for AI agents using the CLI. For developer/contributor docs, see `CLAUDE.md`.

## Architecture

`shd` is a fully dynamic CLI for Coda. There are no hardcoded resource commands — all API interaction goes through a single tool endpoint. Tools are discovered at runtime from Coda's MCP endpoint and cached locally.

Two categories of tools:
- **API tools** (28+): Direct Coda operations like `document_read`, `page_create`, `table_read_rows`. Discovered via `shd discover`.
- **Compound operations** (4): Synthetic tools that orchestrate multiple API calls: `doc_scaffold`, `page_create_with_content`, `doc_summarize`, `table_search`.

Core syntax: `shd <tool_name> --json '{"key":"value"}'`

## Essential: Always Use --pick

API responses can be hundreds of tokens. Use `--pick` to extract only what you need.

```bash
# Bad: returns entire user object (~200 tokens)
shd whoami

# Good: returns just the name (~5 tokens)
shd whoami --pick name

# Multi-pick: returns JSON object with selected fields
shd page_create --json '{...}' --pick canvasUri,pageUri

# Dot paths into nested objects
shd doc_summarize --json '{...}' --pick pages.0.title
```

## Discovering Tools

```bash
shd discover                           # List all tools
shd discover --filter table            # Filter by keyword
shd discover content_modify --compact  # Agent-friendly 5-line summary
shd discover table_create              # Full schema with descriptions
```

Compound tools (`doc_scaffold`, `page_create_with_content`, `doc_summarize`, `table_search`) appear alongside API tools in discovery output.

If you get a `contract_changed` error, run `shd discover --refresh` — a tool may have been renamed.

## The coda:// URI Scheme

All Coda resources are addressed by URI:

| URI pattern | Example |
|-------------|---------|
| `coda://docs/{docId}` | `coda://docs/AbCdEf` |
| `coda://docs/{docId}/pages/{pageId}` | `coda://docs/AbCdEf/pages/section-XyZ` |
| `coda://docs/{docId}/canvases/canvas-{id}` | `coda://docs/AbCdEf/canvases/canvas-123` |
| `coda://docs/{docId}/tables/grid-{id}` | `coda://docs/AbCdEf/tables/grid-456` |

You get these URIs from tool responses (`docUri`, `pageUri`, `canvasUri`, `tableUri`). Never fabricate them — always discover via API calls or synced CONTEXT.md files.

## Payload Delivery

```bash
# Inline (small payloads)
shd page_create --json '{"uri":"coda://docs/abc","title":"My Page"}'

# From file (large payloads, no shell escaping issues)
shd doc_scaffold --json @blueprint.json

# From stdin (piping)
echo '{"uri":"coda://docs/abc"}' | shd doc_summarize --json -
```

## Sync: Local Filesystem Access

`shd sync` materializes a Coda document to `.coda/` as readable files:

```bash
shd sync --doc-url "https://coda.io/d/My-Doc_dAbCdEf"
```

This creates:
```
.coda/
├── INDEX.md                              # All synced docs
└── docs/<slug>/
    ├── CONTEXT.md                        # Pages, tables, columns for this doc
    ├── pages/<slug>.md                   # Page content as markdown
    └── pages/tables/<slug>/rows.ndjson   # Flattened table rows
```

**Reading synced data:**
1. Start with `.coda/INDEX.md` to see what's available
2. Read `.coda/docs/<slug>/CONTEXT.md` for a specific doc's structure
3. Read `.md` files for page content, `rows.ndjson` for table data
4. Grep across `rows.ndjson` files to search all tables

**When to sync vs API:** Sync for reading (bulk exploration, searching). API for writing (creating, updating).

**Not every doc is synced.** If the doc you need isn't in `.coda/`, run `shd sync --doc-url "<url>"` first.

**The `--sync` flag** on any tool call triggers a background sync when the response contains a `docUri`:
```bash
shd doc_scaffold --json @blueprint.json --sync
# → Creates doc, prints result immediately
# → Background process syncs to .coda/ (~20s later)
```

## Error Patterns

All errors are JSON on stderr with exit code 1:

| Error type | Meaning | What to do |
|------------|---------|------------|
| `api_error` | HTTP error from Coda (404, 429, etc.) | Check the message, retry on 429 |
| `validation_error` | Bad payload shape or missing fields | Check required fields via `shd discover <tool> --compact` |
| `contract_changed` | Tool renamed or schema changed | Run `shd discover --refresh` |
| `auth_required` | No token | Run `shd auth login` or set `CODA_API_TOKEN` |

## Chaining Operations

Common multi-step patterns:

**Understand then create:**
```bash
shd doc_summarize --json '{"uri":"coda://docs/SOURCE"}' --pick pages,tables
# Read the structure, then scaffold a similar doc
shd doc_scaffold --json @new-blueprint.json --sync
```

**Sync then search:**
```bash
shd sync --doc-url "https://coda.io/d/..."
# Read .coda/docs/*/CONTEXT.md to find the table
# Grep rows.ndjson for specific values
```

**Create then iterate:**
```bash
shd doc_scaffold --json @blueprint.json --sync --pick docUri
# Wait for background sync, then read CONTEXT.md
# Add more content via page_create_with_content
```

## Skills Directory

Detailed usage guides are in `skills/`:
- `fundamentals/` — Getting started, discovering tools, sync and reading, error recovery
- `workflows/` — Scaffolding docs, creating pages, summarizing, searching, row operations

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

After sync, the manifest tracks status per doc:
- `"status": "complete"` — all pages and tables synced
- `"status": "partial"` — some items failed, re-sync with `--force`

Sync uses atomic writes — `.coda/docs/<slug>/` is either fully synced or not present. Partial writes go to a temp directory and are promoted only on success.

### Pagination

Large results include `_pagination` metadata when truncated:
```json
{"items": [...], "_pagination": {"complete": false, "pagesFetched": 14, "error": "..."}}
```

When using `--pick` on a truncated result, a stderr warning is emitted.

### Cache

Tool schemas are cached for 24 hours. Force refresh: `shd discover --refresh`

### Polish

The `--polish` flag sends text through Claude for grammar/style cleanup before writing to Coda.
- Requires `ANTHROPIC_API_KEY` environment variable
- Non-fatal: if missing or API fails, original text is used with a stderr warning
- Works with any tool that has a `content` field (20+ chars), plus specific support for `content_modify` and `doc_scaffold`
