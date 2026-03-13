# Error Recovery

## When Things Go Wrong

### Compound Operations Return Partial Results

`doc_scaffold` and `page_create_with_content` return `"complete": false` when non-critical steps fail:

```bash
shd doc_scaffold --json @blueprint.json
# Returns: {"docUri": "...", "complete": false, "errors": ["Content for 'Goals': HTTP 500"]}
```

**What to do:** The doc was created. Read `errors[]`, retry failed parts with individual tools:
```bash
shd content_modify --json '{"uri": "coda://docs/abc/pages/goals", "operations": [...]}'
```

### Sync Reports Status

After `shd sync`, check if sync was complete:
```bash
cat .coda/docs/my-doc/__sync.json | grep status
# "status": "complete" or "status": "partial"
```

If partial, re-sync: `shd sync --doc-url "..." --force`

### Pagination Truncation

Large results may be truncated. Check for `_pagination` in the response:
```bash
shd table_read_rows --json '{"uri": "..."}' --pick _pagination
```

### Retries

The CLI automatically retries on transient errors (429 rate limit, 5xx server errors, network issues). Non-retriable errors (400, 401, 403, 404) fail immediately — don't retry these.
