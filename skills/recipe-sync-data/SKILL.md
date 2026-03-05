---
name: recipe-sync-data
version: 0.1.0
description: "Recipe: Sync external data into a Coda table"
metadata:
  requires:
    bins: ["coda", "curl", "jq"]
---

# Recipe: Sync External Data into Coda

Pull data from an external API and upsert it into a Coda table.

## Example: Sync GitHub Issues

```bash
# 1. Fetch issues from GitHub as NDJSON
gh issue list --repo myorg/myrepo --limit 100 --json number,title,state,assignees,labels \
  | jq -c '.[] | {
      "Issue": ("#" + (.number | tostring) + " " + .title),
      "Status": .state,
      "Assignee": (.assignees | map(.login) | join(", ")),
      "Labels": (.labels | map(.name) | join(", "))
    }' \
  | coda rows import <docId> <tableId> --key-columns "Issue"

# 2. Or with curl + any REST API
curl -s "https://api.example.com/items" \
  | jq -c '.data[] | {Name: .name, Value: .amount, Updated: .updated_at}' \
  | coda rows import <docId> <tableId> --key-columns "Name"
```

## Example: CSV to Coda

```bash
# Convert CSV to NDJSON and import
cat data.csv \
  | python3 -c "
import csv, json, sys
reader = csv.DictReader(sys.stdin)
for row in reader:
    print(json.dumps(row))
" \
  | coda rows import <docId> <tableId> --key-columns "ID"
```

## Key Points

- `rows import` reads NDJSON from stdin — one JSON object per line
- Flat objects (`{"Name":"Alice"}`) auto-convert to Coda row format
- `--key-columns` enables upsert: existing rows with matching keys get updated
- Auto-batches into 500-row chunks (public API) or 100-row chunks (tool API)
- Works with any tool that produces JSON: `curl`, `jq`, `gh`, Python, etc.
