---
name: recipe-create-tracker
version: 0.1.0
description: "Recipe: Create a project tracker doc with a tasks table"
metadata:
  requires:
    bins: ["coda"]
---

# Recipe: Create a Project Tracker

Creates a new Coda doc with a project tracking table pre-populated with example tasks.

## Steps

```bash
# 1. Create the doc
DOC_ID=$(coda docs create --title "Project Tracker" | jq -r '.id')

# 2. Wait for doc to be ready
sleep 3

# 3. Get the default page
PAGE_ID=$(coda pages list "$DOC_ID" | jq -r '.items[0].id')

# 4. List tables to see if one was auto-created, or note that
#    table creation requires the Coda UI (the public API doesn't support it yet)

# 5. View the doc
coda docs get "$DOC_ID"
```

## Notes

- The Coda public API does not currently support creating tables directly.
  Tables must be created in the UI or via a template doc.
- For a pre-built tracker, use `--json '{"sourceDoc": "<templateDocId>"}'`
  to copy from an existing template doc that already has the table structure.
