# shd-cli

Agent-first command-line interface for [Coda](https://coda.io). Built in Rust. All commands are dispatched dynamically to Coda's MCP tool endpoint — new tools work without a CLI rebuild.

Inspired by [Google's Workspace CLI](https://github.com/googleworkspace/cli) and [Justin Poehnelt's CLI-for-agents guidance](https://justin.poehnelt.com/posts/rewrite-your-cli-for-ai-agents/).

## Install

```bash
# Option 1: npm (builds from source automatically)
cd shd-cli
npm install
# Binary available at npm/bin/shd

# Option 2: Cargo directly
cargo build --release
# Binary at target/release/shd

# Option 3: Cargo install (adds to PATH)
cargo install --path .
```

## Authentication

```bash
# Option 1: Environment variable (preferred for agents)
export CODA_API_TOKEN="your-token"

# Option 2: Interactive login (stored in ~/.config/coda/credentials)
shd auth login

# Generate an MCP-scoped token at:
# https://coda.io/account?openDialog=CREATE_API_TOKEN&scopeType=mcp#apiSettings
```

## Quick Start

```bash
# Discover available tools
shd discover

# Inspect a specific tool's schema
shd discover table_create

# Call any tool by name
shd whoami
shd document_list --json '{"limit": 5}'
shd table_list --json '{"docId": "<docId>"}'

# Create a doc
shd document_create --json '{"title": "My Project"}'

# Add rows to a table
shd table_add_rows --json '{"docId": "<docId>", "tableId": "<tableId>", "rows": [...]}'

# Read payload from stdin
echo '{"docId": "abc123"}' | shd table_list --json -

# Extract a specific field from the response
shd whoami --pick name

# Fuzzy-match a tool name against cached tools
shd tbl_create --fuzzy --json '{...}'

# Preview request without executing (works without auth)
shd document_create --json '{"title": "Test"}' --dry-run
```

## Commands

The CLI has only 4 built-in commands. Everything else is a dynamically dispatched tool call.

| Command | Description |
|---------|-------------|
| `auth login\|status\|logout` | Manage authentication |
| `discover [tool_name]` | List all tools or inspect a specific tool's schema |
| `mcp` | Start an MCP server over stdio (JSON-RPC) |
| `shell` | Start a persistent JSON-line REPL for agents |
| `<tool_name> [--json '{...}']` | Call any Coda tool dynamically |

Run `shd discover` to see the full list of available tools and their schemas. Tools are fetched from Coda's MCP endpoint and cached locally.

## Global Flags

| Flag | Description |
|------|-------------|
| `--output json\|table\|ndjson` | Output format (default: `json`) |
| `--dry-run` | Preview request without executing |
| `--token <TOKEN>` | Override the stored token |
| `--pick <field>` | Extract a specific field from the response (dot-path, e.g. `name` or `items.0.id`) |
| `--fuzzy` | Resolve tool name via fuzzy matching against cached tools |
| `--sanitize` | Redact prompt injection patterns in API responses |
| `--trace` | Emit NDJSON execution traces to stderr |
| `--quiet` | Suppress informational stderr messages |

## Agent Design Principles

This CLI follows the [agent-first CLI design](https://justin.poehnelt.com/posts/rewrite-your-cli-for-ai-agents/) principles:

- **Fully dynamic** — no hardcoded tool commands. Tools are discovered at runtime from Coda's MCP endpoint and cached locally. New tools work without a CLI rebuild.
- **Raw JSON payloads** — every tool call accepts `--json` for the full API body, plus `--json -` to read from stdin
- **Schema introspection** — `shd discover <tool>` returns the tool's input schema. Cached locally after first fetch.
- **Fuzzy matching** — `--fuzzy` resolves typos and partial tool names against the cached tool list
- **Field extraction** — `--pick name` extracts a single field from the response, keeping agent context windows small
- **Input hardening** — rejects path traversal (`../`), control characters, query injection (`?`, `#`), percent-encoding bypasses (`%2e`)
- **Client-side validation** — payloads are validated against cached tool schemas before sending
- **Dry-run safety** — `--dry-run` on every tool call shows the exact HTTP request without sending it. Works without auth.
- **Structured errors** — all errors are JSON on stderr with exit code 1
- **Agent skill files** — 12 skill files in `skills/` encoding invariants agents can't intuit

## Architecture

```
src/
  main.rs            CLI definition (clap), command dispatch, dynamic tool routing
  client.rs          CodaClient: call_tool(), dry_run_tool(), fetch_tools() (SSE parsing)
  auth.rs            Token resolution: --token flag > CODA_API_TOKEN env > ~/.config/coda/credentials
  validate.rs        Input validation (resource IDs, JSON payloads, stdin support)
  sanitize.rs        Prompt injection detection/redaction (--sanitize flag)
  output.rs          Output formatting: json (pretty), table (comfy-table), ndjson
  error.rs           CodaError enum with ContractChanged variant for schema mismatches
  fuzzy.rs           Fuzzy tool name resolution against cached tools
  schema_cache.rs    Local tool schema cache + client-side payload validation
  trace.rs           NDJSON execution tracing to stderr
  commands/
    tools.rs         Core call() function for dynamic tool dispatch
    mcp.rs           MCP server over stdio (JSON-RPC), dynamically loads tools
    discover.rs      Lists/inspects tools fetched from the MCP endpoint
    shell.rs         Persistent JSON-line REPL for agents
    auth_cmd.rs      login, status, logout subcommands
```

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
DOC=$(shd document_create --json '{"title": "Q2 Planning"}' --pick id)

# Create a table with typed columns
RESULT=$(shd table_create --json "{
  \"docId\": \"$DOC\",
  \"canvasId\": \"canvas-$DOC\",
  \"name\": \"Tasks\",
  \"columns\": [
    {\"name\":\"Task\",\"isDisplayColumn\":true},
    {\"name\":\"Status\",\"format\":{\"type\":\"sl\",\"selectOptions\":[\"To Do\",\"In Progress\",\"Done\"]}},
    {\"name\":\"Priority\",\"format\":{\"type\":\"sl\",\"selectOptions\":[\"Low\",\"Medium\",\"High\"]}},
    {\"name\":\"Due\",\"format\":{\"type\":\"dp\"}}
  ]
}")
TABLE=$(echo "$RESULT" | jq -r '.tableId')

# Add rows
shd table_add_rows --json "{
  \"docId\": \"$DOC\",
  \"tableId\": \"$TABLE\",
  \"rows\": [
    {\"cells\": [{\"column\": \"Task\", \"value\": \"Design review\"}, {\"column\": \"Status\", \"value\": \"To Do\"}]}
  ]
}"
```

## Stats

| Metric | Value |
|--------|-------|
| Language | Rust |
| Cold startup | ~13ms |
| Rust LOC | ~2,500 |
| Built-in commands | 4 (`auth`, `discover`, `mcp`, `shell`) |
| Dynamic tools | All Coda MCP tools (discovered at runtime) |
| Agent skills | 12 |

## License

MIT
