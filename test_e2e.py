#!/usr/bin/env python3
"""End-to-end test for the Superhuman Docs CLI.

Requires:
- Public API token in CODA_API_TOKEN or stored via `coda auth login`
- MCP-scoped token passed as first argument (for tool commands)

Usage: python3 test_e2e.py <mcp_token> [public_token]
"""

import json
import subprocess
import sys
import time
import os

CLI = "./target/release/coda"
MCP_TOKEN = sys.argv[1] if len(sys.argv) > 1 else None
PUBLIC_TOKEN = sys.argv[2] if len(sys.argv) > 2 else None

if not MCP_TOKEN:
    print("Usage: python3 test_e2e.py <mcp_token> [public_token]")
    sys.exit(1)

passed = 0
failed = 0
errors = []

def test(name, cmd, stdin_data=None, expect_error=False, check=None, token=None):
    """Run a test and report pass/fail."""
    global passed, failed
    args = [CLI]
    if token:
        args += ["--token", token]
    args += cmd
    result = subprocess.run(args, capture_output=True, text=True, input=stdin_data)

    ok = True
    detail = ""

    if expect_error:
        if result.returncode == 0:
            ok = False
            detail = "Expected error but got success"
    else:
        if result.returncode != 0:
            ok = False
            detail = result.stderr.strip()[:120]

    if ok and check and result.returncode == 0:
        try:
            data = json.loads(result.stdout)
            check_result = check(data)
            if check_result is not True:
                ok = False
                detail = str(check_result)
        except json.JSONDecodeError:
            if check:
                ok = False
                detail = f"Could not parse JSON: {result.stdout[:80]}"

    status = "PASS" if ok else "FAIL"
    print(f"  [{status}] {name}")
    if ok:
        passed += 1
    else:
        failed += 1
        errors.append((name, detail))
        if detail:
            print(f"         {detail}")

    return result.stdout if result.returncode == 0 else None


print()
print("=" * 60)
print("  SUPERHUMAN DOCS CLI — End-to-End Test")
print("=" * 60)

# ============================================================
# Phase 1: No-auth commands
# ============================================================
print("\n--- Phase 1: No-auth commands ---")

test("--version", ["--version"])
result = subprocess.run([CLI, "--help"], capture_output=True, text=True)
all_output = result.stdout + result.stderr
if "Superhuman Docs" in all_output and result.returncode == 0:
    passed += 1
    print(f"  [PASS] --help shows banner")
else:
    failed += 1
    print(f"  [FAIL] --help shows banner")
    errors.append(("--help shows banner", "SUPERHUMAN not in output"))
test("schema list", ["schema", "list"],
     check=lambda d: "resources" in d)
test("schema rows.list", ["schema", "rows.list"],
     check=lambda d: d.get("operationId") == "listRows")
test("schema docs.create", ["schema", "docs.create"],
     check=lambda d: "requestBody" in d)
test("dry-run without auth", ["docs", "create", "--title", "test", "--dry-run"],
     check=lambda d: d.get("method") == "POST")
test("input validation: path traversal", ["rows", "list", "../../etc", "table1", "--dry-run"],
     expect_error=True)
test("input validation: query injection", ["rows", "list", "abc?x=y", "table1", "--dry-run"],
     expect_error=True)
test("input validation: control chars", ["rows", "list", "abc\tdef", "table1", "--dry-run"],
     expect_error=True)

# ============================================================
# Phase 2: Public API (needs public token or stored creds)
# ============================================================
print("\n--- Phase 2: Public API ---")

token = PUBLIC_TOKEN or MCP_TOKEN  # try MCP token for public API too

out = test("whoami", ["whoami"], token=token,
     check=lambda d: "name" in d)

out = test("docs list", ["docs", "list", "--limit", "2"], token=token,
     check=lambda d: "items" in d)

if out:
    docs = json.loads(out)
    if docs.get("items"):
        doc_id = docs["items"][0]["id"]
        doc_name = docs["items"][0]["name"]
        print(f"         Using doc: {doc_name} ({doc_id})")

        test("docs get", ["docs", "get", doc_id], token=token,
             check=lambda d: d.get("id") == doc_id)

        out2 = test("tables list", ["tables", "list", doc_id], token=token,
             check=lambda d: "items" in d)

        test("pages list", ["pages", "list", doc_id], token=token,
             check=lambda d: "items" in d)

        test("folders list", ["folders", "list"], token=token,
             check=lambda d: "items" in d)

        if out2:
            tables = json.loads(out2)
            if tables.get("items"):
                table_id = tables["items"][0]["id"]
                print(f"         Using table: {tables['items'][0]['name']} ({table_id})")

                test("columns list", ["columns", "list", doc_id, table_id], token=token,
                     check=lambda d: "items" in d)

                test("rows list", ["rows", "list", doc_id, table_id, "--limit", "3"], token=token,
                     check=lambda d: "items" in d)

                test("rows list --fields", ["rows", "list", doc_id, table_id,
                     "--fields", "Task,Status", "--limit", "2"], token=token,
                     check=lambda d: all(
                         set(item.get("values", {}).keys()).issubset({"Task", "Status"})
                         for item in d.get("items", [])
                     ) if d.get("items") else True)

                test("rows list --output table", ["rows", "list", doc_id, table_id,
                     "--limit", "2", "--output", "table"], token=token)

                test("rows list --output ndjson", ["rows", "list", doc_id, table_id,
                     "--limit", "2", "--output", "ndjson"], token=token)
    else:
        print("         (no docs found, skipping doc-specific tests)")

test("resolve-url", ["resolve-url", "https://coda.io/d/_duwRjkvTAr3"], token=token,
     check=lambda d: d.get("type") == "apiLink")

# ============================================================
# Phase 3: Tool endpoint (needs MCP token)
# ============================================================
print("\n--- Phase 3: Tool endpoint (internal API) ---")

# Create a test doc
print("  Creating test doc...")
out = test("tool: document_create", ["tool", "raw", "placeholder", "document_create",
     "--json", '{"title":"E2E Test Doc"}'], token=MCP_TOKEN,
     check=lambda d: "docId" in d)

if not out:
    print("  SKIPPING tool tests — document_create failed")
else:
    doc = json.loads(out)
    DOC = doc["docId"]
    CANVAS = doc["pages"][0]["canvasId"]
    PAGE = doc["pages"][0]["pageId"]
    print(f"         Doc: {DOC}, Canvas: {CANVAS}")
    time.sleep(3)

    # Page operations
    test("tool: page_update", ["tool", "raw", DOC, "page_update",
         "--json", json.dumps({"docId": DOC, "pageId": PAGE,
                               "updateFields": {"title": "Overview", "subtitle": "E2E Test"}})],
         token=MCP_TOKEN, check=lambda d: True)

    out = test("tool: page_create", ["tool", "raw", DOC, "page_create",
         "--json", json.dumps({"docId": DOC, "parentPageId": PAGE,
                               "title": "Data", "subtitle": "Tables and data"})],
         token=MCP_TOKEN, check=lambda d: "canvasId" in d)

    data_canvas = json.loads(out)["canvasId"] if out else None

    # Content writing
    test("tool: content_modify (markdown)", ["tool", "content-modify", DOC, CANVAS,
         "--operations", json.dumps([{
             "operation": "insert_element",
             "blockType": "markdown",
             "content": "# E2E Test Doc\n\nThis doc was created by the **Superhuman Docs CLI** end-to-end test."
         }])], token=MCP_TOKEN, check=lambda d: d.get("success") == True)

    test("tool: content_modify (callout)", ["tool", "content-modify", DOC, CANVAS,
         "--operations", json.dumps([{
             "operation": "insert_element",
             "blockType": "callout",
             "content": "All tests passed!",
             "quickStyle": "success",
             "insertPosition": "page_start",
         }])], token=MCP_TOKEN, check=lambda d: d.get("success") == True)

    # Table creation
    if data_canvas:
        out = test("tool: table-create", ["tool", "table-create", DOC, data_canvas,
             "--name", "Test Table",
             "--columns", json.dumps([
                 {"name": "Name", "isDisplayColumn": True},
                 {"name": "Score", "format": {"type": "num", "precision": 0}},
                 {"name": "Status", "format": {"type": "sl", "selectOptions": ["Active", "Done"]}},
                 {"name": "Amount", "format": {"type": "curr", "code": "USD", "precision": 2}},
             ]),
             "--rows", json.dumps([
                 ["Seed Row", 100, "Active", 99.99],
             ])], token=MCP_TOKEN,
             check=lambda d: "tableId" in d and d.get("rowCount") == 1)

        if out:
            table_data = json.loads(out)
            TABLE = table_data["tableId"]
            COL_IDS = [c["columnId"] for c in table_data["columns"]]
            print(f"         Table: {TABLE}, Columns: {len(COL_IDS)}")

            # Bulk row add
            rows = [[f"Person {i}", i * 10, "Active" if i % 2 == 0 else "Done", i * 100.50]
                    for i in range(1, 51)]
            test("tool: table-add-rows (50 rows)", ["tool", "table-add-rows", DOC, TABLE,
                 "--columns", json.dumps(COL_IDS),
                 "--rows", json.dumps(rows)], token=MCP_TOKEN,
                 check=lambda d: d.get("rowCount") == 51)

            # Import from stdin
            more_rows = [[f"Imported {i}", i, "Active", i * 5.0] for i in range(51, 101)]
            test("tool: import-rows (50 from stdin)", ["tool", "import-rows", DOC, TABLE,
                 "--columns", json.dumps(COL_IDS)],
                 stdin_data=json.dumps(more_rows), token=MCP_TOKEN,
                 check=lambda d: d.get("totalRows") == 50)

            # View configure
            test("tool: view-configure", ["tool", "view-configure", DOC, TABLE,
                 "--name", "Active Only",
                 "--filter", 'Status = "Active"'], token=MCP_TOKEN,
                 check=lambda d: "name" in d)

            # Add columns
            test("tool: table-add-columns", ["tool", "table-add-columns", DOC, TABLE,
                 "--columns", json.dumps([
                     {"name": "Notes"},
                     {"name": "Priority", "format": {"type": "sl", "selectOptions": ["Low", "Medium", "High"]}},
                 ])], token=MCP_TOKEN)

    # Formula
    test("tool: formula-execute", ["tool", "formula-execute", DOC,
         "--formula", "Now()"], token=MCP_TOKEN,
         check=lambda d: "value" in d)

    # Dynamic dispatch — tools we never wrapped
    test("dynamic: url_decode", ["tool", "url_decode", DOC,
         "--json", json.dumps({"url": f"https://coda.io/d/_d{DOC}"})], token=MCP_TOKEN,
         check=lambda d: d.get("docId") == DOC)

    test("dynamic: search", ["tool", "search", DOC,
         "--json", json.dumps({"docId": DOC, "query": "Test"})], token=MCP_TOKEN,
         check=lambda d: "results" in d)

    test("dynamic: whoami", ["tool", "whoami", DOC,
         "--json", "{}"], token=MCP_TOKEN,
         check=lambda d: "name" in d)

    # tool list (discovery)
    test("tool list (discovery)", ["tool", "list", DOC], token=MCP_TOKEN,
         check=lambda d: "content" in d)

    # MCP server
    print("\n--- Phase 4: MCP server ---")

    mcp_input = '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}\n{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}\n'
    result = subprocess.run([CLI, "--token", MCP_TOKEN, "mcp"],
                           capture_output=True, text=True, input=mcp_input, timeout=10)
    lines = [l for l in result.stdout.strip().split('\n') if l]
    if len(lines) >= 2:
        init_resp = json.loads(lines[0])
        tools_resp = json.loads(lines[1])
        init_ok = init_resp.get("result", {}).get("protocolVersion") == "2024-11-05"
        tools_count = len(tools_resp.get("result", {}).get("tools", []))
        if init_ok:
            passed += 1
            print(f"  [PASS] MCP initialize")
        else:
            failed += 1
            print(f"  [FAIL] MCP initialize")
            errors.append(("MCP initialize", str(init_resp)))
        if tools_count > 20:
            passed += 1
            print(f"  [PASS] MCP tools/list ({tools_count} tools)")
        else:
            failed += 1
            print(f"  [FAIL] MCP tools/list (only {tools_count} tools)")
            errors.append(("MCP tools/list", f"{tools_count} tools"))
    else:
        failed += 2
        print(f"  [FAIL] MCP server (no output)")
        errors.append(("MCP server", "no output"))

    # Cleanup
    print("\n--- Cleanup ---")
    test("tool: document_delete", ["tool", "raw", DOC, "document_delete",
         "--json", json.dumps({"docId": DOC})], token=MCP_TOKEN)

# ============================================================
# Results
# ============================================================
print()
print("=" * 60)
total = passed + failed
print(f"  Results: {passed}/{total} passed, {failed} failed")
if errors:
    print()
    print("  Failures:")
    for name, detail in errors:
        print(f"    - {name}: {detail}")
print("=" * 60)
print()

sys.exit(0 if failed == 0 else 1)
