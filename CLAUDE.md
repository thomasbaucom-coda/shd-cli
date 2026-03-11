# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo test                     # Run all unit tests
cargo test validate            # Run tests in a specific module (by name filter)
cargo clippy                   # Lint
cargo fmt -- --check           # Check formatting
cargo fmt                      # Auto-format
```

The binary is named `shd` (defined in Cargo.toml `[[bin]]`).

## Architecture

This is a Rust CLI (`coda`) for interacting with the Coda.io API. It follows a **tool-only, fully dynamic architecture** — all commands (except `auth`, `discover`, `import`, `mcp`) are dispatched dynamically to Coda's MCP tool endpoint at runtime. New tools work without a CLI rebuild.

### Key Design Decisions

- **No public API surface**: The CLI was refactored to eliminate per-resource command modules (docs, rows, tables, etc.). All API interaction goes through a single tool endpoint (`POST /apis/mcp/vbeta/tool` or `/apis/mcp/vbeta/docs/{docId}/tool`).
- **External subcommand dispatch**: Unknown subcommands are caught by clap's `external_subcommand` and routed to `dispatch_tool()` in `main.rs`, which calls `commands::tools::call()`.
- **Tool discovery**: `coda discover` fetches the tool list from Coda's MCP endpoint via SSE (Server-Sent Events) and parses `tools/list` results. No hardcoded tool registry.

### Source Layout

- `src/main.rs` — CLI definition (clap), command dispatch, dynamic tool routing via `dispatch_tool()`
- `src/client.rs` — `CodaClient` with `call_tool()`, `dry_run_tool()`, `fetch_tools()` (SSE parsing), and error classification
- `src/auth.rs` — Token resolution chain: `--token` flag > `CODA_API_TOKEN` env > `~/.config/coda/credentials` file
- `src/validate.rs` — Input validation (resource IDs reject control chars, `?`, `#`, `%`, `..`) and JSON payload parsing (supports `--json -` for stdin)
- `src/sanitize.rs` — Prompt injection detection/redaction in API responses (enabled via `--sanitize` flag)
- `src/output.rs` — Output formatting: json (pretty), table (comfy-table), ndjson
- `src/error.rs` — `CodaError` enum with `ContractChanged` variant for tool schema mismatches
- `src/commands/tools.rs` — Core `call()` function and `import_rows()` (stdin batch import)
- `src/commands/mcp.rs` — MCP server over stdio (JSON-RPC), dynamically loads tools from Coda on startup
- `src/commands/discover.rs` — Lists/inspects tools fetched from the MCP endpoint
- `src/commands/auth_cmd.rs` — `login`, `status`, `logout` subcommands
- `src/commands/sync.rs` — `sync` command: materialize docs to `.coda/` filesystem
- `src/cell.rs` — Cell value unwrapping and row flattening for sync
- `src/slug.rs` — Slugification, Coda browser URL parsing

### Agent-Facing Documentation

- `CONTEXT.md` — Agent usage guide (how to use the CLI). Separate from this file.
- `skills/fundamentals/` — Getting started, discovering tools, sync and reading
- `skills/workflows/` — Scaffolding, page creation, summarizing, searching, row ops, create-then-sync

When adding/removing CLI flags or compound operations, update the corresponding skill files and CONTEXT.md.

### Error Handling Pattern

All errors are `CodaError` (thiserror). Main catches errors and prints structured JSON to stderr with exit code 1. The `ContractChanged` variant specifically handles when a tool is renamed/removed or its schema changes — it directs the user to run `coda discover`.

### Input Validation

`validate_resource_id()` is used for all user-supplied IDs (docId, tableId, etc.) before they're interpolated into URL paths. It rejects path traversal, query injection, percent-encoding bypasses, and control characters.
