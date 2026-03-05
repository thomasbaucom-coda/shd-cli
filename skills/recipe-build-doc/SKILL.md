---
name: recipe-build-doc
version: 0.1.0
description: "Recipe: Build a complete Coda doc from scratch — pages, tables, content, and data"
metadata:
  requires:
    bins: ["coda"]
    env: ["CODA_API_TOKEN (MCP-scoped for full capabilities)"]
---

# Recipe: Build a Complete Doc

Creates a Coda doc with structured pages, typed tables, and populated data — entirely from the CLI.

## Full Workflow

```bash
# 1. Create the doc
DOC=$(coda docs create --title "Project Tracker" | jq -r '.id')
sleep 3

# 2. Get the default page
OVERVIEW=$(coda pages list "$DOC" | jq -r '.items[0].id')

# 3. Create additional pages
coda pages create "$DOC" --name "Tasks" --json '{"subtitle":"All project tasks"}'
coda pages create "$DOC" --name "Team" --json '{"subtitle":"Team members and roles"}'
coda pages create "$DOC" --name "Notes" --json '{"subtitle":"Meeting notes and decisions"}'

# 4. Get the Tasks page canvas ID
TASKS_PAGE=$(coda pages list "$DOC" | jq -r '.items[] | select(.name=="Tasks") | .id')

# 5. Write content to the Overview page
coda tool content-modify "$DOC" "$OVERVIEW" --operations '[
  {"operation":"insert_element","blockType":"markdown","content":"# Project Tracker\n\nTrack tasks, team members, and project notes in one place.\n\n## Quick Links\n- **Tasks** — All project tasks with status and priority\n- **Team** — Team members and roles\n- **Notes** — Meeting notes and decisions"}
]'

# 6. Create a tasks table on the Tasks page
RESULT=$(coda tool table-create "$DOC" "$TASKS_PAGE" \
  --name "Tasks" \
  --columns '[
    {"name":"Task","isDisplayColumn":true},
    {"name":"Status","format":{"type":"sl","selectOptions":["To Do","In Progress","Done","Blocked"]}},
    {"name":"Priority","format":{"type":"sl","selectOptions":["Low","Medium","High","Critical"]}},
    {"name":"Assignee"},
    {"name":"Due Date","format":{"type":"dp"}},
    {"name":"Notes"}
  ]')
TABLE=$(echo "$RESULT" | jq -r '.tableId')
echo "Created table: $TABLE"

# 7. Get column IDs from the response
COLS=$(echo "$RESULT" | jq -r '[.columns[].columnId] | @json')
echo "Columns: $COLS"

# 8. Bulk import tasks
echo '[
  ["Set up repo", "Done", "High", "Alice", "2025-01-15", ""],
  ["Design database schema", "Done", "High", "Bob", "2025-01-20", "Postgres"],
  ["Build API endpoints", "In Progress", "High", "Alice", "2025-02-01", "REST + GraphQL"],
  ["Write tests", "In Progress", "Medium", "Charlie", "2025-02-10", ""],
  ["Deploy to staging", "To Do", "Medium", "Bob", "2025-02-15", ""],
  ["Load testing", "To Do", "Low", "Charlie", "2025-02-20", ""],
  ["Production deploy", "To Do", "Critical", "Alice", "2025-03-01", "Needs sign-off"]
]' | coda tool import-rows "$DOC" "$TABLE" --columns "$COLS"

# 9. Add a filtered view
coda tool view-add "$DOC" "$TABLE" \
  --name "Active Tasks" \
  --layout card \
  --filter 'Status != "Done"'
```

## Notes

- Steps 1-4 use the **public API** (standard token works)
- Steps 5-9 use the **internal tool API** (requires MCP-scoped token)
- `coda tool import-rows` reads from stdin and auto-batches (100 rows per API call)
- Column IDs from `table-create` response must be passed to `table-add-rows` / `import-rows`
