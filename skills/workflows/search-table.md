# Search a Table

`table_search` reads all rows from a table and filters them client-side by column value. Returns only matching rows.

> Prerequisite: Read [Getting Started](../fundamentals/getting-started.md)

## When to Use

- Finding specific rows by column value (status, owner, name, etc.)
- Filtering without reading the entire table response yourself
- Quick queries when you already know the table URI

Use `shd sync` + `grep` instead for searching across **multiple tables** or when you need repeated reads.

## Command

```bash
shd table_search --json '{
  "uri": "coda://docs/DOC_ID/tables/TABLE_ID",
  "column": "Column Name",
  "value": "search value"
}'
```

## Operators

| Operator | Meaning |
|----------|---------|
| `eq` (default) | Exact match (case-insensitive) |
| `ne` | Not equal |
| `contains` | Substring match (case-insensitive) |

## Examples

**Find all "Todo" items:**
```bash
shd table_search --json '{
  "uri": "coda://docs/AbCdEf/tables/grid-123",
  "column": "Status",
  "value": "Todo"
}' --pick rows,matchCount
```

**Search by owner with contains:**
```bash
shd table_search --json '{
  "uri": "coda://docs/AbCdEf/tables/grid-123",
  "column": "Owner",
  "value": "Tyler",
  "operator": "contains"
}'
```

**Find everything NOT done:**
```bash
shd table_search --json '{
  "uri": "coda://docs/AbCdEf/tables/grid-123",
  "column": "Status",
  "value": "Done",
  "operator": "ne"
}' --pick matchCount
```

## Response

```json
{
  "rows": [...],
  "matchCount": 3,
  "totalRows": 42,
  "filter": {"column": "Status", "operator": "eq", "value": "Todo"}
}
```

## How to Get the Table URI

1. **From synced data:** Read `.coda/docs/<slug>/CONTEXT.md` — lists table URIs
2. **From doc_summarize:** `shd doc_summarize --json '{"uri":"coda://docs/..."}' --pick tables`
3. **From a previous response:** Any tool that creates a table returns `tableUri`

## Gotchas

- **Column name must match exactly** (case-insensitive). Use `shd doc_summarize` to check column names.
- **Column IDs work too** (e.g., `"column": "c-AbCdEf"`), but names are easier.
- **All rows are fetched** then filtered client-side. For very large tables (5000+ rows), this may be slow.
- **Matching is case-insensitive** for all operators.

## See also

- [Summarize Doc](summarize-doc.md) — find table URIs and column names
- [Read and Write Rows](read-and-write-rows.md) — direct row operations
- [Sync and Read](../fundamentals/sync-and-read.md) — grep across synced tables for multi-table search
