# Discovering Tools

`shd discover` lists all available tools and their schemas. Use it to understand what's available before calling tools.

## List All Tools

```bash
shd discover
```

Returns every tool with its name and description. This includes both API tools from Coda and compound operations.

## Filter by Keyword

```bash
shd discover --filter table
shd discover --filter page
shd discover --filter content
```

Case-insensitive substring match on tool name and description.

## Inspect a Specific Tool

```bash
# Compact view — agent-friendly, ~5 lines
shd discover content_modify --compact

# Full view — complete JSON schema with all field descriptions
shd discover content_modify
```

**Always use `--compact` first.** It shows required fields and their types, which is usually enough. Only use the full view if you need field descriptions or nested object schemas.

Example compact output:
```
content_modify
  Insert, replace, or delete content blocks in a page.
  Required: uri (string), operations (array)
  Optional: (none)
```

## Compound Tools

Four synthetic tools appear alongside API tools in discovery:

| Tool | What it does |
|------|-------------|
| `doc_scaffold` | Create complete doc with pages, tables, rows in one call |
| `page_create_with_content` | Create page + insert markdown (2 calls → 1) |
| `doc_summarize` | Condensed doc overview: pages, content previews, tables |
| `table_search` | Read table rows and filter by column value |

These are orchestrated locally — they call multiple API tools under the hood.

## Cache Behavior

Tool schemas are cached locally for 24 hours. To force a fresh fetch:

```bash
shd discover --refresh
```

Use `--refresh` when:
- You get a `contract_changed` error
- You suspect a new tool was added
- It's been more than a day since you last discovered

## Common Gotchas

- **Tool names use underscores**, not hyphens: `table_read_rows` not `table-read-rows`
- **Use `--fuzzy` if unsure of the exact name**: `shd create_table --fuzzy` resolves to `table_create`
- **Compound tools have different schemas** than their underlying API tools — always discover the compound tool directly

## See also

- [Getting Started](getting-started.md) — auth, --pick, safety rules
- [Sync and Read](sync-and-read.md) — pull docs to local filesystem
