---
name: coda-tool-tables
version: 0.1.0
description: "Coda Internal Tools: create tables, add columns, manage views (requires MCP-scoped token)"
metadata:
  requires:
    bins: ["coda"]
    env: ["CODA_API_TOKEN (MCP-scoped)"]
---

# tool table-* and view-*

> **PREREQUISITE:** Read `../coda-shared/SKILL.md` for auth and safety rules.
> **REQUIRES:** MCP-scoped API token. Generate at coda.io/account with MCP scope.

## Commands

| Command | Description |
|---------|-------------|
| `tool table-create` | Create a table with typed columns on a page |
| `tool table-add-rows` | Add rows to an existing table (bulk, typed) |
| `tool table-add-columns` | Add columns to an existing table |
| `tool table-delete-rows` | Delete rows from a table |
| `tool table-update-rows` | Update rows in a table |
| `tool import-rows` | Import rows from stdin, auto-batched (max 100/batch) |
| `tool view-configure` | Configure a view (rename, filter, change layout) |

## Workflow: Create a Table and Populate It

```bash
# 1. Find the page to put the table on
coda pages list <docId> --output table

# 2. Create the table with typed columns
coda tool table-create <docId> <canvasId> \
  --name "Deals" \
  --columns '[
    {"name":"Company","isDisplayColumn":true},
    {"name":"Value","format":{"type":"curr","code":"USD","precision":0}},
    {"name":"Stage","format":{"type":"sl","selectOptions":["Prospecting","Negotiation","Closed Won"]}}
  ]'

# 3. Note the tableId and column IDs from the response

# 4. Add rows (values in column order)
coda tool table-add-rows <docId> <tableId> \
  --columns '["c-abc","c-def","c-ghi"]' \
  --rows '[["Acme",50000,"Prospecting"],["TechCo",120000,"Negotiation"]]'

# 5. Or bulk import from stdin
python3 generate_rows.py | coda tool import-rows <docId> <tableId> \
  --columns '["c-abc","c-def","c-ghi"]'
```

## Workflow: Configure a Filtered View

Every table has a default view. Configure it with a filter:

```bash
# Filter the default view to show only active deals
coda tool view-configure <docId> <tableId> \
  --name "Active Deals" \
  --filter 'Stage != "Closed Won"'

# Change layout to card view
coda tool view-configure <docId> <tableId> \
  --layout card

# Clear a filter
coda tool view-configure <docId> <tableId> \
  --filter none
```

Note: Use `$'...'` quoting in zsh for formulas containing `!=`.

## Column Format Types

| Type | Example |
|------|---------|
| `none` | Plain text |
| `num` | Number (set `precision`) |
| `curr` | Currency (set `code`: USD, EUR, etc.) |
| `per` | Percentage |
| `check` | Checkbox |
| `sl` | Select list (set `selectOptions`) |
| `dp` | Date picker |
| `dt` | DateTime picker |
| `email` | Email |
| `link` | URL/link |
| `person` | Person reference |
| `scale` | Star/icon rating |
| `slider` | Slider range |
| `lookup` | Reference to another table |
