# Coda CLI (`coda`) Context

The `coda` CLI provides programmatic access to Coda docs, pages, tables, rows, formulas, controls, folders, and permissions. It also runs as an MCP server for direct AI agent integration.

## Rules of Engagement for Agents

* **Schema Discovery:** If you don't know the exact JSON payload structure, run `coda schema <resource>.<method>` first to inspect parameters and schemas. This is a local operation — no network call.
* **Context Window Protection:** Coda tables can return many columns per row. ALWAYS use `--fields "Col1,Col2"` when listing rows to limit output to only the columns you need.
* **Dry-Run Safety:** ALWAYS use `--dry-run` for mutating operations (create, update, delete, upsert) to validate your request before actual execution.
* **ID Discovery:** Get resource IDs from list commands before operating on them. Do not guess or fabricate IDs. Use `coda resolve-url` to extract IDs from Coda URLs.
* **Confirm Destructive Actions:** Always confirm with the user before running delete commands.
* **Use --page-all for bulk reads:** When you need all rows from a table, use `--page-all` which auto-paginates and streams NDJSON.

## Core Syntax

```bash
coda <resource> <method> [args] [flags]
```

Use `--help` on any command for usage:

```bash
coda --help
coda <resource> --help
coda <resource> <method> --help
```

### Global Flags

| Flag | Description |
|------|-------------|
| `--token <TOKEN>` | API token (overrides CODA_API_TOKEN env var) |
| `--output <FORMAT>` | Output format: `json` (default), `table`, `ndjson` |
| `--dry-run` | Preview the HTTP request without sending it |

### Key Patterns

**1. Reading data — always limit output:**
```bash
coda docs list --limit 10
coda rows list <docId> <tableId> --fields "Name,Status" --limit 20
coda schema rows.list
```

**2. Writing data — always dry-run first:**
```bash
coda docs create --json '{"title": "My Doc"}' --dry-run
coda rows upsert <docId> <tableId> --json '{"rows": [...]}' --dry-run
```

**3. Schema introspection (no network call):**
```bash
coda schema list                  # List all resources
coda schema rows                  # List methods for rows
coda schema rows.list             # Full parameter and schema info
coda schema docs.create           # Request body schema
```

**4. Working with rows:**
```bash
# List with field filtering
coda rows list <docId> <tableId> --fields "Name,Email,Status" --limit 50

# Stream ALL rows (auto-paginated NDJSON)
coda rows list <docId> <tableId> --page-all

# Upsert rows (insert or update by key column)
coda rows upsert <docId> <tableId> --json '{
  "rows": [
    {"cells": [{"column": "Name", "value": "Alice"}, {"column": "Status", "value": "Active"}]}
  ],
  "keyColumns": ["Name"]
}'

# Push a button
coda rows push-button <docId> <tableId> <rowId> <columnId>
```

**5. URL resolution:**
```bash
# Decode a Coda URL to get docId, pageId, tableId, etc.
coda resolve-url "https://coda.io/d/_dAbCdEf/Page_suXYZ"
```

**6. Permissions:**
```bash
coda permissions list <docId>
coda permissions metadata <docId>
coda permissions add <docId> --json '{"access": "readonly", "principal": {"type": "email", "email": "user@example.com"}}' --dry-run
```

**7. MCP server mode:**
```bash
# Start as an MCP server over stdio (for Claude Desktop, VS Code, etc.)
coda mcp
```

## Authentication

Set `CODA_API_TOKEN` environment variable (preferred for agents) or run `coda auth login`.

## Error Handling

All errors are returned as structured JSON on stderr:
```json
{"error": true, "message": "API error (404): Doc not found"}
```

Exit code 1 indicates an error. Exit code 0 indicates success.
