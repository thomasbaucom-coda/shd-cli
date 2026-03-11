# Create Then Sync

The `--sync` flag on any tool call triggers a background sync when the response contains a `docUri`. This lets you create a doc and immediately have it available as local files.

> Prerequisite: Read [Sync and Read](../fundamentals/sync-and-read.md)

## When to Use

- After creating a new doc that you (or an agent) will read from later
- When you want CONTEXT.md available immediately for follow-up operations
- Bootstrapping a new project where agents need filesystem access

## How It Works

```bash
shd doc_scaffold --json @blueprint.json --sync
```

1. The tool call executes normally (creates the doc)
2. The JSON result prints to stdout immediately
3. A background child process spawns to sync the doc
4. Files appear in `.coda/` after ~20 seconds (Coda needs time to provision the doc)

You don't wait for the sync — the CLI returns immediately.

## Examples

**Scaffold and sync:**
```bash
shd doc_scaffold --json '{
  "title": "Q3 Planning",
  "pages": [{"title": "Goals", "content": "# Q3 Goals"}]
}' --sync --pick docUri

# Output appears immediately:
# coda://docs/NewDocId
#
# [sync] Syncing in background (pid 12345). Files will appear in .coda/ shortly.
```

**Any creation tool works with --sync:**
```bash
# document_create
shd document_create --json '{"title": "New Doc"}' --sync

# doc_scaffold (most common)
shd doc_scaffold --json @blueprint.json --sync
```

## Verifying Sync Completed

The background process writes files silently. Check for completion:

```bash
# Check if CONTEXT.md exists
cat .coda/docs/*/CONTEXT.md 2>/dev/null

# Or check INDEX.md
cat .coda/INDEX.md
```

If the doc isn't there yet, either:
- Wait a few more seconds (Coda provision time)
- Run `shd sync --doc-uri "coda://docs/..."` manually

## Workflow: Create Based on Existing

```bash
# 1. Sync an existing doc to understand its structure
shd sync --doc-url "https://coda.io/d/Source-Doc_dAbCdEf"

# 2. Read the structure
cat .coda/docs/source-doc-abcdef/CONTEXT.md

# 3. Create a new doc based on what you learned
shd doc_scaffold --json @new-blueprint.json --sync

# 4. Both docs are now in .coda/ for future reference
cat .coda/INDEX.md
```

## Workflow: Fresh Start

```bash
# No existing docs needed — just create and sync
shd doc_scaffold --json @blueprint.json --sync

# Agent can start reading .coda/ once sync completes
# CONTEXT.md will list all pages and tables
```

## Gotchas

- **Sync takes ~20 seconds** after doc creation because Coda needs time to provision the doc. The CLI's retry logic handles this automatically.
- **The `--sync` flag only works on tool calls** (external subcommands), not on `shd sync` itself.
- **If the response has no `docUri`**, you'll see: `[sync] No docUri found in response — nothing to sync.`
- **Background process inherits your auth.** If your token is in a credential file, it works. If it's only in your shell environment, the background process also picks it up.

## See also

- [Scaffold Doc](scaffold-doc.md) — create complete docs
- [Sync and Read](../fundamentals/sync-and-read.md) — manual sync and reading files
- [Getting Started](../fundamentals/getting-started.md) — auth and global flags
