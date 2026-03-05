---
name: coda-docs
version: 0.1.0
description: "Coda Docs: create, list, get, and delete documents"
metadata:
  requires:
    bins: ["coda"]
---

# docs

> **PREREQUISITE:** Read `../coda-shared/SKILL.md` for auth, global flags, and safety rules.

```bash
coda docs <method> [flags]
```

## Methods

| Method | Description |
|--------|-------------|
| `list` | List accessible docs |
| `get` | Get a doc by ID |
| `create` | Create a new doc |
| `delete` | Permanently delete a doc |

## Usage

### List docs

```bash
coda docs list --limit 10
coda docs list --query "Budget" --output table
```

### Get a doc

```bash
coda docs get <docId>
```

### Create a doc

```bash
# Convenience flag
coda docs create --title "Q4 Planning"

# Full API payload
coda docs create --json '{"title": "Q4 Planning", "folderId": "fl-abc123"}'

# Always dry-run first
coda docs create --title "Test" --dry-run
```

### Delete a doc

```bash
# Preview first
coda docs delete <docId> --dry-run

# Execute (DESTRUCTIVE — confirm with user)
coda docs delete <docId>
```

## Schema

```bash
coda schema docs.list
coda schema docs.create
```
