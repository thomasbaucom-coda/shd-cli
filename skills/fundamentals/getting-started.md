# Getting Started with shd

The `shd` CLI is a dynamic interface to Coda. All commands dispatch to Coda's tool endpoint at runtime — there are no hardcoded resource commands.

## Authentication

Three methods, in priority order:

```bash
# 1. Flag (highest priority)
shd whoami --token "your-token"

# 2. Environment variable
export CODA_API_TOKEN="your-token"
shd whoami

# 3. Stored credential (interactive)
shd auth login    # Opens browser, stores token locally
shd auth status   # Check current auth
shd auth logout   # Remove stored token
```

## Calling Tools

Every tool follows the same pattern:

```bash
shd <tool_name> --json '{"key": "value"}'
```

No payload needed for read-only tools like `whoami`:
```bash
shd whoami
```

## Always Use --pick

This is the single most important flag for agents. API responses can be hundreds of tokens. `--pick` extracts only the fields you need:

```bash
# Without --pick: ~200 tokens of user profile
shd whoami

# With --pick: just the name, ~5 tokens
shd whoami --pick name

# Multi-pick: returns a JSON object
shd doc_scaffold --json @blueprint.json --pick docUri,browserLink
# → {"docUri": "coda://docs/abc", "browserLink": "https://coda.io/d/..."}

# Dot paths for nested data
shd doc_summarize --json '{"uri":"coda://docs/abc"}' --pick pages.0.title
```

**Rule of thumb:** Always use `--pick` unless you specifically need the full response.

## Payload Delivery

Three ways to pass JSON payloads:

```bash
# Inline — for small payloads
shd page_create --json '{"uri":"coda://docs/abc","title":"New Page"}'

# From file — for large payloads, avoids shell escaping
shd doc_scaffold --json @blueprint.json

# From stdin — for piping
cat data.json | shd table_add_rows --json -
```

`@file` is recommended for anything with nested JSON or special characters.

## Safety Rules

1. **Use `--dry-run` before mutations** to preview the request:
   ```bash
   shd page_create --json '{"uri":"coda://docs/abc","title":"Test"}' --dry-run
   ```

2. **Never fabricate resource IDs or URIs.** Always get them from tool responses or synced CONTEXT.md files.

3. **Confirm destructive actions** with the user before executing deletes.

4. **Use `shd discover <tool> --compact`** to check a tool's schema before calling it for the first time.

## Output Formats

```bash
shd whoami                        # Default: pretty JSON
shd whoami --output table         # Human-readable table
shd whoami --output ndjson        # One-line JSON (for piping)
```

## Error Patterns

Errors are structured JSON on stderr with exit code 1:

```json
{"error": true, "type": "validation_error", "message": "Missing required field: uri"}
```

| Type | Meaning | Action |
|------|---------|--------|
| `api_error` | Coda API returned an error | Check message, retry on 429 |
| `validation_error` | Bad payload | Run `shd discover <tool> --compact` for required fields |
| `contract_changed` | Tool renamed/removed | Run `shd discover --refresh` |
| `auth_required` | No token found | Run `shd auth login` |

## See also

- [Discovering Tools](discover-tools.md) — find and inspect available tools
- [Sync and Read](sync-and-read.md) — pull docs to local filesystem
- [Scaffold Doc](../workflows/scaffold-doc.md) — create complete docs
