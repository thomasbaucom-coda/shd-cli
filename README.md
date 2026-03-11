# shd — Superhuman Docs CLI

Agent-first command-line interface for [Coda](https://coda.io). Built in Rust. All commands dispatch dynamically to Coda's MCP tool endpoint — new tools work without a CLI rebuild.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/thomasbaucom-coda/shd-cli/main/install.sh | bash
```

Requires the [GitHub CLI](https://cli.github.com/) (`gh`) authenticated with repo access. No Rust or npm needed — downloads a pre-built binary.

<details>
<summary>Alternative: install via npm</summary>

```bash
npm install -g @thomasbaucom-coda/shd
```

Requires [GitHub Packages auth](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-npm-registry#authenticating-to-github-packages). The binary command is `shd`.
</details>

<details>
<summary>Alternative: build from source</summary>

```bash
cargo build --release
# Binary at target/release/shd

cargo install --path .
# Adds to PATH
```
</details>

## Authenticate

```bash
shd auth login
```

Opens your browser to generate an MCP-scoped API token. Paste it when prompted.

Or set it directly:
```bash
export CODA_API_TOKEN="your-token"
```

## Quick Start

```bash
# Check your connection
shd whoami

# Discover available tools
shd discover

# Compact schema (agent-friendly, 5 lines instead of 300)
shd discover content_modify --compact

# Build a complete doc in one call
shd doc_scaffold --json @blueprint.json

# Create a page with content
shd page_create_with_content --json '{
  "uri": "coda://docs/DOC_ID",
  "title": "My Page",
  "content": "# Hello\n\nMarkdown content here."
}'

# Summarize an existing doc
shd doc_summarize --json '{"uri": "coda://docs/DOC_ID"}'

# Extract specific fields from responses
shd whoami --pick name
shd page_create --json '{...}' --pick canvasUri,pageUri
# Returns: {"canvasUri": "...", "pageUri": "..."}
```

## Compound Operations

These compose multiple API calls into single CLI invocations — fewer calls, no URI chaining, no sleeps.

| Tool | What it does | Calls saved |
|------|-------------|-------------|
| `doc_scaffold` | Build complete doc from JSON blueprint (pages, content, tables, rows) | ~32 → 1 |
| `page_create_with_content` | Create page + insert markdown | 2 → 1 |
| `doc_summarize` | Condensed doc overview (pages, content previews, tables, row counts) | ~10 → 1 |
| `table_search` | Filter table rows by column value (eq/ne/contains) | 1 → 1 (filtered) |

### doc_scaffold

```bash
cat > blueprint.json << 'EOF'
{
  "title": "Sprint Planning",
  "pages": [
    {"title": "Goals", "content": "# Sprint Goals\n\n- Ship feature X\n- Fix bug Y"},
    {"title": "Tasks", "content": "# Task Board", "tables": [{
      "name": "Tasks",
      "columns": [{"name": "Task"}, {"name": "Status"}, {"name": "Owner"}],
      "rows": [
        ["Design review", "Done", "Alice"],
        ["Backend API", "In Progress", "Bob"],
        ["Write tests", "To Do", "Carol"]
      ]
    }]}
  ]
}
EOF

shd doc_scaffold --json @blueprint.json
```

## Commands

4 built-in commands + 4 compound operations. Everything else is a dynamically dispatched tool call.

| Command | Description |
|---------|-------------|
| `auth login\|status\|logout` | Manage authentication |
| `discover [tool] [--compact] [--filter X]` | List tools or inspect schemas |
| `mcp` | Start MCP server over stdio (JSON-RPC) |
| `shell` | Persistent JSON-line REPL for agents |
| `doc_scaffold` | Build complete doc from blueprint |
| `page_create_with_content` | Create page with markdown content |
| `doc_summarize` | Condensed doc overview |
| `table_search` | Filter table rows by column value |
| `<tool_name> [--json '{...}']` | Call any Coda tool dynamically |

## Global Flags

| Flag | Description |
|------|-------------|
| `--pick <field>` | Extract field(s) from response. Multi: `--pick a,b` returns JSON object |
| `--json @file.json` | Read payload from file (no shell escaping) |
| `--json -` | Read payload from stdin |
| `--dry-run` | Preview request without executing |
| `--compact` | Compact schema view (with `discover`) |
| `--fuzzy` | Fuzzy-match tool names |
| `--trace` | Emit NDJSON execution traces to stderr |
| `--output json\|table\|ndjson` | Output format (default: `json`) |
| `--sanitize` | Redact prompt injection patterns in responses |
| `--quiet` | Suppress informational messages |
| `--token <TOKEN>` | Override stored token |

## Agent Design

Built for AI agents following [agent-first CLI design](https://justin.poehnelt.com/posts/rewrite-your-cli-for-ai-agents/) principles:

- **Fully dynamic** — tools discovered at runtime from Coda's MCP endpoint, cached locally
- **`--pick` reduces tokens** — extract only needed fields, multi-pick returns JSON objects
- **`--json @file`** — eliminates shell escaping issues for large payloads
- **`discover --compact`** — 5-line schema view instead of 300-line JSON dump
- **Compound operations** — `doc_scaffold` replaces ~32 chained calls with 1
- **Error hints** — pick errors show available fields; error-as-success responses detected
- **Client-side validation** — payloads validated against cached schemas before sending
- **Structured errors** — JSON on stderr, exit code 1, agent-friendly error types
- **Dry-run** — `--dry-run` previews requests without auth
- **9 skill files** in `skills/` (3 fundamentals + 6 workflows) teach agents CLI patterns, Coda best practices, and safety rules
- **CONTEXT.md** at repo root provides agent-facing orientation (separate from developer CLAUDE.md)

## Architecture

```
src/
  main.rs              CLI definition (clap), dispatch, dynamic tool routing
  client.rs            CodaClient: call_tool(), auto-pagination, error parsing
  auth.rs              Token resolution: --token > CODA_API_TOKEN > credential file
  validate.rs          Input validation, --json @file support
  output.rs            JSON/table/ndjson formatting, --pick output
  error.rs             CodaError enum with agent-friendly variants
  fuzzy.rs             Fuzzy tool name resolution
  schema_cache.rs      Local schema cache + client-side validation
  sanitize.rs          Prompt injection detection/redaction
  trace.rs             NDJSON execution tracing
  commands/
    compound.rs        Compound operations (scaffold, summarize, search)
    tools.rs           Core tool dispatch and --pick
    mcp.rs             MCP server over stdio (JSON-RPC)
    discover.rs        Tool listing with --compact
    shell.rs           Persistent REPL for agents
    auth_cmd.rs        login, status, logout
```

## License

MIT
