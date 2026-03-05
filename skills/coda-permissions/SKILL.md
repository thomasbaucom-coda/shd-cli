---
name: coda-permissions
version: 0.1.0
description: "Coda Permissions: manage doc sharing, access control, and permissions"
metadata:
  requires:
    bins: ["coda"]
---

# permissions

> **PREREQUISITE:** Read `../coda-shared/SKILL.md` for auth, global flags, and safety rules.

```bash
coda permissions <method> <docId> [args] [flags]
```

## Methods

| Method | Description |
|--------|-------------|
| `list` | List permissions on a doc |
| `metadata` | Get sharing metadata (canShare, canCopy, etc.) |
| `add` | Add a permission to a doc |
| `remove` | Remove a permission |

## Usage

### List permissions

```bash
coda permissions list <docId>
```

### Check sharing metadata

```bash
coda permissions metadata <docId>
```

### Add a permission (dry-run first)

```bash
coda permissions add <docId> --json '{
  "access": "readonly",
  "principal": {"type": "email", "email": "user@example.com"}
}' --dry-run
```

### Remove a permission (confirm with user)

```bash
coda permissions remove <docId> <permissionId> --dry-run
```

## Schema

```bash
coda schema permissions.list
coda schema permissions.add
```
