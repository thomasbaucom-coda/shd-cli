---
name: coda-rows
version: 0.1.0
description: "Coda Rows: list, get, upsert, update, delete, push buttons"
metadata:
  requires:
    bins: ["coda"]
---

# rows

> **PREREQUISITE:** Read `../coda-shared/SKILL.md` for auth, global flags, and safety rules.

```bash
coda rows <method> <docId> <tableId> [args] [flags]
```

## Methods

| Method | Description |
|--------|-------------|
| `list` | List rows (with filtering and field selection) |
| `get` | Get a single row by ID |
| `upsert` | Insert or update rows (batch, with key columns) |
| `update` | Update a single row |
| `delete` | Delete a single row |
| `delete-rows` | Delete multiple rows at once |
| `push-button` | Push a button column on a row |

## Critical Rules

1. **Always use `--fields`** when listing rows to protect context window
2. **Always `--dry-run` before upsert/update/delete**
3. **Discover columns first** with `coda columns list` before writing

## Usage

### List rows (READ — safe)

```bash
# Minimal: just the fields you need
coda rows list <docId> <tableId> --fields "Name,Status" --limit 20

# With search
coda rows list <docId> <tableId> --query '"Status":"Active"' --fields "Name,Email"

# Sort by update time
coda rows list <docId> <tableId> --sort-by updatedAt --limit 10
```

### Get a single row

```bash
coda rows get <docId> <tableId> <rowId>
```

### Upsert rows (WRITE — dry-run first)

```bash
# Dry-run
coda rows upsert <docId> <tableId> --json '{
  "rows": [
    {"cells": [
      {"column": "Name", "value": "Alice"},
      {"column": "Status", "value": "Active"}
    ]}
  ],
  "keyColumns": ["Name"]
}' --dry-run

# Execute
coda rows upsert <docId> <tableId> --json '{...}'
```

### Update a single row

```bash
coda rows update <docId> <tableId> <rowId> --json '{
  "row": {
    "cells": [
      {"column": "Status", "value": "Done"}
    ]
  }
}' --dry-run
```

### Delete rows (DESTRUCTIVE — confirm with user)

```bash
# Single row
coda rows delete <docId> <tableId> <rowId> --dry-run

# Multiple rows
coda rows delete-rows <docId> <tableId> --json '{
  "rowIds": ["i-row-abc", "i-row-def"]
}' --dry-run
```

### Push a button

```bash
coda rows push-button <docId> <tableId> <rowId> <columnId>
```

## Schema

```bash
coda schema rows.list       # Parameters for listing
coda schema rows.upsert     # Request body for upsert
coda schema rows.update     # Request body for update
```
