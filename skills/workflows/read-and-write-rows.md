# Read and Write Rows

Direct table operations using dynamic tool dispatch. For filtered reads, see [Search Table](search-table.md) instead.

> Prerequisite: Read [Getting Started](../fundamentals/getting-started.md)

## Discover the Schema First

Always check the tool schema before calling:

```bash
shd discover table_read_rows --compact
shd discover table_add_rows --compact
```

## Reading Rows

```bash
shd table_read_rows --json '{"uri": "coda://docs/DOC_ID/tables/TABLE_ID"}' --pick rows
```

For large tables, limit rows:
```bash
shd table_read_rows --json '{"uri": "...", "rowLimit": 10}' --pick rows
```

**Row values use column IDs**, not names:
```json
{
  "rows": [
    {"rowId": "i-abc", "values": {"c-1": {"content": "Alice"}, "c-2": 42}}
  ]
}
```

To map column IDs to names, use `doc_summarize` or read the synced `__schema.json`.

## Adding Rows

```bash
shd table_add_rows --json '{
  "uri": "coda://docs/DOC_ID/tables/TABLE_ID",
  "columns": ["c-ColId1", "c-ColId2"],
  "rows": [
    ["Value A", "Value B"],
    ["Value C", "Value D"]
  ]
}'
```

**You need column IDs**, not names. Get them from:
- Synced `__schema.json` (`columnId` field)
- `doc_summarize` response (tables[].columns)
- The `table_create` response

## Updating Rows

Check the schema first:
```bash
shd discover table_update_rows --compact
```

Then call with the appropriate payload structure.

## The Column ID Problem

Coda's API uses column IDs (`c-AbCdEf`) internally, but human-readable column names in the UI. When writing rows, you need the IDs.

**Quick way to get column IDs:**
```bash
# Option 1: From synced schema
cat .coda/docs/<slug>/pages/tables/<slug>/__schema.json

# Option 2: From doc_summarize
shd doc_summarize --json '{"uri":"coda://docs/..."}' --pick tables

# Option 3: Discover via page_read
shd page_read --json '{"uri":"<canvasUri>","contentTypesToInclude":["tables"]}' --pick tables
```

## Bulk Import from Stdin

For large datasets, pipe JSON through stdin:
```bash
cat rows.json | shd table_add_rows --json -
```

## Gotchas

- **Column IDs are required for writes** — you can't use column names with `table_add_rows`
- **Row values in reads are wrapped** in `{content: "..."}` objects — use synced `rows.ndjson` for flattened data
- **Auto-pagination** fetches up to 5000 rows automatically (50 pages of 100)
- **Use `--pick rows`** to avoid pulling metadata you don't need

## See also

- [Search Table](search-table.md) — filtered reads by column value
- [Scaffold Doc](scaffold-doc.md) — create tables with rows in one call
- [Discover Tools](../fundamentals/discover-tools.md) — check any tool's schema
