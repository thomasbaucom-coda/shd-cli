# shd-cli

Agent-first command-line interface for [Coda](https://coda.io). Built in Rust. Inspired by [Google's Workspace CLI](https://github.com/googleworkspace/cli) and [Justin Poehnelt's CLI-for-agents guidance](https://justin.poehnelt.com/posts/rewrite-your-cli-for-ai-agents/).

## Why a CLI instead of an MCP?

| | MCP | CLI |
|---|---|---|
| Bulk writes | One tool call at a time through the LLM | `python3 generate.py \| coda rows import` — zero LLM round trips |
| Composability | Locked inside the model's tool loop | Pipes, scripts, `jq`, `curl`, cron — standard Unix |
| Auditability | Buried in chat history | `--dry-run`, shell history, CI logs |
| Startup | JSON-RPC handshake + session | 13ms cold start |
| Batch import | 1 row per tool call (burns tokens) | 500 rows per API call, auto-batched from stdin |

The CLI also ships an MCP server (`coda mcp`) for contexts where MCP is needed.

## Install

```bash
# Option 1: npm (builds from source automatically)
cd shd-cli
npm install
# Binary available at npm/bin/coda

# Option 2: Cargo directly
cargo build --release
# Binary at target/release/coda

# Option 3: Cargo install (adds to PATH)
cargo install --path .
```

## Authentication

```bash
# Option 1: Environment variable (preferred for agents)
export CODA_API_TOKEN="your-token"

# Option 2: Interactive login (stored in ~/.config/coda/credentials)
coda auth login

# For internal tool commands (table create, content write, etc.)
# you need an MCP-scoped token from:
# https://coda.io/account?openDialog=CREATE_API_TOKEN&scopeType=mcp#apiSettings
```

## Quick Start

```bash
# List your docs
coda docs list --limit 5

# Get a doc
coda docs get <docId>

# List tables and rows
coda tables list <docId>
coda rows list <docId> <tableId> --fields "Name,Status" --limit 10

# Create a doc
coda docs create --title "My Project"

# Upsert rows
coda rows upsert <docId> <tableId> --json '{
  "rows": [{"cells": [{"column": "Name", "value": "Alice"}]}]
}'

# Bulk import from stdin (auto-batched)
cat data.ndjson | coda rows import <docId> <tableId>

# Schema introspection (no network call, 13ms)
coda schema rows.list
```

## Commands

### Public API

| Command | Description |
|---------|-------------|
| `auth login\|status\|logout` | Manage authentication |
| `whoami` | Current user info |
| `docs list\|get\|create\|delete` | Manage docs |
| `pages list\|get\|create\|update\|delete\|content` | Manage pages |
| `tables list\|get` | List and inspect tables |
| `columns list\|get` | List and inspect columns |
| `rows list\|get\|upsert\|update\|delete\|push-button\|import` | Full row CRUD + bulk import |
| `formulas list\|get` | List and inspect formulas |
| `controls list\|get` | List and inspect controls |
| `folders list\|get\|create\|delete` | Manage folders |
| `permissions list\|metadata\|add\|remove` | Manage doc sharing |
| `resolve-url` | Decode a Coda URL to resource IDs |
| `schema` | Inspect API schema (offline) |
| `mcp` | Start MCP server over stdio |

### Internal Tool API (requires MCP-scoped token)

| Command | Description |
|---------|-------------|
| `tool table-create` | Create a table with typed columns |
| `tool table-add-rows` | Add rows (bulk, typed) |
| `tool table-add-columns` | Add columns to a table |
| `tool table-delete-rows` | Delete rows |
| `tool table-update-rows` | Update rows |
| `tool import-rows` | Bulk import from stdin (100/batch) |
| `tool content-modify` | Write page content (markdown, callouts, code blocks) |
| `tool comment-manage` | Add, reply to, delete comments |
| `tool formula-create` | Create a named formula |
| `tool formula-execute` | Evaluate a CFL expression |
| `tool view-configure` | Configure view filters and layout |
| `tool raw` | Call any internal tool by name |

## Global Flags

| Flag | Description |
|------|-------------|
| `--output json\|table\|ndjson` | Output format (default: `json`) |
| `--dry-run` | Preview request without executing |
| `--token <TOKEN>` | Override the stored token |
| `--page-all` | Auto-paginate, stream as NDJSON (on `docs list`, `rows list`) |
| `--fields "Col1,Col2"` | Limit row output to specific columns |

## Agent Design Principles

This CLI follows the [agent-first CLI design](https://justin.poehnelt.com/posts/rewrite-your-cli-for-ai-agents/) principles:

- **Raw JSON payloads** — every mutation accepts `--json` for the full API body, plus `--json -` to read from stdin
- **Schema introspection** — `coda schema rows.list` returns params, types, and response schemas from the embedded OpenAPI spec. No network call, 13ms.
- **Context window discipline** — `--fields` limits row columns, `--page-all` streams NDJSON instead of buffering, `--limit` caps results
- **Input hardening** — rejects path traversal (`../`), control characters, query injection (`?`, `#`), percent-encoding bypasses (`%2e`)
- **Dry-run safety** — `--dry-run` on every mutation shows the exact HTTP request without sending it. Works without auth.
- **Structured errors** — all errors are JSON on stderr with exit code 1
- **Agent skill files** — 12 skill files in `skills/` encoding invariants agents can't intuit

## Architecture

```
src/
  main.rs          CLI definition (clap) and command dispatch
  client.rs        HTTP client for both public API and internal tool endpoint
  auth.rs          Token resolution (flag > env > credential file)
  validate.rs      Input validation + JSON stdin support
  output.rs        json | table | ndjson formatters
  paginate.rs      Auto-pagination with field filtering
  error.rs         Structured error types
  commands/
    docs.rs        Public API: docs CRUD
    pages.rs       Public API: pages CRUD
    tables.rs      Public API: table reads
    columns.rs     Public API: column reads
    rows.rs        Public API: row CRUD + bulk import
    formulas.rs    Public API: formula reads
    controls.rs    Public API: control reads
    folders.rs     Public API: folder CRUD
    permissions.rs Public API: ACL management
    tools.rs       Internal tool endpoint: tables, content, comments, formulas, views
    mcp.rs         MCP server (stdio JSON-RPC, 24 tools)
    schema.rs      OpenAPI schema introspection
    resolve_url.rs URL-to-ID decoder
    whoami.rs      Current user info
    auth_cmd.rs    Auth management CLI
```

## Two API Surfaces

The CLI uses two Coda API surfaces:

| Surface | Endpoint | Token | Capabilities |
|---------|----------|-------|-------------|
| **Public API** | `/apis/v1/` | Standard API token | Docs, pages, rows (CRUD/upsert), columns (read), formulas (read), folders, permissions |
| **Internal Tool API** | `/apis/mcp/vbeta/docs/{docId}/tool` | MCP-scoped token | Table creation, column management, content writing, comments, formula creation, view configuration |

The public API can't create tables or write page content. The internal tool API can do everything but requires an MCP-scoped token (generate at coda.io/account with MCP scope).

**Token scopes are not yet unified** — an MCP-scoped token can't call the public API, and a standard token can't call the tool endpoint. This is a known limitation being addressed on the Coda side.

## Skills

The `skills/` directory contains agent skill files (markdown with YAML frontmatter) that teach AI agents how to use the CLI effectively:

| Skill | Description |
|-------|-------------|
| `coda-shared` | Auth, global flags, safety rules |
| `coda-docs` | Doc CRUD |
| `coda-pages` | Page management |
| `coda-tables` | Table and column inspection |
| `coda-rows` | Row CRUD, field filtering, bulk operations |
| `coda-permissions` | Sharing and ACL management |
| `coda-tool-tables` | Table creation, typed columns, views |
| `coda-tool-content` | Page content, comments, formulas |
| `recipe-build-doc` | End-to-end: create doc with tables and data |
| `recipe-export-table` | Export table to NDJSON for processing |
| `recipe-create-tracker` | Create a project tracker from a template |
| `recipe-sync-data` | Pipe external data (GitHub, APIs, CSV) into Coda |

## Example: Build a Complete Doc

```bash
# Create doc
DOC=$(coda docs create --title "Q2 Planning" | jq -r '.id')
sleep 3

# Create pages
coda pages create "$DOC" --name "Tasks"
TASKS_PAGE=$(coda pages list "$DOC" | jq -r '.items[-1].id')

# Create table with typed columns (MCP token required)
RESULT=$(coda tool table-create "$DOC" "$TASKS_PAGE" \
  --name "Tasks" \
  --columns '[
    {"name":"Task","isDisplayColumn":true},
    {"name":"Status","format":{"type":"sl","selectOptions":["To Do","In Progress","Done"]}},
    {"name":"Priority","format":{"type":"sl","selectOptions":["Low","Medium","High"]}},
    {"name":"Due","format":{"type":"dp"}}
  ]')
TABLE=$(echo "$RESULT" | jq -r '.tableId')
COLS=$(echo "$RESULT" | jq -r '[.columns[].columnId] | @json')

# Import 100 rows from a script
python3 generate_tasks.py | coda tool import-rows "$DOC" "$TABLE" --columns "$COLS"

# Configure a filtered view
coda tool view-configure "$DOC" "$TABLE" --name "Active" --filter 'Status != "Done"'
```

## Stats

| Metric | Value |
|--------|-------|
| Language | Rust |
| Binary size | 6.7MB |
| Cold startup | ~13ms |
| Rust LOC | ~4,000 |
| CLI commands | 16 top-level + 12 tool subcommands |
| MCP tools | 24 |
| Agent skills | 12 |
| Dependencies | 0 runtime (static binary) |

## License

MIT
