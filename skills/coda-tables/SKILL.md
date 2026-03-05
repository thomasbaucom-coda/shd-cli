---
name: coda-tables
version: 0.1.0
description: "Coda Tables: list tables, inspect columns, manage views"
metadata:
  requires:
    bins: ["coda"]
---

# tables & columns

> **PREREQUISITE:** Read `../coda-shared/SKILL.md` for auth, global flags, and safety rules.

```bash
coda tables <method> <docId> [flags]
coda columns <method> <docId> <tableId> [flags]
```

## Table Methods

| Method | Description |
|--------|-------------|
| `list` | List tables in a doc |
| `get` | Get table metadata by ID |

## Column Methods

| Method | Description |
|--------|-------------|
| `list` | List columns in a table |
| `get` | Get column metadata by ID |

## Usage

### Discover tables

```bash
coda tables list <docId> --output table
coda tables get <docId> <tableId>
```

### Discover columns (required before writing rows)

```bash
coda columns list <docId> <tableId>
coda columns get <docId> <tableId> <columnId>
```

## Workflow: Table Discovery

Before reading or writing rows, discover the table structure:

```bash
# 1. Find the table
coda tables list <docId> --output table

# 2. Get column names and IDs
coda columns list <docId> <tableId>

# 3. Now read rows with specific fields
coda rows list <docId> <tableId> --fields "Name,Status,Due Date" --limit 20
```

## Schema

```bash
coda schema tables.list
coda schema columns.list
```
