---
name: recipe-export-table
version: 0.1.0
description: "Recipe: Export a Coda table to NDJSON for processing"
metadata:
  requires:
    bins: ["coda", "jq"]
---

# Recipe: Export a Table to NDJSON

Stream all rows from a Coda table as newline-delimited JSON for piping into other tools.

## Steps

```bash
# 1. Find the doc
coda docs list --query "My Project" --limit 5 --output table

# 2. Find the table
coda tables list <docId> --output table

# 3. Check columns (to know what fields to request)
coda columns list <docId> <tableId> --output table

# 4. Export all rows as NDJSON (auto-paginated)
coda rows list <docId> <tableId> --page-all > export.ndjson

# 5. Filter to specific fields with jq
coda rows list <docId> <tableId> --fields "Name,Status,Due Date" --page-all \
  | jq '{name: .values.Name, status: .values.Status, due: .values["Due Date"]}'

# 6. Convert to CSV
coda rows list <docId> <tableId> --fields "Name,Status" --page-all \
  | jq -r '[.values.Name, .values.Status] | @csv'
```

## Notes

- `--page-all` automatically fetches all pages and outputs NDJSON
- Always use `--fields` to limit columns and reduce output size
- Each line in the output is a complete JSON object (one per row)
- Pipe to `jq`, `mlr`, or any NDJSON-aware tool for further processing
