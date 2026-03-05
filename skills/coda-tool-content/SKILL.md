---
name: coda-tool-content
version: 0.1.0
description: "Coda Internal Tools: write page content, manage comments, create formulas (requires MCP-scoped token)"
metadata:
  requires:
    bins: ["coda"]
    env: ["CODA_API_TOKEN (MCP-scoped)"]
---

# tool content-modify, comment-manage, formula-*

> **PREREQUISITE:** Read `../coda-shared/SKILL.md` for auth and safety rules.
> **REQUIRES:** MCP-scoped API token.

## Commands

| Command | Description |
|---------|-------------|
| `tool content-modify` | Add text, headings, lists, code blocks, images to a page |
| `tool comment-manage` | Add, reply to, or delete comments |
| `tool formula-create` | Create a named formula on a page |
| `tool formula-execute` | Evaluate a CFL expression |

## Writing Page Content

```bash
# Add markdown content to a page
coda tool content-modify <docId> <canvasId> --operations '[
  {"operation":"insert_element","blockType":"markdown","content":"# Welcome\n\nThis is **bold** and this is a list:\n- Item 1\n- Item 2"}
]'

# Add a code block
coda tool content-modify <docId> <canvasId> --operations '[
  {"operation":"insert_element","blockType":"codeblock","content":"const x = 42;","language":"javascript"}
]'

# Add a divider
coda tool content-modify <docId> <canvasId> --operations '[
  {"operation":"insert_element","blockType":"divider"}
]'
```

## Managing Comments

```bash
# Add a comment to a page
coda tool comment-manage <docId> --json '{
  "data": {"action":"add_to_page","pageId":"<pageId>","content":"Great work on this!"}
}'

# Add a comment to a specific table row
coda tool comment-manage <docId> --json '{
  "data": {"action":"add_to_row","tableId":"<tableId>","rowId":"<rowId>","content":"Needs review"}
}'

# Reply to a comment thread
coda tool comment-manage <docId> --json '{
  "data": {"action":"add_reply","threadId":"<threadId>","content":"Thanks, fixed!"}
}'

# Delete a comment thread
coda tool comment-manage <docId> --json '{
  "data": {"action":"delete_thread","threadId":"<threadId>"}
}'
```

## Formulas

```bash
# Create a named formula on a page
coda tool formula-create <docId> <canvasId> \
  --name "Total Revenue" \
  --formula 'Sum(Deals.filter(Stage="Closed Won").[Deal Value])'

# Execute an expression (returns the computed result)
coda tool formula-execute <docId> --formula 'Now()'
coda tool formula-execute <docId> --formula 'Deals.Count()'
```

## Content Operation Types

| Operation | Description |
|-----------|-------------|
| `insert_element` | Insert a new block (markdown, codeblock, divider, image, callout) |
| `replace_text` | Replace text content in the page |
| `replace_element_text` | Replace text within a specific element |
| `delete_element` | Delete an element by ID |
| `delete_element_by_text` | Delete an element by matching text |

## Block Types for insert_element

| Block Type | Fields |
|------------|--------|
| `markdown` | `content` (markdown string) |
| `codeblock` | `content`, `language` (optional) |
| `divider` | `lineStyle` (optional) |
| `image` | `url`, `altText` (optional) |
| `callout` | `content`, `quickStyle` (info/warning/success/error), `icon`, `color` (optional) |
