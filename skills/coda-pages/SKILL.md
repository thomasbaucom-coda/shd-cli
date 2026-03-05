---
name: coda-pages
version: 0.1.0
description: "Coda Pages: create, list, get, update, delete pages and page content"
metadata:
  requires:
    bins: ["coda"]
---

# pages

> **PREREQUISITE:** Read `../coda-shared/SKILL.md` for auth, global flags, and safety rules.

```bash
coda pages <method> <docId> [args] [flags]
```

## Methods

| Method | Description |
|--------|-------------|
| `list` | List pages in a doc |
| `get` | Get page metadata |
| `create` | Create a new page |
| `update` | Update page properties |
| `delete` | Delete a page |
| `content` | Get page content (child objects like tables, controls) |

## Usage

### List pages

```bash
coda pages list <docId> --output table
```

### Get page details

```bash
coda pages get <docId> <pageId>
```

### Create a page

```bash
# Simple
coda pages create <docId> --name "Meeting Notes"

# Full payload
coda pages create <docId> --json '{"name": "Meeting Notes", "subtitle": "Weekly sync"}'
```

### Update a page

```bash
coda pages update <docId> <pageId> --json '{"name": "Renamed Page"}' --dry-run
```

### Get page content

```bash
coda pages content <docId> <pageId>
```

### Delete a page (DESTRUCTIVE)

```bash
coda pages delete <docId> <pageId> --dry-run
```

## Schema

```bash
coda schema pages.list
coda schema pages.create
```
