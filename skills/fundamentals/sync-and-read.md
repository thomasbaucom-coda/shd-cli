# Sync and Read

`shd sync` pulls a Coda document to the local filesystem so you can read its content with standard file tools (Read, Glob, Grep) instead of making API calls.

## When to Use Sync vs API

| Need | Use |
|------|-----|
| Read a doc's structure, pages, table data | `shd sync` then read files |
| Search across multiple tables | `shd sync` then grep `rows.ndjson` |
| Create or modify content | API tools (`doc_scaffold`, `content_modify`, etc.) |
| Quick one-off read | `shd doc_summarize` (no sync needed) |

Sync is for **reading at scale**. API tools are for **writing**.

## Syncing a Document

```bash
# From a browser URL (easiest)
shd sync --doc-url "https://coda.io/d/My-Doc_dAbCdEf"

# From a coda:// URI
shd sync --doc-uri "coda://docs/AbCdEf"
```

This creates files in `.coda/` (project-local by default):

```
.coda/
├── INDEX.md                                  # Lists ALL synced docs
├── .gitignore                                # Auto-created, excludes from git
└── docs/
    └── my-doc-abcdef/
        ├── CONTEXT.md                        # This doc's pages, tables, columns
        ├── __doc.json                        # Doc metadata (title, URI)
        └── pages/
            ├── overview-xyz123.md            # Page content as markdown
            ├── overview-xyz123.json          # Page metadata (URIs)
            └── tables/
                └── tasks-abc456/
                    ├── __schema.json         # Column definitions
                    └── rows.ndjson           # Flattened row data
```

## Reading Synced Data

**Step 1: Start with INDEX.md**
```
# Synced Coda Docs
## Docs
- docs/sprint-board-qx8zl4/ — "Sprint Board" (2 pages, 1 table)
- docs/meeting-notes-bqwazw/ — "Meeting Notes" (5 pages, 0 tables)
```

**Step 2: Read a doc's CONTEXT.md**
```
# Sprint Board
Source: coda://docs/Qx8zL47kb_

## Pages
- sprint-goals-ywsbvr.md — "Sprint Goals"
- backlog-pmf22d.md — "Backlog"

## Tables
- tables/sprint-items-gl3ckt/ — "Sprint Items" (4 columns, 5 rows)
  Columns: Title, Status, Assignee, Points
```

**Step 3: Read specific files**
- `.md` files are page content — just markdown, directly readable
- `rows.ndjson` has one flattened JSON row per line:
  ```
  {"_rowId":"i-abc","Title":"Ship sync","Status":"Done","Assignee":"Tyler","Points":8}
  ```
- `__schema.json` has column definitions with IDs, types, and formulas

**Step 4: Search across tables**
```bash
# Find all rows where Status is "Todo" across every synced table
grep '"Status":"Todo"' .coda/docs/*/pages/tables/*/rows.ndjson
```

## Syncing Multiple Docs

Each `shd sync` call adds to `.coda/`. Sync as many docs as you need:

```bash
shd sync --doc-url "https://coda.io/d/Doc-A_dAbc"
shd sync --doc-url "https://coda.io/d/Doc-B_dXyz"
# INDEX.md now lists both docs
```

## Useful Flags

| Flag | Effect |
|------|--------|
| `--force` | Re-sync everything (reserved for future use) |
| `--tables-only` | Skip page content, only sync table data |
| `--max-rows N` | Limit rows per table (default: 5000) |
| `--root <path>` | Change output directory (default: `.coda`) |
| `--dry-run` | Preview what would be synced without writing files |

## Important Notes

- **Synced data is a snapshot**, not live. Re-run `shd sync` to refresh.
- **Not every Coda doc is synced.** If the doc you need isn't in `.coda/`, sync it first.
- **`.coda/.gitignore` auto-excludes** synced data from git commits.
- **Row data is flattened**: column names as keys, not column IDs. Values are unwrapped from Coda's wrapper objects.

## See also

- [Getting Started](getting-started.md) — auth, --pick, safety rules
- [Create Then Sync](../workflows/create-then-sync.md) — the --sync flag for auto-syncing after creation
- [Scaffold Doc](../workflows/scaffold-doc.md) — create complete docs
