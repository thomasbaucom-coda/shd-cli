#!/usr/bin/env python3
"""Automated evaluation harness: SHD CLI vs Coda MCP (direct).

Runs identical tasks through both interfaces and captures:
  - Token proxy metrics (request/response byte sizes)
  - Latency (wall-clock ms per call)
  - Tool call count per task
  - Output quality (correctness checks)

Usage:
    python3 eval/eval_harness.py [--doc-id DOC_ID]

If --doc-id is omitted, creates a throwaway doc automatically.
Requires CODA_API_TOKEN in env or stored via `shd auth login`.
"""

import argparse
import json
import os
import subprocess
import sys
import time
import uuid
from dataclasses import dataclass, field, asdict
from typing import Any, Optional

RUN_ID = uuid.uuid4().hex[:6]

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

CLI = os.environ.get("SHD_CLI", "./target/release/shd")
DOC_ID: Optional[str] = None
DOC_URI: Optional[str] = None
CANVAS_URI: Optional[str] = None

# ---------------------------------------------------------------------------
# Metrics collection
# ---------------------------------------------------------------------------

@dataclass
class CallMetric:
    task: str
    path: str  # "cli" or "mcp"
    tool: str
    request_bytes: int = 0
    response_bytes: int = 0
    duration_ms: int = 0
    success: bool = True
    error: str = ""

@dataclass
class TaskResult:
    task: str
    path: str
    calls: list[CallMetric] = field(default_factory=list)
    total_request_bytes: int = 0
    total_response_bytes: int = 0
    total_duration_ms: int = 0
    call_count: int = 0
    quality_pass: bool = True
    quality_notes: str = ""

    def finalize(self):
        self.call_count = len(self.calls)
        self.total_request_bytes = sum(c.request_bytes for c in self.calls)
        self.total_response_bytes = sum(c.response_bytes for c in self.calls)
        self.total_duration_ms = sum(c.duration_ms for c in self.calls)


results: list[TaskResult] = []

# ---------------------------------------------------------------------------
# CLI helpers
# ---------------------------------------------------------------------------

def cli_call(tool: str, payload: Optional[dict] = None, pick: Optional[str] = None,
             trace: bool = True) -> tuple:
    """Call shd <tool> --json '...' and return (parsed_result, raw_stdout, req_bytes, resp_bytes, ms)."""
    args = [CLI, tool]
    payload_str = ""
    if payload is not None:
        payload_str = json.dumps(payload, separators=(',', ':'))
        args += ["--json", payload_str]
    if pick:
        args += ["--pick", pick]
    if trace:
        args += ["--trace"]

    req_bytes = len(tool.encode()) + len(payload_str.encode())
    start = time.monotonic()
    proc = subprocess.run(args, capture_output=True, text=True)
    elapsed_ms = int((time.monotonic() - start) * 1000)

    stdout = proc.stdout.strip()
    stderr = proc.stderr.strip()
    resp_bytes = len(stdout.encode())

    if proc.returncode != 0:
        return None, stderr, req_bytes, resp_bytes, elapsed_ms

    try:
        parsed = json.loads(stdout)
        # Coda sometimes returns {"error": "..."} with exit code 0
        if isinstance(parsed, dict) and "error" in parsed and len(parsed) == 1:
            return None, str(parsed["error"]), req_bytes, resp_bytes, elapsed_ms
        return parsed, stdout, req_bytes, resp_bytes, elapsed_ms
    except json.JSONDecodeError:
        return stdout, stdout, req_bytes, resp_bytes, elapsed_ms


def mcp_call(tool: str, arguments: Optional[dict] = None) -> tuple:
    """Call a tool via the MCP server (JSON-RPC over stdio) and return same tuple."""
    # Build JSON-RPC request
    rpc_request = json.dumps({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {"name": tool, "arguments": arguments or {}}
    })
    # We need to initialize first, then call
    init_msg = json.dumps({
        "jsonrpc": "2.0", "id": 0,
        "method": "initialize",
        "params": {"protocolVersion": "2024-11-05",
                    "clientInfo": {"name": "eval-harness", "version": "0.1"}}
    })
    initialized_msg = json.dumps({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    })

    stdin_data = f"{init_msg}\n{initialized_msg}\n{rpc_request}\n"
    req_bytes = len(rpc_request.encode())

    start = time.monotonic()
    proc = subprocess.run(
        [CLI, "mcp"], capture_output=True, text=True, input=stdin_data,
        timeout=30
    )
    elapsed_ms = int((time.monotonic() - start) * 1000)

    # Parse the response lines — last non-empty line should be the tool result
    resp_lines = [l for l in proc.stdout.strip().split("\n") if l.strip()]
    resp_bytes = sum(len(l.encode()) for l in resp_lines)

    # Find the response with id=1
    for line in resp_lines:
        try:
            msg = json.loads(line)
            if msg.get("id") == 1:
                if "error" in msg:
                    return None, json.dumps(msg["error"]), req_bytes, resp_bytes, elapsed_ms
                result = msg.get("result", {})
                # MCP wraps in content[0].text
                content = result.get("content", [])
                if content and content[0].get("text"):
                    try:
                        parsed = json.loads(content[0]["text"])
                        raw = content[0]["text"]
                        return parsed, raw, req_bytes, resp_bytes, elapsed_ms
                    except json.JSONDecodeError:
                        return content[0]["text"], content[0]["text"], req_bytes, resp_bytes, elapsed_ms
                return result, json.dumps(result), req_bytes, resp_bytes, elapsed_ms
        except json.JSONDecodeError:
            continue

    return None, proc.stderr.strip()[:200], req_bytes, resp_bytes, elapsed_ms


# ---------------------------------------------------------------------------
# Evaluation tasks
# ---------------------------------------------------------------------------

def task_discovery_cli(tr: TaskResult):
    """CLI path: discover tools and find row-related ones."""
    # Step 1: list all tools
    data, raw, rb, rpb, ms = cli_call("discover", pick=None, trace=True)
    tr.calls.append(CallMetric("discovery", "cli", "discover",
                               rb, rpb, ms, data is not None))

    # Step 2: filter for row tools (single call with --filter)
    args = [CLI, "discover", "--filter", "row"]
    start = time.monotonic()
    proc = subprocess.run(args, capture_output=True, text=True)
    elapsed = int((time.monotonic() - start) * 1000)
    tr.calls.append(CallMetric("discovery", "cli", "discover --filter row",
                               len("row".encode()), len(proc.stdout.encode()),
                               elapsed, proc.returncode == 0))

    tr.quality_pass = proc.returncode == 0 and "table_read_rows" in proc.stdout
    tr.quality_notes = "Found row-related tools" if tr.quality_pass else "Missing expected tools"


def task_discovery_mcp(tr: TaskResult):
    """MCP path: discover tools and find row-related ones."""
    # Step 1: tools/list
    rpc = json.dumps({
        "jsonrpc": "2.0", "id": 1,
        "method": "tools/list", "params": {}
    })
    init = json.dumps({
        "jsonrpc": "2.0", "id": 0,
        "method": "initialize",
        "params": {"protocolVersion": "2024-11-05",
                    "clientInfo": {"name": "eval", "version": "0.1"}}
    })
    notif = json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"})

    start = time.monotonic()
    proc = subprocess.run([CLI, "mcp"], capture_output=True, text=True,
                          input=f"{init}\n{notif}\n{rpc}\n", timeout=30)
    elapsed = int((time.monotonic() - start) * 1000)
    resp_bytes = len(proc.stdout.encode())

    tr.calls.append(CallMetric("discovery", "mcp", "tools/list",
                               len(rpc.encode()), resp_bytes, elapsed,
                               proc.returncode == 0))

    # Step 2: agent must parse full tool list to find row tools (no filter)
    # This is where token cost diverges — MCP returns ALL tool schemas
    found_row_tools = "table_read_rows" in proc.stdout
    tr.quality_pass = found_row_tools
    tr.quality_notes = "Found row tools in full list" if found_row_tools else "Could not locate row tools"


def task_read_cli(tr: TaskResult):
    """CLI path: read doc structure, then list rows from a table."""
    # Step 1: read doc
    data, raw, rb, rpb, ms = cli_call("document_read", {"uri": DOC_URI})
    tr.calls.append(CallMetric("read", "cli", "document_read",
                               rb, rpb, ms, data is not None))
    tr.quality_pass = data is not None and "pages" in (data if isinstance(data, dict) else {})
    tr.quality_notes = "Doc structure retrieved" if tr.quality_pass else "Failed to read doc"


def task_read_mcp(tr: TaskResult):
    """MCP path: read doc structure."""
    data, raw, rb, rpb, ms = mcp_call("document_read", {"uri": DOC_URI})
    tr.calls.append(CallMetric("read", "mcp", "document_read",
                               rb, rpb, ms, data is not None))
    tr.quality_pass = data is not None and "pages" in (data if isinstance(data, dict) else {})
    tr.quality_notes = "Doc structure retrieved" if tr.quality_pass else "Failed to read doc"


def task_write_cli(tr: TaskResult):
    """CLI path: create a page with content."""
    # Step 1: create page
    data, raw, rb, rpb, ms = cli_call("page_create", {
        "uri": DOC_URI, "title": f"CLI Eval Page {RUN_ID}"
    })
    tr.calls.append(CallMetric("write", "cli", "page_create",
                               rb, rpb, ms, data is not None))

    if data and isinstance(data, dict):
        page_uri = data.get("pageUri") or data.get("canvasUri")
        if page_uri:
            time.sleep(1)
            # Step 2: add content
            data2, raw2, rb2, rpb2, ms2 = cli_call("content_modify", {
                "uri": page_uri,
                "operations": [{"operation": "insert_element",
                                "blockType": "markdown",
                                "content": "# Hello from CLI eval\nThis page was created by the evaluation harness."}]
            })
            tr.calls.append(CallMetric("write", "cli", "content_modify",
                                       rb2, rpb2, ms2, data2 is not None))
            tr.quality_pass = data2 is not None
            tr.quality_notes = "Page created with content" if tr.quality_pass else "Content insertion failed"
            return

    tr.quality_pass = False
    tr.quality_notes = "Page creation failed"


def task_write_mcp(tr: TaskResult):
    """MCP path: create a page with content."""
    data, raw, rb, rpb, ms = mcp_call("page_create", {
        "uri": DOC_URI, "title": f"MCP Eval Page {RUN_ID}"
    })
    tr.calls.append(CallMetric("write", "mcp", "page_create",
                               rb, rpb, ms, data is not None))

    if data and isinstance(data, dict):
        page_uri = data.get("pageUri") or data.get("canvasUri")
        if page_uri:
            time.sleep(1)
            data2, raw2, rb2, rpb2, ms2 = mcp_call("content_modify", {
                "uri": page_uri,
                "operations": [{"operation": "insert_element",
                                "blockType": "markdown",
                                "content": "# Hello from MCP eval\nThis page was created by the evaluation harness."}]
            })
            tr.calls.append(CallMetric("write", "mcp", "content_modify",
                                       rb2, rpb2, ms2, data2 is not None))
            tr.quality_pass = data2 is not None
            tr.quality_notes = "Page created with content" if tr.quality_pass else "Content insertion failed"
            return

    tr.quality_pass = False
    tr.quality_notes = "Page creation failed"


def task_multistep_cli(tr: TaskResult):
    """CLI path: create table, add columns, insert rows, read back."""
    # Step 1: create table with columns
    data, raw, rb, rpb, ms = cli_call("table_create", {
        "uri": CANVAS_URI,
        "name": f"CLI Eval Table {RUN_ID}",
        "columns": [
            {"name": "Name", "type": "text"},
            {"name": "Score", "type": "number"},
            {"name": "Active", "type": "checkbox"}
        ]
    })
    tr.calls.append(CallMetric("multistep", "cli", "table_create",
                               rb, rpb, ms, data is not None))

    if not data or not isinstance(data, dict):
        tr.quality_pass = False
        tr.quality_notes = "Table creation failed"
        return

    table_uri = data.get("tableUri")
    col_ids = [c.get("columnId") or c.get("id") for c in data.get("columns", [])]
    if not table_uri or len(col_ids) < 3:
        tr.quality_pass = False
        tr.quality_notes = f"Missing table URI or columns: {json.dumps(data)[:200]}"
        return

    time.sleep(2)

    # Step 2: insert 5 rows
    rows = [
        [f"Item-{i}", i * 10, i % 2 == 0]
        for i in range(1, 6)
    ]
    data2, raw2, rb2, rpb2, ms2 = cli_call("table_add_rows", {
        "uri": table_uri,
        "columns": col_ids,
        "rows": rows
    })
    tr.calls.append(CallMetric("multistep", "cli", "table_add_rows",
                               rb2, rpb2, ms2, data2 is not None))

    time.sleep(2)

    # Step 3: read rows back (with --pick to reduce output)
    data3, raw3, rb3, rpb3, ms3 = cli_call("table_read_rows", {
        "uri": table_uri
    })
    tr.calls.append(CallMetric("multistep", "cli", "table_read_rows",
                               rb3, rpb3, ms3, data3 is not None))

    if data3 and isinstance(data3, dict):
        items = data3.get("items", data3.get("rows", []))
        tr.quality_pass = len(items) >= 5
        tr.quality_notes = f"Read back {len(items)} rows" if tr.quality_pass else f"Expected 5 rows, got {len(items)}"
    else:
        tr.quality_pass = False
        tr.quality_notes = "Failed to read rows back"


def task_multistep_mcp(tr: TaskResult):
    """MCP path: create table, add columns, insert rows, read back."""
    data, raw, rb, rpb, ms = mcp_call("table_create", {
        "uri": CANVAS_URI,
        "name": f"MCP Eval Table {RUN_ID}",
        "columns": [
            {"name": "Name", "type": "text"},
            {"name": "Score", "type": "number"},
            {"name": "Active", "type": "checkbox"}
        ]
    })
    tr.calls.append(CallMetric("multistep", "mcp", "table_create",
                               rb, rpb, ms, data is not None))

    if not data or not isinstance(data, dict):
        tr.quality_pass = False
        tr.quality_notes = "Table creation failed"
        return

    table_uri = data.get("tableUri")
    col_ids = [c.get("columnId") or c.get("id") for c in data.get("columns", [])]
    if not table_uri or len(col_ids) < 3:
        tr.quality_pass = False
        tr.quality_notes = f"Missing table URI or columns"
        return

    time.sleep(2)

    data2, raw2, rb2, rpb2, ms2 = mcp_call("table_add_rows", {
        "uri": table_uri,
        "columns": col_ids,
        "rows": [[f"Item-{i}", i * 10, i % 2 == 0] for i in range(1, 6)]
    })
    tr.calls.append(CallMetric("multistep", "mcp", "table_add_rows",
                               rb2, rpb2, ms2, data2 is not None))

    time.sleep(2)

    data3, raw3, rb3, rpb3, ms3 = mcp_call("table_read_rows", {
        "uri": table_uri
    })
    tr.calls.append(CallMetric("multistep", "mcp", "table_read_rows",
                               rb3, rpb3, ms3, data3 is not None))

    if data3 and isinstance(data3, dict):
        items = data3.get("items", data3.get("rows", []))
        tr.quality_pass = len(items) >= 5
        tr.quality_notes = f"Read back {len(items)} rows" if tr.quality_pass else f"Expected 5 rows, got {len(items)}"
    else:
        tr.quality_pass = False
        tr.quality_notes = "Failed to read rows back"


def task_error_recovery_cli(tr: TaskResult):
    """CLI path: call with missing field, observe error, fix and retry."""
    # Step 1: intentional bad call
    data, raw, rb, rpb, ms = cli_call("page_create", {"uri": DOC_URI})  # missing title
    tr.calls.append(CallMetric("error_recovery", "cli", "page_create (bad)",
                               rb, rpb, ms, success=False,
                               error="intentional — missing title"))

    # Step 2: correct call
    data2, raw2, rb2, rpb2, ms2 = cli_call("page_create", {
        "uri": DOC_URI, "title": f"CLI Recovery Page {RUN_ID}"
    })
    tr.calls.append(CallMetric("error_recovery", "cli", "page_create (fixed)",
                               rb2, rpb2, ms2, data2 is not None))

    tr.quality_pass = data is None and data2 is not None
    tr.quality_notes = "Error caught client-side, retry succeeded" if tr.quality_pass else "Unexpected result"


def task_error_recovery_mcp(tr: TaskResult):
    """MCP path: call with missing field, observe error, fix and retry."""
    data, raw, rb, rpb, ms = mcp_call("page_create", {"uri": DOC_URI})  # missing title
    tr.calls.append(CallMetric("error_recovery", "mcp", "page_create (bad)",
                               rb, rpb, ms, success=False,
                               error="intentional — missing title"))

    data2, raw2, rb2, rpb2, ms2 = mcp_call("page_create", {
        "uri": DOC_URI, "title": f"MCP Recovery Page {RUN_ID}"
    })
    tr.calls.append(CallMetric("error_recovery", "mcp", "page_create (fixed)",
                               rb2, rpb2, ms2, data2 is not None))

    tr.quality_pass = data is None and data2 is not None
    tr.quality_notes = "Error caught, retry succeeded" if tr.quality_pass else "Unexpected result"


# ---------------------------------------------------------------------------
# Report generation
# ---------------------------------------------------------------------------

def generate_report(results: list[TaskResult]) -> str:
    lines = []
    lines.append("=" * 70)
    lines.append("  SHD CLI vs Coda MCP — Evaluation Report")
    lines.append("=" * 70)
    lines.append("")

    # Group by task
    tasks = {}
    for r in results:
        tasks.setdefault(r.task, {})[r.path] = r

    lines.append(f"{'Task':<20} {'Path':<6} {'Calls':>6} {'Req KB':>8} {'Resp KB':>9} {'Time ms':>8} {'Quality':>8}")
    lines.append("-" * 70)

    totals = {"cli": {"calls": 0, "req": 0, "resp": 0, "ms": 0, "pass": 0, "total": 0},
              "mcp": {"calls": 0, "req": 0, "resp": 0, "ms": 0, "pass": 0, "total": 0}}

    for task_name, paths in tasks.items():
        for path_name in ["cli", "mcp"]:
            if path_name not in paths:
                continue
            r = paths[path_name]
            req_kb = r.total_request_bytes / 1024
            resp_kb = r.total_response_bytes / 1024
            q = "PASS" if r.quality_pass else "FAIL"
            lines.append(f"{task_name:<20} {path_name:<6} {r.call_count:>6} {req_kb:>7.1f}k {resp_kb:>8.1f}k {r.total_duration_ms:>7}  {q:>8}")
            totals[path_name]["calls"] += r.call_count
            totals[path_name]["req"] += r.total_request_bytes
            totals[path_name]["resp"] += r.total_response_bytes
            totals[path_name]["ms"] += r.total_duration_ms
            totals[path_name]["total"] += 1
            totals[path_name]["pass"] += 1 if r.quality_pass else 0

    lines.append("-" * 70)
    lines.append("")
    lines.append("TOTALS:")
    for path in ["cli", "mcp"]:
        t = totals[path]
        lines.append(f"  {path.upper()}: {t['calls']} calls, "
                     f"{t['req']/1024:.1f}k request bytes, "
                     f"{t['resp']/1024:.1f}k response bytes, "
                     f"{t['ms']}ms total, "
                     f"{t['pass']}/{t['total']} quality pass")

    lines.append("")

    # Token estimation (rough: 1 token ≈ 4 bytes for English text / JSON)
    cli_tokens = (totals["cli"]["req"] + totals["cli"]["resp"]) / 4
    mcp_tokens = (totals["mcp"]["req"] + totals["mcp"]["resp"]) / 4
    if mcp_tokens > 0:
        ratio = mcp_tokens / max(cli_tokens, 1)
        lines.append(f"TOKEN ESTIMATE (req+resp bytes / 4):")
        lines.append(f"  CLI: ~{int(cli_tokens)} tokens")
        lines.append(f"  MCP: ~{int(mcp_tokens)} tokens")
        lines.append(f"  MCP/CLI ratio: {ratio:.2f}x")
        if ratio > 1:
            lines.append(f"  → MCP uses ~{ratio:.1f}x more tokens than CLI")
        else:
            lines.append(f"  → CLI uses ~{1/ratio:.1f}x more tokens than MCP")

    lines.append("")
    lines.append("QUALITY NOTES:")
    for r in results:
        lines.append(f"  [{r.path.upper()}] {r.task}: {r.quality_notes}")

    lines.append("")
    lines.append("AGENT EASE-OF-USE ASSESSMENT:")
    lines.append("  CLI advantages:")
    lines.append("    - --pick reduces response size (fewer tokens in context)")
    lines.append("    - --filter on discover avoids parsing full tool list")
    lines.append("    - Client-side validation catches errors before network round-trip")
    lines.append("    - --dry-run enables safe exploration without auth")
    lines.append("    - --fuzzy tolerates tool name typos")
    lines.append("    - Skill files provide structured guidance")
    lines.append("  MCP advantages:")
    lines.append("    - Native JSON-RPC — no shell escaping needed")
    lines.append("    - Single persistent connection (if kept alive)")
    lines.append("    - Standard protocol — works with any MCP client")
    lines.append("")

    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    global DOC_ID, DOC_URI, CANVAS_URI

    parser = argparse.ArgumentParser(description="SHD CLI vs MCP evaluation harness")
    parser.add_argument("--doc-id", help="Existing doc ID (skips doc creation)")
    args = parser.parse_args()

    if args.doc_id:
        DOC_ID = args.doc_id
    else:
        print("Creating throwaway evaluation doc...")
        data, raw, _, _, _ = cli_call("document_create", {"title": "[EVAL] Harness Run"})
        if not data or not isinstance(data, dict):
            print(f"FATAL: Could not create doc: {raw}", file=sys.stderr)
            sys.exit(1)
        DOC_ID = data.get("docUri", "").split("/")[-1]
        if not DOC_ID:
            print(f"FATAL: No docId in response: {raw}", file=sys.stderr)
            sys.exit(1)
        print(f"  Doc created: {DOC_ID}")
        time.sleep(2)

    DOC_URI = f"coda://docs/{DOC_ID}"

    # Get canvas URI from doc structure
    print("Reading doc structure...")
    data, _, _, _, _ = cli_call("document_read", {"uri": DOC_URI})
    if data and isinstance(data, dict):
        pages = data.get("pages", [])
        if pages:
            CANVAS_URI = pages[0].get("canvasUri")

    if not CANVAS_URI:
        print("FATAL: Could not get canvas URI from doc", file=sys.stderr)
        sys.exit(1)

    print(f"  Doc URI: {DOC_URI}")
    print(f"  Canvas URI: {CANVAS_URI}")
    print()

    # Define task pairs
    task_pairs = [
        ("discovery",       task_discovery_cli,       task_discovery_mcp),
        ("read",            task_read_cli,             task_read_mcp),
        ("write",           task_write_cli,            task_write_mcp),
        ("multistep",       task_multistep_cli,        task_multistep_mcp),
        ("error_recovery",  task_error_recovery_cli,   task_error_recovery_mcp),
    ]

    for task_name, cli_fn, mcp_fn in task_pairs:
        print(f"Running task: {task_name}")

        # CLI path
        tr_cli = TaskResult(task=task_name, path="cli")
        try:
            cli_fn(tr_cli)
        except Exception as e:
            tr_cli.quality_pass = False
            tr_cli.quality_notes = f"Exception: {e}"
        tr_cli.finalize()
        results.append(tr_cli)
        print(f"  CLI: {tr_cli.call_count} calls, {tr_cli.total_duration_ms}ms, "
              f"{'PASS' if tr_cli.quality_pass else 'FAIL'}")

        # MCP path
        tr_mcp = TaskResult(task=task_name, path="mcp")
        try:
            mcp_fn(tr_mcp)
        except Exception as e:
            tr_mcp.quality_pass = False
            tr_mcp.quality_notes = f"Exception: {e}"
        tr_mcp.finalize()
        results.append(tr_mcp)
        print(f"  MCP: {tr_mcp.call_count} calls, {tr_mcp.total_duration_ms}ms, "
              f"{'PASS' if tr_mcp.quality_pass else 'FAIL'}")
        print()

    # Generate and print report
    report = generate_report(results)
    print(report)

    # Save report and raw data
    report_path = "eval/eval_report.txt"
    with open(report_path, "w") as f:
        f.write(report)
    print(f"\nReport saved to {report_path}")

    data_path = "eval/eval_data.json"
    with open(data_path, "w") as f:
        json.dump([asdict(r) for r in results], f, indent=2)
    print(f"Raw data saved to {data_path}")

    # Exit code: 1 if any quality checks failed
    all_pass = all(r.quality_pass for r in results)
    sys.exit(0 if all_pass else 1)


if __name__ == "__main__":
    main()
