# Create a Page with Content

`page_create_with_content` creates a new page and inserts markdown in a single call. It replaces the two-step `page_create` → `content_modify` chain.

> Prerequisite: Read [Getting Started](../fundamentals/getting-started.md)

## When to Use

- Adding a page to an **existing** doc
- Creating nested subpages under an existing page
- Inserting markdown content at page creation time

Use `doc_scaffold` instead if you're creating a **new doc from scratch**.

## Command

```bash
shd page_create_with_content --json '{
  "uri": "coda://docs/DOC_ID",
  "title": "Page Title",
  "content": "# Markdown content here"
}'
```

## The URI Determines Placement

| URI type | Result |
|----------|--------|
| `coda://docs/{docId}` | Top-level page in the doc |
| `coda://docs/{docId}/pages/{pageId}` | Nested subpage under the given page |

Get the doc URI from `doc_scaffold` response, synced `__doc.json`, or `doc_summarize`.

## Examples

**Add a top-level page:**
```bash
shd page_create_with_content --json '{
  "uri": "coda://docs/AbCdEf",
  "title": "Weekly Status",
  "content": "# Week of March 10\n\n## Highlights\n- Shipped sync command\n- Fixed auth flow\n\n## Blockers\nNone this week."
}' --pick canvasUri,pageUri
```

**Add a nested subpage:**
```bash
shd page_create_with_content --json '{
  "uri": "coda://docs/AbCdEf/pages/section-XyZ",
  "title": "March 10 Notes",
  "subtitle": "Sprint 47",
  "content": "# Meeting Notes\n\nAttendees: Tyler, Sarah, Alex"
}'
```

**Content only (no markdown):**
```bash
shd page_create_with_content --json '{
  "uri": "coda://docs/AbCdEf",
  "title": "Empty Page"
}'
```

## Response

```json
{
  "canvasUri": "coda://docs/AbCdEf/canvases/canvas-123",
  "pageUri": "coda://docs/AbCdEf/pages/section-456",
  "contentWritten": true
}
```

Use `canvasUri` for subsequent content operations (`content_modify`). Use `pageUri` to create subpages under this page.

## Gotchas

- Content is **markdown**, not HTML. Coda renders it.
- Use `canvasUri` (not `pageUri`) when calling `content_modify` to add more content later.
- The `subtitle` field is optional.
- You need the doc URI first — get it from `shd doc_summarize`, synced CONTEXT.md, or a previous scaffold response.

## See also

- [Scaffold Doc](scaffold-doc.md) — create a complete doc from scratch
- [Summarize Doc](summarize-doc.md) — find URIs for existing docs and pages
- [Discover Tools](../fundamentals/discover-tools.md) — inspect `content_modify` schema for advanced content operations
