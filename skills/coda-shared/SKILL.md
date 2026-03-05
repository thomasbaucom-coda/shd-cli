---
name: coda-shared
version: 0.1.0
description: "Coda CLI: shared rules, authentication, and global flags"
metadata:
  requires:
    bins: ["coda"]
    env: ["CODA_API_TOKEN"]
---

# Coda CLI — Shared Rules

> **Read this before using any other coda skill.**

## Authentication

The CLI requires a Coda API token. Set it via:

```bash
export CODA_API_TOKEN="your-token-here"
```

Or authenticate interactively:

```bash
coda auth login
coda auth status
```

## Global Flags

Every command supports:

| Flag | Description |
|------|-------------|
| `--output json` | Machine-readable JSON (default) |
| `--output table` | Human-readable table |
| `--output ndjson` | Newline-delimited JSON for streaming |
| `--dry-run` | Show the HTTP request without executing |
| `--token <T>` | Override the API token for this call |

## Safety Rules

1. **Always `--dry-run` before mutations.** Create, update, delete, and upsert operations should be previewed first.
2. **Always use `--fields` on row list calls.** Coda tables can have dozens of columns — limit to what you need.
3. **Never fabricate IDs.** Always discover IDs via list commands or `coda schema`.
4. **Use `coda schema` before writing JSON payloads.** It shows exact parameter names, types, and required fields.
5. **Confirm destructive actions with the user.** Deletes cannot be undone.

## Error Format

Errors are JSON on stderr with exit code 1:

```json
{"error": true, "message": "..."}
```

## Resource ID Discovery

```bash
# Find a doc
coda docs list --query "Project Tracker" --limit 5

# Find tables in a doc
coda tables list <docId>

# Find columns in a table
coda columns list <docId> <tableId>

# Inspect a row
coda rows get <docId> <tableId> <rowId>
```
