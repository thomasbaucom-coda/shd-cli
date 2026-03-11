# Summarize a Doc

`doc_summarize` reads a document and returns a condensed overview of its structure: pages, content previews, tables, columns, and row counts. ~500 tokens instead of 10+ API calls.

> Prerequisite: Read [Getting Started](../fundamentals/getting-started.md)

## When to Use

- Understanding an existing doc's structure before modifying it
- Discovering table URIs and column names
- Quick inspection without a full `shd sync`

Use `shd sync` instead if you need to read full page content or table row data.

## Command

```bash
shd doc_summarize --json '{"uri": "coda://docs/DOC_ID"}'
```

## Example

```bash
shd doc_summarize --json '{"uri": "coda://docs/AbCdEf"}' --pick pages,tables
```

Returns:
```json
{
  "pages": [
    {"title": "Overview", "canvasUri": "coda://docs/.../canvases/...", "contentPreview": "# Project Overview\n\nThis doc tracks...", "tables": 0},
    {"title": "Backlog", "canvasUri": "coda://docs/.../canvases/...", "tables": 1}
  ],
  "tables": [
    {"name": "Tasks", "tableUri": "coda://docs/.../tables/grid-...", "columns": ["Title","Status","Owner"], "rowCount": 42, "page": "Backlog"}
  ]
}
```

## Common Patterns

**Find a table URI to search or modify:**
```bash
shd doc_summarize --json '{"uri":"coda://docs/AbCdEf"}' --pick tables
# → Find the tableUri for the table you need
shd table_search --json '{"uri":"<tableUri>","column":"Status","value":"Todo"}'
```

**Understand a doc before scaffolding a similar one:**
```bash
# Summarize the source doc
shd doc_summarize --json '{"uri":"coda://docs/SOURCE_ID"}' --pick pages,tables
# Read the structure, then create a similar blueprint
shd doc_scaffold --json @new-blueprint.json --sync
```

## Gotchas

- **Max 20 pages** are read. Larger docs will have truncated results.
- **Content preview is truncated** to 200 characters per page.
- **Row data is NOT included** — only column names and row counts. Use `table_search` or `shd sync` for actual row data.
- **Use `--pick`** to reduce output. The full response includes doc metadata you rarely need.

## See also

- [Search Table](search-table.md) — find specific rows after getting a table URI
- [Sync and Read](../fundamentals/sync-and-read.md) — full content sync for deeper reading
- [Scaffold Doc](scaffold-doc.md) — create a doc based on what you learned
