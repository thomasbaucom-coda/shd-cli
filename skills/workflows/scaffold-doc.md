# Scaffold a Complete Doc

`doc_scaffold` creates a complete Coda document with pages, markdown content, tables, and rows in a single CLI call. It replaces ~32 chained API calls with one.

> Prerequisite: Read [Getting Started](../fundamentals/getting-started.md)

## When to Use

- Creating a new doc with a known structure (pages, tables, data)
- Building from a template or blueprint
- Bootstrapping project trackers, meeting docs, team hubs

Use `page_create_with_content` instead if you're adding a page to an **existing** doc.

## Command

```bash
shd doc_scaffold --json '{"title":"Doc Title","pages":[...]}' [--sync] [--pick docUri]
```

## Blueprint Format

```json
{
  "title": "Sprint Planning",
  "pages": [
    {
      "title": "Goals",
      "subtitle": "Optional subtitle",
      "content": "# Markdown content\n\nSupports full markdown."
    },
    {
      "title": "Tasks",
      "content": "# Task Board",
      "tables": [{
        "name": "Sprint Items",
        "columns": [
          {"name": "Task"},
          {"name": "Status"},
          {"name": "Owner"},
          {"name": "Points"}
        ],
        "rows": [
          ["Design review", "Done", "Alice", "3"],
          ["Backend API", "In Progress", "Bob", "8"],
          ["Write tests", "To Do", "Carol", "5"]
        ]
      }]
    }
  ]
}
```

## Examples

**Simple doc with two pages:**
```bash
shd doc_scaffold --json '{
  "title": "Meeting Notes",
  "pages": [
    {"title": "Agenda", "content": "# Today'\''s Agenda\n\n1. Status updates\n2. Blockers\n3. Next steps"},
    {"title": "Action Items", "content": "# Action Items\n\nTo be filled during meeting."}
  ]
}' --pick docUri,browserLink
```

**From a file (recommended for complex blueprints):**
```bash
cat > blueprint.json << 'EOF'
{
  "title": "Project Tracker",
  "pages": [
    {"title": "Overview", "content": "# Project Overview\n\nKey metrics and status."},
    {"title": "Backlog", "tables": [{
      "name": "Tasks",
      "columns": [{"name":"Title"},{"name":"Status"},{"name":"Owner"},{"name":"Priority"}],
      "rows": [
        ["MVP feature", "In Progress", "Tyler", "High"],
        ["Bug fixes", "Todo", "Alex", "Medium"]
      ]
    }]}
  ]
}
EOF

shd doc_scaffold --json @blueprint.json --sync
```

**Create and immediately sync to local files:**
```bash
shd doc_scaffold --json @blueprint.json --sync --pick docUri
# Returns docUri immediately
# Background process syncs to .coda/ (~20s later)
```

## Coda Best Practices for Blueprints

When designing your blueprint:

- **One table per noun.** Don't create separate "Q1 Tasks" and "Q2 Tasks" tables — use one "Tasks" table. Coda's views can filter by quarter.
- **Keep docs focused.** One doc per project or team. Don't build mega-docs.
- **Use descriptive column names.** "Owner" not "Col3". Agents and humans both benefit.
- **Separate permanent from ephemeral.** A team wiki and a sprint tracker should be different docs.

## Response

```json
{
  "docUri": "coda://docs/AbCdEf",
  "browserLink": "https://coda.io/d/_dAbCdEf",
  "pages": [...],
  "tables": [...],
  "totalRows": 5,
  "errors": []
}
```

Check the `errors` array — partial failures are reported here (e.g., a table created but rows failed to insert).

## Gotchas

- Rows are arrays of values, ordered by column definition order — not objects with named keys
- Tables are created on the **page** they're defined under, not at the doc level
- Use `--pick docUri` to get just the URI you need for subsequent operations
- The `--sync` flag spawns a background process — files appear in `.coda/` after ~20 seconds

## See also

- [Create Then Sync](create-then-sync.md) — the --sync flag workflow
- [Create Page Content](create-page-content.md) — add pages to existing docs
- [Summarize Doc](summarize-doc.md) — understand an existing doc before scaffolding a similar one
