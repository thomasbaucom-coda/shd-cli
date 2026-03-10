#!/usr/bin/env python3
"""Comparative doc-build benchmark: SHD CLI vs Coda MCP.

Builds an identical 10-page document with multiple tables and ~150 rows
of fake data through both the CLI and MCP paths, measuring:
  - Per-call token proxy (request + response bytes)
  - Latency per call and cumulative
  - Total tool calls required
  - Data integrity (row counts verified)

Usage:
    python3 eval/eval_doc_build.py

Creates two throwaway docs (one per path) and produces a side-by-side report.
"""

import json
import os
import random
import subprocess
import sys
import time
import uuid
from dataclasses import dataclass, field, asdict
from typing import Any, Optional

RUN_ID = uuid.uuid4().hex[:6]
CLI = os.environ.get("SHD_CLI", "./target/release/shd")

# ---------------------------------------------------------------------------
# Metrics
# ---------------------------------------------------------------------------

@dataclass
class CallMetric:
    phase: str
    tool: str
    request_bytes: int = 0
    response_bytes: int = 0
    duration_ms: int = 0
    success: bool = True
    error: str = ""

@dataclass
class BuildResult:
    path: str  # "cli" or "mcp"
    doc_id: str = ""
    calls: list = field(default_factory=list)
    pages_created: int = 0
    tables_created: int = 0
    rows_inserted: int = 0
    content_blocks: int = 0
    errors: list = field(default_factory=list)

    @property
    def total_calls(self):
        return len(self.calls)

    @property
    def total_request_bytes(self):
        return sum(c.request_bytes for c in self.calls)

    @property
    def total_response_bytes(self):
        return sum(c.response_bytes for c in self.calls)

    @property
    def total_duration_ms(self):
        return sum(c.duration_ms for c in self.calls)

    @property
    def total_tokens_est(self):
        return (self.total_request_bytes + self.total_response_bytes) / 4

# ---------------------------------------------------------------------------
# CLI path helpers
# ---------------------------------------------------------------------------

def cli_call(tool, payload=None, pick=None):
    """Call shd <tool> and return (data, CallMetric)."""
    args = [CLI, tool]
    payload_str = ""
    if payload is not None:
        payload_str = json.dumps(payload, separators=(',', ':'))
        args += ["--json", payload_str]
    if pick:
        args += ["--pick", pick]
    args += ["--trace"]

    req_bytes = len(tool.encode()) + len(payload_str.encode())
    start = time.monotonic()
    proc = subprocess.run(args, capture_output=True, text=True)
    elapsed_ms = int((time.monotonic() - start) * 1000)
    stdout = proc.stdout.strip()
    resp_bytes = len(stdout.encode())

    metric = CallMetric("", tool, req_bytes, resp_bytes, elapsed_ms)

    if proc.returncode != 0:
        metric.success = False
        metric.error = proc.stderr.strip()[:200]
        return None, metric

    try:
        parsed = json.loads(stdout)
        if isinstance(parsed, dict) and "error" in parsed and len(parsed) == 1:
            metric.success = False
            metric.error = str(parsed["error"])[:200]
            return None, metric
        return parsed, metric
    except json.JSONDecodeError:
        return stdout, metric


def mcp_call(tool, arguments=None):
    """Call tool via MCP server and return (data, CallMetric)."""
    rpc_request = json.dumps({
        "jsonrpc": "2.0", "id": 1,
        "method": "tools/call",
        "params": {"name": tool, "arguments": arguments or {}}
    })
    init = json.dumps({
        "jsonrpc": "2.0", "id": 0,
        "method": "initialize",
        "params": {"protocolVersion": "2024-11-05",
                    "clientInfo": {"name": "eval-build", "version": "0.1"}}
    })
    notif = json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"})
    stdin_data = f"{init}\n{notif}\n{rpc_request}\n"
    req_bytes = len(rpc_request.encode())

    start = time.monotonic()
    proc = subprocess.run([CLI, "mcp"], capture_output=True, text=True,
                          input=stdin_data, timeout=60)
    elapsed_ms = int((time.monotonic() - start) * 1000)

    resp_lines = [l for l in proc.stdout.strip().split("\n") if l.strip()]
    resp_bytes = sum(len(l.encode()) for l in resp_lines)
    metric = CallMetric("", tool, req_bytes, resp_bytes, elapsed_ms)

    for line in resp_lines:
        try:
            msg = json.loads(line)
            if msg.get("id") == 1:
                if "error" in msg:
                    metric.success = False
                    metric.error = json.dumps(msg["error"])[:200]
                    return None, metric
                result = msg.get("result", {})
                content = result.get("content", [])
                if content and content[0].get("text"):
                    try:
                        parsed = json.loads(content[0]["text"])
                        return parsed, metric
                    except json.JSONDecodeError:
                        return content[0]["text"], metric
                return result, metric
        except json.JSONDecodeError:
            continue

    metric.success = False
    metric.error = "No response found"
    return None, metric


# ---------------------------------------------------------------------------
# Document blueprint — 10 pages, 6 tables, ~150 rows
# ---------------------------------------------------------------------------

PAGE_SPECS = [
    {"title": "Overview",       "content": "# Project Atlas\n\n**Status:** Active | **Lead:** Sarah Chen | **Target:** Q3 2026\n\nProject Atlas is a cross-functional initiative to build the next-generation workspace platform. This doc tracks all workstreams, team members, deliverables, and risks."},
    {"title": "Engineering",    "content": "# Engineering Workstream\n\nCore platform development covering backend services, API, and infrastructure.\n\n**Lead:** Sarah Chen\n**Team Size:** 8 engineers\n**Sprint Cadence:** 2-week sprints"},
    {"title": "Design",         "content": "# Design Workstream\n\nUI/UX design, design system maintenance, and user research.\n\n**Lead:** Marcus Rivera\n**Team Size:** 4 designers"},
    {"title": "Marketing",      "content": "# Marketing Workstream\n\nLaunch campaigns, content strategy, and go-to-market planning.\n\n**Lead:** Priya Patel\n**Budget:** $300K"},
    {"title": "Sales",          "content": "# Sales Workstream\n\nEnterprise pipeline, pricing strategy, and partner enablement.\n\n**Lead:** James Wilson\n**Q3 Target:** $2M ARR"},
    {"title": "Support",        "content": "# Support Workstream\n\nDocumentation, training materials, and launch readiness.\n\n**Lead:** Ana Rodriguez"},
    {"title": "Milestones",     "content": "# Key Milestones\n\nAll cross-functional milestones and delivery dates."},
    {"title": "Risk Register",  "content": "# Risk Register\n\nTracked risks with likelihood, impact, and mitigation plans.\n\n> Review weekly. Escalate High/Critical risks immediately."},
    {"title": "Metrics",        "content": "# Launch Metrics\n\nKey performance indicators and success criteria.\n\n**North Star:** 10K active users in first 30 days"},
    {"title": "Meeting Notes",  "content": "# Meeting Notes\n\nWeekly standup notes and decision log.\n\n---\n\n## Week 1 — Mar 9, 2026\n- Kickoff complete\n- All workstream leads confirmed\n- First milestone review scheduled for Mar 20"},
]

TABLE_SPECS = [
    {
        "page_index": 0,  # Overview
        "name": "Team Directory",
        "columns": [
            {"name": "Name"},
            {"name": "Role"},
            {"name": "Department"},
            {"name": "Email"},
            {"name": "Location"},
            {"name": "Capacity"},
        ],
        "rows": [
            ["Sarah Chen", "Engineering Lead", "Engineering", "sarah@co.com", "San Francisco", 100],
            ["Alex Kim", "Senior Backend", "Engineering", "alex.k@co.com", "San Francisco", 100],
            ["Jordan Lee", "Senior Frontend", "Engineering", "jordan@co.com", "New York", 100],
            ["Taylor Swift", "Platform Eng", "Engineering", "taylor.s@co.com", "Remote", 80],
            ["Casey Brown", "DevOps", "Engineering", "casey@co.com", "San Francisco", 100],
            ["Morgan Chen", "QA Lead", "Engineering", "morgan@co.com", "San Francisco", 100],
            ["Robin Diaz", "Backend Eng", "Engineering", "robin@co.com", "Austin", 100],
            ["Jamie Park", "Frontend Eng", "Engineering", "jamie.p@co.com", "Remote", 80],
            ["Marcus Rivera", "Design Lead", "Design", "marcus@co.com", "New York", 100],
            ["Elena Vasquez", "Senior Designer", "Design", "elena@co.com", "New York", 100],
            ["Sam Okafor", "UX Researcher", "Design", "sam.o@co.com", "Remote", 80],
            ["Kai Tanaka", "Motion Designer", "Design", "kai@co.com", "Tokyo", 60],
            ["Priya Patel", "Marketing Lead", "Marketing", "priya@co.com", "San Francisco", 100],
            ["David Park", "Content Strategy", "Marketing", "david.p@co.com", "New York", 100],
            ["Lisa Wang", "Growth Marketing", "Marketing", "lisa@co.com", "Remote", 80],
            ["Chris Johnson", "PMM", "Marketing", "chris.j@co.com", "San Francisco", 100],
            ["James Wilson", "Sales Lead", "Sales", "james@co.com", "Chicago", 100],
            ["Rachel Green", "Enterprise AE", "Sales", "rachel@co.com", "New York", 100],
            ["Mike Torres", "SMB AE", "Sales", "mike.t@co.com", "Remote", 100],
            ["Nina Patel", "Sales Engineer", "Sales", "nina@co.com", "San Francisco", 80],
            ["Ana Rodriguez", "Support Lead", "Support", "ana@co.com", "Austin", 100],
            ["Ben Davis", "Support Eng", "Support", "ben.d@co.com", "Austin", 100],
        ],
    },
    {
        "page_index": 6,  # Milestones
        "name": "Milestones",
        "columns": [
            {"name": "Milestone"},
            {"name": "Date"},
            {"name": "Workstream"},
            {"name": "Status"},
            {"name": "Owner"},
        ],
        "rows": [
            ["Architecture review", "2026-03-15", "Engineering", "Complete", "Sarah Chen"],
            ["Design system v2", "2026-03-30", "Design", "Complete", "Marcus Rivera"],
            ["API beta ready", "2026-04-15", "Engineering", "In Progress", "Alex Kim"],
            ["Marketing site redesign", "2026-04-20", "Marketing", "In Progress", "Priya Patel"],
            ["Enterprise pricing final", "2026-04-30", "Sales", "Upcoming", "James Wilson"],
            ["Internal dogfood", "2026-05-01", "All", "Upcoming", "Sarah Chen"],
            ["Support docs complete", "2026-05-15", "Support", "Upcoming", "Ana Rodriguez"],
            ["Sales enablement", "2026-05-20", "Sales", "Upcoming", "James Wilson"],
            ["Launch campaign live", "2026-06-01", "Marketing", "Upcoming", "Priya Patel"],
            ["Press embargo lifts", "2026-06-10", "Marketing", "Upcoming", "David Park"],
            ["GA launch", "2026-06-15", "All", "Upcoming", "Sarah Chen"],
            ["Post-launch retro", "2026-07-01", "All", "Upcoming", "Sarah Chen"],
        ],
    },
    {
        "page_index": 7,  # Risk Register
        "name": "Risks",
        "columns": [
            {"name": "Risk"},
            {"name": "Likelihood"},
            {"name": "Impact"},
            {"name": "Workstream"},
            {"name": "Mitigation"},
            {"name": "Owner"},
            {"name": "Status"},
        ],
        "rows": [
            ["API perf under load", "Medium", "Critical", "Engineering", "Load testing week 3", "Casey Brown", "Mitigating"],
            ["Design migration breaks UI", "Low", "High", "Design", "Phased rollout", "Marcus Rivera", "Open"],
            ["Marketing budget cut", "High", "High", "Marketing", "Prioritize organic", "Priya Patel", "Mitigating"],
            ["Key engineer leaves", "Low", "Critical", "Engineering", "Cross-training", "Sarah Chen", "Open"],
            ["Competitor launches first", "Medium", "Medium", "All", "Focus differentiation", "James Wilson", "Accepted"],
            ["Data migration fails", "Medium", "High", "Engineering", "Dry runs + rollback", "Alex Kim", "Mitigating"],
            ["Sales not trained", "Low", "Medium", "Sales", "Start enablement early", "James Wilson", "Open"],
            ["Third-party API breaks", "Low", "High", "Engineering", "Fallback impls", "Taylor Swift", "Open"],
            ["Launch date slips", "Medium", "High", "All", "Weekly reviews", "Sarah Chen", "Open"],
            ["Support overwhelmed", "High", "Medium", "Support", "Hire temp, self-serve", "Ana Rodriguez", "Mitigating"],
            ["Accessibility gaps", "Medium", "High", "Design", "Audit + remediation", "Elena Vasquez", "Mitigating"],
            ["Localization not ready", "High", "Medium", "Marketing", "Phase 2", "Lisa Wang", "Accepted"],
            ["Security review delays", "Low", "Critical", "Engineering", "Engaged early", "Morgan Chen", "Open"],
            ["Beta feedback negative", "Medium", "High", "All", "Rapid iteration", "Sarah Chen", "Open"],
            ["Partners not ready", "Medium", "Medium", "Engineering", "Shared timeline", "Alex Kim", "Mitigating"],
        ],
    },
    {
        "page_index": 8,  # Metrics
        "name": "Launch KPIs",
        "columns": [
            {"name": "Metric"},
            {"name": "Target"},
            {"name": "Current"},
            {"name": "Status"},
            {"name": "Owner"},
        ],
        "rows": [
            ["Active users (30-day)", "10,000", "0", "Not Started", "Sarah Chen"],
            ["NPS score", "> 50", "—", "Not Started", "Sam Okafor"],
            ["Enterprise deals signed", "15", "3", "In Progress", "James Wilson"],
            ["Support ticket resolution (p50)", "< 4 hours", "—", "Not Started", "Ana Rodriguez"],
            ["API uptime", "99.95%", "99.98%", "On Track", "Casey Brown"],
            ["Docs coverage", "100%", "35%", "In Progress", "Ben Davis"],
            ["Marketing qualified leads", "500", "120", "In Progress", "Priya Patel"],
            ["Feature adoption rate", "> 60%", "—", "Not Started", "Marcus Rivera"],
            ["Revenue (Q3)", "$2M ARR", "$800K", "In Progress", "James Wilson"],
            ["Bug escape rate", "< 2%", "1.5%", "On Track", "Morgan Chen"],
        ],
    },
]

def generate_tasks():
    """Generate ~50 tasks across workstreams."""
    random.seed(42)
    eng = ["Build auth service", "API v2 endpoints", "DB migration", "WebSocket support",
           "Rate limiting", "Search indexer", "Admin API", "Monitoring setup",
           "Caching layer", "File upload service", "Webhook system", "Audit logging",
           "Notification service", "Feature flags", "SSO integration"]
    design = ["Component library v2", "Onboarding flow", "Dashboard redesign",
              "Settings page", "Icon set", "Email templates", "Accessibility audit",
              "Mobile responsive", "Dark mode", "Animation specs"]
    mktg = ["Launch blog post", "Launch video", "Landing page", "Press release",
            "Social calendar", "Case studies", "Email campaign", "Product demo",
            "SEO optimization", "Partner materials"]
    sales = ["Pricing page", "Enterprise template", "ROI calculator", "Feature training",
             "CRM workflows", "Battle cards", "Demo environment", "Objection guide"]
    support = ["Help center articles", "Video tutorials", "Troubleshooting guide",
               "Ticketing setup", "Escalation procedures", "Migration guide", "FAQ doc"]

    ws_map = {
        "Engineering": (eng, ["Sarah Chen", "Alex Kim", "Jordan Lee", "Casey Brown", "Morgan Chen"]),
        "Design": (design, ["Marcus Rivera", "Elena Vasquez", "Sam Okafor"]),
        "Marketing": (mktg, ["Priya Patel", "David Park", "Lisa Wang"]),
        "Sales": (sales, ["James Wilson", "Rachel Green", "Mike Torres"]),
        "Support": (support, ["Ana Rodriguez", "Ben Davis"]),
    }
    statuses = ["To Do", "In Progress", "In Review", "Done", "Blocked"]
    priorities = ["Low", "Medium", "High", "Critical"]

    rows = []
    for ws, (tasks, people) in ws_map.items():
        for task in tasks:
            status = random.choices(statuses, weights=[30, 25, 15, 25, 5])[0]
            priority = random.choices(priorities, weights=[20, 35, 30, 15])[0]
            assignee = random.choice(people)
            effort = random.randint(1, 5)
            month = random.randint(3, 7)
            day = random.randint(1, 28)
            rows.append([task, ws, status, priority, assignee, effort, f"2026-{month:02d}-{day:02d}"])
    random.shuffle(rows)
    return rows

TASK_TABLE_SPEC = {
    "page_index": 1,  # Engineering (first workstream page)
    "name": "All Tasks",
    "columns": [
        {"name": "Task"},
        {"name": "Workstream"},
        {"name": "Status"},
        {"name": "Priority"},
        {"name": "Assignee"},
        {"name": "Effort"},
        {"name": "Due Date"},
    ],
}

# Extra content for pages that already have tables
EXTRA_CONTENT = {
    1: "## Sprint Board\nCurrent sprint items are tagged In Progress. Use the table below to track all tasks across workstreams.",
    6: "## Timeline\nAll dates are targets. Escalate if any milestone is at risk of slipping by more than 1 week.",
    7: "## Risk Matrix\n| | Low Impact | Medium | High | Critical |\n|---|---|---|---|---|\n| High Likelihood | Accept | Mitigate | Mitigate | Escalate |\n| Medium | Accept | Monitor | Mitigate | Mitigate |\n| Low | Accept | Accept | Monitor | Mitigate |",
    8: "## Tracking Cadence\nMetrics are updated weekly by each workstream lead. Dashboard review every Monday.",
    9: "## Action Items\n- [ ] Schedule Week 2 standup\n- [ ] Share design review calendar invite\n- [ ] Send out engineering onboarding docs\n- [x] Confirm exec sponsor",
}


# ---------------------------------------------------------------------------
# Build functions
# ---------------------------------------------------------------------------

def build_via_cli():
    """Build the full doc via shd CLI. Returns BuildResult."""
    br = BuildResult(path="cli")
    print(f"\n{'='*60}")
    print(f"  BUILDING VIA CLI (run {RUN_ID})")
    print(f"{'='*60}")

    # 1. Create doc
    print("\n[1/7] Creating doc...")
    data, m = cli_call("document_create", {"title": f"[EVAL-CLI] Atlas {RUN_ID}"})
    m.phase = "create_doc"
    br.calls.append(m)
    if not data or not isinstance(data, dict):
        br.errors.append(f"Doc creation failed: {m.error}")
        return br
    doc_uri = data.get("docUri")
    br.doc_id = doc_uri.split("/")[-1] if doc_uri else ""
    first_canvas = data.get("pages", [{}])[0].get("canvasUri")
    first_page = data.get("pages", [{}])[0].get("pageUri")
    print(f"  Doc: {br.doc_id}")
    time.sleep(2)

    # 2. Rename first page to Overview, create 9 more pages
    print("\n[2/7] Creating 10 pages...")
    data, m = cli_call("page_update", {
        "uri": first_page,
        "updateFields": {"title": PAGE_SPECS[0]["title"]}
    })
    m.phase = "create_pages"
    br.calls.append(m)

    page_canvases = [first_canvas]  # index 0 = Overview
    page_uris = [first_page]

    for i, spec in enumerate(PAGE_SPECS[1:], 1):
        data, m = cli_call("page_create", {
            "uri": doc_uri, "title": spec["title"]
        })
        m.phase = "create_pages"
        br.calls.append(m)
        if data and isinstance(data, dict):
            page_canvases.append(data.get("canvasUri"))
            page_uris.append(data.get("pageUri"))
            br.pages_created += 1
        else:
            page_canvases.append(None)
            page_uris.append(None)
            br.errors.append(f"Page {spec['title']} failed: {m.error}")
        time.sleep(0.5)

    br.pages_created += 1  # count the renamed first page
    print(f"  Created {br.pages_created} pages")

    # 3. Add content to all pages
    print("\n[3/7] Adding page content...")
    for i, spec in enumerate(PAGE_SPECS):
        canvas = page_canvases[i]
        if not canvas:
            continue
        data, m = cli_call("content_modify", {
            "uri": canvas,
            "operations": [{"operation": "insert_element", "blockType": "markdown",
                            "content": spec["content"]}]
        })
        m.phase = "add_content"
        br.calls.append(m)
        if m.success:
            br.content_blocks += 1
        time.sleep(0.5)

    # Extra content for specific pages
    for page_idx, content in EXTRA_CONTENT.items():
        canvas = page_canvases[page_idx] if page_idx < len(page_canvases) else None
        if not canvas:
            continue
        data, m = cli_call("content_modify", {
            "uri": canvas,
            "operations": [{"operation": "insert_element", "blockType": "markdown",
                            "content": content}]
        })
        m.phase = "add_content_extra"
        br.calls.append(m)
        if m.success:
            br.content_blocks += 1
        time.sleep(0.3)

    print(f"  Added {br.content_blocks} content blocks")

    # 4. Create tables
    print("\n[4/7] Creating tables...")
    table_uris = {}
    table_col_ids = {}

    for spec in TABLE_SPECS:
        canvas = page_canvases[spec["page_index"]]
        if not canvas:
            continue
        time.sleep(1)
        data, m = cli_call("table_create", {
            "uri": canvas,
            "name": spec["name"],
            "columns": spec["columns"],
        })
        m.phase = "create_tables"
        br.calls.append(m)
        if data and isinstance(data, dict):
            tbl_uri = data.get("tableUri")
            col_ids = [c.get("columnId") or c.get("id") for c in data.get("columns", [])]
            table_uris[spec["name"]] = tbl_uri
            table_col_ids[spec["name"]] = col_ids
            br.tables_created += 1
            print(f"  {spec['name']}: {len(col_ids)} cols")
        else:
            br.errors.append(f"Table {spec['name']} failed: {m.error}")

    # Task table
    canvas = page_canvases[TASK_TABLE_SPEC["page_index"]]
    if canvas:
        time.sleep(1)
        data, m = cli_call("table_create", {
            "uri": canvas,
            "name": TASK_TABLE_SPEC["name"],
            "columns": TASK_TABLE_SPEC["columns"],
        })
        m.phase = "create_tables"
        br.calls.append(m)
        if data and isinstance(data, dict):
            table_uris["All Tasks"] = data.get("tableUri")
            table_col_ids["All Tasks"] = [c.get("columnId") or c.get("id") for c in data.get("columns", [])]
            br.tables_created += 1
            print(f"  All Tasks: {len(table_col_ids['All Tasks'])} cols")

    print(f"  Created {br.tables_created} tables")

    # 5. Insert rows into tables
    print("\n[5/7] Inserting rows...")
    for spec in TABLE_SPECS:
        tbl_uri = table_uris.get(spec["name"])
        col_ids = table_col_ids.get(spec["name"])
        if not tbl_uri or not col_ids:
            continue
        time.sleep(2)
        data, m = cli_call("table_add_rows", {
            "uri": tbl_uri,
            "columns": col_ids,
            "rows": spec["rows"],
        })
        m.phase = "insert_rows"
        br.calls.append(m)
        if m.success:
            br.rows_inserted += len(spec["rows"])
            print(f"  {spec['name']}: +{len(spec['rows'])} rows")
        else:
            br.errors.append(f"Rows for {spec['name']} failed: {m.error}")

    # Insert tasks — all in one batch to avoid Coda select-list inference issues
    task_rows = generate_tasks()
    tbl_uri = table_uris.get("All Tasks")
    col_ids = table_col_ids.get("All Tasks")
    if tbl_uri and col_ids:
        time.sleep(2)
        data, m = cli_call("table_add_rows", {
            "uri": tbl_uri,
            "columns": col_ids,
            "rows": task_rows,
        })
        m.phase = "insert_tasks"
        br.calls.append(m)
        if m.success:
            br.rows_inserted += len(task_rows)
            print(f"  All Tasks: +{len(task_rows)} rows")
        else:
            br.errors.append(f"Task insert failed: {m.error}")

    print(f"  Total rows inserted: {br.rows_inserted}")

    # 6. Verify by reading back
    print("\n[6/7] Verifying doc structure...")
    data, m = cli_call("document_read", {"uri": doc_uri})
    m.phase = "verify"
    br.calls.append(m)
    if data and isinstance(data, dict):
        actual_pages = len(data.get("pages", []))
        print(f"  Pages in doc: {actual_pages}")

    # 7. Read back one table to verify row count
    print("\n[7/7] Verifying row counts...")
    for tbl_name in ["Team Directory", "All Tasks"]:
        tbl_uri = table_uris.get(tbl_name)
        if not tbl_uri:
            continue
        time.sleep(1)
        data, m = cli_call("table_read_rows", {"uri": tbl_uri})
        m.phase = "verify_rows"
        br.calls.append(m)
        if data and isinstance(data, dict):
            items = data.get("items", data.get("rows", []))
            print(f"  {tbl_name}: {len(items)} rows verified")

    print(f"\n  Doc URL: https://coda.io/d/_d{br.doc_id}")
    return br


def build_via_cli_optimized():
    """Build using CLI with --pick on every call to minimize token usage.

    This represents how a well-tuned agent would actually use the CLI:
    only extract the fields needed for chaining to the next step.
    """
    br = BuildResult(path="cli+pick")
    print(f"\n{'='*60}")
    print(f"  BUILDING VIA CLI + --pick (run {RUN_ID})")
    print(f"{'='*60}")

    # 1. Create doc — multi-pick docUri and pages
    print("\n[1/7] Creating doc...")
    data, m = cli_call("document_create",
                       {"title": f"[EVAL-PICK] Atlas {RUN_ID}"},
                       pick="docUri,pages")
    m.phase = "create_doc"
    br.calls.append(m)
    if not data or not isinstance(data, dict):
        br.errors.append(f"Doc creation failed: {m.error}")
        return br

    doc_uri = data.get("docUri")
    br.doc_id = doc_uri.split("/")[-1] if doc_uri else ""
    pages_list = data.get("pages", [{}])
    first_canvas = pages_list[0].get("canvasUri") if pages_list else None
    first_page = pages_list[0].get("pageUri") if pages_list else None
    print(f"  Doc: {br.doc_id}")
    time.sleep(2)

    # 2. Create pages — only pick canvasUri and pageUri from each
    print("\n[2/7] Creating 10 pages...")
    data, m = cli_call("page_update", {
        "uri": first_page,
        "updateFields": {"title": PAGE_SPECS[0]["title"]}
    })
    m.phase = "create_pages"
    br.calls.append(m)

    page_canvases = [first_canvas]
    page_uris = [first_page]

    for spec in PAGE_SPECS[1:]:
        # Multi-pick now returns JSON object
        data, m = cli_call("page_create",
                           {"uri": doc_uri, "title": spec["title"]},
                           pick="canvasUri,pageUri")
        m.phase = "create_pages"
        br.calls.append(m)
        if data and isinstance(data, dict):
            page_canvases.append(data.get("canvasUri"))
            page_uris.append(data.get("pageUri"))
            br.pages_created += 1
        else:
            page_canvases.append(None)
            page_uris.append(None)
            br.errors.append(f"Page {spec['title']} failed: {m.error}")
        time.sleep(0.5)

    br.pages_created += 1
    print(f"  Created {br.pages_created} pages")

    # 3. Add content — no useful pick here (response is just confirmation)
    print("\n[3/7] Adding page content...")
    for i, spec in enumerate(PAGE_SPECS):
        canvas = page_canvases[i]
        if not canvas:
            continue
        data, m = cli_call("content_modify", {
            "uri": canvas,
            "operations": [{"operation": "insert_element", "blockType": "markdown",
                            "content": spec["content"]}]
        })
        m.phase = "add_content"
        br.calls.append(m)
        if m.success:
            br.content_blocks += 1
        time.sleep(0.5)

    for page_idx, content in EXTRA_CONTENT.items():
        canvas = page_canvases[page_idx] if page_idx < len(page_canvases) else None
        if not canvas:
            continue
        data, m = cli_call("content_modify", {
            "uri": canvas,
            "operations": [{"operation": "insert_element", "blockType": "markdown",
                            "content": content}]
        })
        m.phase = "add_content_extra"
        br.calls.append(m)
        if m.success:
            br.content_blocks += 1
        time.sleep(0.3)

    print(f"  Added {br.content_blocks} content blocks")

    # 4. Create tables — pick only tableUri and columns
    print("\n[4/7] Creating tables...")
    table_uris = {}
    table_col_ids = {}

    for spec in TABLE_SPECS:
        canvas = page_canvases[spec["page_index"]]
        if not canvas:
            continue
        time.sleep(1)
        # Multi-pick now returns JSON object
        data, m = cli_call("table_create", {
            "uri": canvas,
            "name": spec["name"],
            "columns": spec["columns"],
        }, pick="tableUri,columns")
        m.phase = "create_tables"
        br.calls.append(m)
        if data and isinstance(data, dict):
            tbl_uri = data.get("tableUri")
            col_ids = [c.get("columnId") or c.get("id") for c in data.get("columns", [])]
            table_uris[spec["name"]] = tbl_uri
            table_col_ids[spec["name"]] = col_ids
            br.tables_created += 1
            print(f"  {spec['name']}: {len(col_ids)} cols")
        else:
            br.errors.append(f"Table {spec['name']} failed: {m.error}")

    canvas = page_canvases[TASK_TABLE_SPEC["page_index"]]
    if canvas:
        time.sleep(1)
        data, m = cli_call("table_create", {
            "uri": canvas,
            "name": TASK_TABLE_SPEC["name"],
            "columns": TASK_TABLE_SPEC["columns"],
        }, pick="tableUri,columns")
        m.phase = "create_tables"
        br.calls.append(m)
        if data and isinstance(data, dict):
            table_uris["All Tasks"] = data.get("tableUri")
            cols = data.get("columns", [])
            table_col_ids["All Tasks"] = [c.get("columnId") or c.get("id") for c in cols] if isinstance(cols, list) else []
            br.tables_created += 1
            print(f"  All Tasks: {len(table_col_ids['All Tasks'])} cols")

    print(f"  Created {br.tables_created} tables")

    # 5. Insert rows — pick only rowCount for confirmation
    print("\n[5/7] Inserting rows...")
    for spec in TABLE_SPECS:
        tbl_uri = table_uris.get(spec["name"])
        col_ids = table_col_ids.get(spec["name"])
        if not tbl_uri or not col_ids:
            continue
        time.sleep(2)
        data, m = cli_call("table_add_rows", {
            "uri": tbl_uri,
            "columns": col_ids,
            "rows": spec["rows"],
        }, pick="rowCount")
        m.phase = "insert_rows"
        br.calls.append(m)
        if m.success:
            br.rows_inserted += len(spec["rows"])
            print(f"  {spec['name']}: +{len(spec['rows'])} rows")
        else:
            br.errors.append(f"Rows for {spec['name']} failed: {m.error}")

    task_rows = generate_tasks()
    tbl_uri = table_uris.get("All Tasks")
    col_ids = table_col_ids.get("All Tasks")
    if tbl_uri and col_ids:
        time.sleep(2)
        data, m = cli_call("table_add_rows", {
            "uri": tbl_uri,
            "columns": col_ids,
            "rows": task_rows,
        }, pick="rowCount")
        m.phase = "insert_tasks"
        br.calls.append(m)
        if m.success:
            br.rows_inserted += len(task_rows)
            print(f"  All Tasks: +{len(task_rows)} rows")
        else:
            br.errors.append(f"Task insert failed: {m.error}")

    print(f"  Total rows inserted: {br.rows_inserted}")

    # 6. Verify — pick only page count
    print("\n[6/7] Verifying doc structure...")
    data, m = cli_call("document_read", {"uri": doc_uri}, pick="pages")
    m.phase = "verify"
    br.calls.append(m)
    if data and isinstance(data, dict):
        pages_list = data.get("pages", [])
        print(f"  Pages in doc: {len(pages_list)}")
    elif isinstance(data, list):
        print(f"  Pages in doc: {len(data)}")

    # 7. Verify rows — pick only items count
    print("\n[7/7] Verifying row counts...")
    for tbl_name in ["Team Directory", "All Tasks"]:
        tbl_uri = table_uris.get(tbl_name)
        if not tbl_uri:
            continue
        time.sleep(1)
        data, m = cli_call("table_read_rows", {"uri": tbl_uri}, pick="items")
        m.phase = "verify_rows"
        br.calls.append(m)
        if data and isinstance(data, list):
            print(f"  {tbl_name}: {len(data)} rows verified")
        elif data and isinstance(data, dict):
            items = data.get("items", data.get("rows", []))
            print(f"  {tbl_name}: {len(items)} rows verified")

    print(f"\n  Doc URL: https://coda.io/d/_d{br.doc_id}")
    return br


def build_via_mcp():
    """Build the full doc via MCP server. Returns BuildResult."""
    br = BuildResult(path="mcp")
    print(f"\n{'='*60}")
    print(f"  BUILDING VIA MCP (run {RUN_ID})")
    print(f"{'='*60}")

    # 1. Create doc
    print("\n[1/7] Creating doc...")
    data, m = mcp_call("document_create", {"title": f"[EVAL-MCP] Atlas {RUN_ID}"})
    m.phase = "create_doc"
    br.calls.append(m)
    if not data or not isinstance(data, dict):
        br.errors.append(f"Doc creation failed: {m.error}")
        return br
    doc_uri = data.get("docUri")
    br.doc_id = doc_uri.split("/")[-1] if doc_uri else ""
    first_canvas = data.get("pages", [{}])[0].get("canvasUri")
    first_page = data.get("pages", [{}])[0].get("pageUri")
    print(f"  Doc: {br.doc_id}")
    time.sleep(2)

    # 2. Create pages
    print("\n[2/7] Creating 10 pages...")
    data, m = mcp_call("page_update", {
        "uri": first_page,
        "updateFields": {"title": PAGE_SPECS[0]["title"]}
    })
    m.phase = "create_pages"
    br.calls.append(m)

    page_canvases = [first_canvas]
    page_uris = [first_page]

    for i, spec in enumerate(PAGE_SPECS[1:], 1):
        data, m = mcp_call("page_create", {"uri": doc_uri, "title": spec["title"]})
        m.phase = "create_pages"
        br.calls.append(m)
        if data and isinstance(data, dict):
            page_canvases.append(data.get("canvasUri"))
            page_uris.append(data.get("pageUri"))
            br.pages_created += 1
        else:
            page_canvases.append(None)
            page_uris.append(None)
            br.errors.append(f"Page {spec['title']} failed: {m.error}")
        time.sleep(0.5)

    br.pages_created += 1
    print(f"  Created {br.pages_created} pages")

    # 3. Add content
    print("\n[3/7] Adding page content...")
    for i, spec in enumerate(PAGE_SPECS):
        canvas = page_canvases[i]
        if not canvas:
            continue
        data, m = mcp_call("content_modify", {
            "uri": canvas,
            "operations": [{"operation": "insert_element", "blockType": "markdown",
                            "content": spec["content"]}]
        })
        m.phase = "add_content"
        br.calls.append(m)
        if m.success:
            br.content_blocks += 1
        time.sleep(0.5)

    for page_idx, content in EXTRA_CONTENT.items():
        canvas = page_canvases[page_idx] if page_idx < len(page_canvases) else None
        if not canvas:
            continue
        data, m = mcp_call("content_modify", {
            "uri": canvas,
            "operations": [{"operation": "insert_element", "blockType": "markdown",
                            "content": content}]
        })
        m.phase = "add_content_extra"
        br.calls.append(m)
        if m.success:
            br.content_blocks += 1
        time.sleep(0.3)

    print(f"  Added {br.content_blocks} content blocks")

    # 4. Create tables
    print("\n[4/7] Creating tables...")
    table_uris = {}
    table_col_ids = {}

    for spec in TABLE_SPECS:
        canvas = page_canvases[spec["page_index"]]
        if not canvas:
            continue
        time.sleep(1)
        data, m = mcp_call("table_create", {
            "uri": canvas,
            "name": spec["name"],
            "columns": spec["columns"],
        })
        m.phase = "create_tables"
        br.calls.append(m)
        if data and isinstance(data, dict):
            tbl_uri = data.get("tableUri")
            col_ids = [c.get("columnId") or c.get("id") for c in data.get("columns", [])]
            table_uris[spec["name"]] = tbl_uri
            table_col_ids[spec["name"]] = col_ids
            br.tables_created += 1
            print(f"  {spec['name']}: {len(col_ids)} cols")
        else:
            br.errors.append(f"Table {spec['name']} failed: {m.error}")

    canvas = page_canvases[TASK_TABLE_SPEC["page_index"]]
    if canvas:
        time.sleep(1)
        data, m = mcp_call("table_create", {
            "uri": canvas,
            "name": TASK_TABLE_SPEC["name"],
            "columns": TASK_TABLE_SPEC["columns"],
        })
        m.phase = "create_tables"
        br.calls.append(m)
        if data and isinstance(data, dict):
            table_uris["All Tasks"] = data.get("tableUri")
            table_col_ids["All Tasks"] = [c.get("columnId") or c.get("id") for c in data.get("columns", [])]
            br.tables_created += 1
            print(f"  All Tasks: {len(table_col_ids['All Tasks'])} cols")

    print(f"  Created {br.tables_created} tables")

    # 5. Insert rows
    print("\n[5/7] Inserting rows...")
    for spec in TABLE_SPECS:
        tbl_uri = table_uris.get(spec["name"])
        col_ids = table_col_ids.get(spec["name"])
        if not tbl_uri or not col_ids:
            continue
        time.sleep(2)
        data, m = mcp_call("table_add_rows", {
            "uri": tbl_uri,
            "columns": col_ids,
            "rows": spec["rows"],
        })
        m.phase = "insert_rows"
        br.calls.append(m)
        if m.success:
            br.rows_inserted += len(spec["rows"])
            print(f"  {spec['name']}: +{len(spec['rows'])} rows")
        else:
            br.errors.append(f"Rows for {spec['name']} failed: {m.error}")

    task_rows = generate_tasks()
    tbl_uri = table_uris.get("All Tasks")
    col_ids = table_col_ids.get("All Tasks")
    if tbl_uri and col_ids:
        time.sleep(2)
        data, m = mcp_call("table_add_rows", {
            "uri": tbl_uri,
            "columns": col_ids,
            "rows": task_rows,
        })
        m.phase = "insert_tasks"
        br.calls.append(m)
        if m.success:
            br.rows_inserted += len(task_rows)
            print(f"  All Tasks: +{len(task_rows)} rows")
        else:
            br.errors.append(f"Task insert failed: {m.error}")

    print(f"  Total rows inserted: {br.rows_inserted}")

    # 6. Verify
    print("\n[6/7] Verifying doc structure...")
    data, m = mcp_call("document_read", {"uri": doc_uri})
    m.phase = "verify"
    br.calls.append(m)
    if data and isinstance(data, dict):
        actual_pages = len(data.get("pages", []))
        print(f"  Pages in doc: {actual_pages}")

    # 7. Verify rows
    print("\n[7/7] Verifying row counts...")
    for tbl_name in ["Team Directory", "All Tasks"]:
        tbl_uri = table_uris.get(tbl_name)
        if not tbl_uri:
            continue
        time.sleep(1)
        data, m = mcp_call("table_read_rows", {"uri": tbl_uri})
        m.phase = "verify_rows"
        br.calls.append(m)
        if data and isinstance(data, dict):
            items = data.get("items", data.get("rows", []))
            print(f"  {tbl_name}: {len(items)} rows verified")

    print(f"\n  Doc URL: https://coda.io/d/_d{br.doc_id}")
    return br


# ---------------------------------------------------------------------------
# Report
# ---------------------------------------------------------------------------

def phase_summary(br):
    """Group calls by phase and return summary dict."""
    phases = {}
    for c in br.calls:
        p = c.phase or "unknown"
        if p not in phases:
            phases[p] = {"calls": 0, "req": 0, "resp": 0, "ms": 0, "errors": 0}
        phases[p]["calls"] += 1
        phases[p]["req"] += c.request_bytes
        phases[p]["resp"] += c.response_bytes
        phases[p]["ms"] += c.duration_ms
        if not c.success:
            phases[p]["errors"] += 1
    return phases


def generate_report(cli_br, pick_br, mcp_br):
    lines = []
    lines.append("=" * 95)
    lines.append("  DOC BUILD BENCHMARK — SHD CLI vs CLI+pick vs Coda MCP")
    lines.append("=" * 95)
    lines.append("")

    all_brs = [cli_br, pick_br, mcp_br]
    labels = ["CLI", "CLI+pick", "MCP"]

    # Summary table
    lines.append(f"{'Metric':<26} {'CLI':>15} {'CLI+pick':>15} {'MCP':>15}  {'MCP/pick':>8}")
    lines.append("-" * 95)

    def row(name, vals, fmt="d", suffix=""):
        if fmt == "d":
            cells = [f"{v:>14}{suffix}" for v in vals]
        elif fmt == "f":
            cells = [f"{v:>14.1f}{suffix}" for v in vals]
        else:
            cells = [f"{v:>15}" for v in vals]
        ratio = vals[2] / max(vals[1], 1) if vals[1] else 0
        lines.append(f"{name:<26} {cells[0]} {cells[1]} {cells[2]}  {ratio:>7.1f}x")

    row("Total tool calls", [b.total_calls for b in all_brs], fmt="s")
    row("Request bytes", [b.total_request_bytes for b in all_brs], suffix="b")
    row("Response bytes", [b.total_response_bytes for b in all_brs], suffix="b")
    totals = [b.total_request_bytes + b.total_response_bytes for b in all_brs]
    row("Total bytes (req+resp)", totals, suffix="b")
    row("Est. tokens (bytes/4)", [int(b.total_tokens_est) for b in all_brs])
    row("Wall-clock time", [b.total_duration_ms / 1000 for b in all_brs], fmt="f", suffix="s")

    # Counts (no ratio)
    for label, attr in [("Pages created", "pages_created"), ("Tables created", "tables_created"),
                        ("Rows inserted", "rows_inserted"), ("Content blocks", "content_blocks")]:
        vals = [getattr(b, attr) for b in all_brs]
        lines.append(f"{label:<26} {vals[0]:>15} {vals[1]:>15} {vals[2]:>15}  {'—':>8}")
    err_vals = [len(b.errors) for b in all_brs]
    lines.append(f"{'Errors':<26} {err_vals[0]:>15} {err_vals[1]:>15} {err_vals[2]:>15}  {'—':>8}")
    lines.append("")

    # Phase breakdown — three-way
    lines.append("PHASE BREAKDOWN:")
    lines.append(f"{'Phase':<20} {'CLI KB':>9} {'pick KB':>9} {'MCP KB':>9}  {'MCP/pick':>8} {'pick savings':>13}")
    lines.append("-" * 95)

    phase_maps = [phase_summary(b) for b in all_brs]
    all_phases = sorted(set(sum([list(p.keys()) for p in phase_maps], [])))

    for phase in all_phases:
        kbs = []
        for pm in phase_maps:
            p = pm.get(phase, {"req": 0, "resp": 0})
            kbs.append((p["req"] + p["resp"]) / 1024)
        ratio = kbs[2] / max(kbs[1], 0.001)
        savings = (1 - kbs[1] / max(kbs[0], 0.001)) * 100 if kbs[0] > 0 else 0
        lines.append(f"{phase:<20} {kbs[0]:>8.1f}k {kbs[1]:>8.1f}k {kbs[2]:>8.1f}k  {ratio:>7.1f}x {savings:>11.0f}%")

    lines.append("")

    # Token cost analysis
    lines.append("TOKEN COST ANALYSIS (for agentic workflows):")
    for label, br in zip(labels, all_brs):
        lines.append(f"  {label:<12} ~{int(br.total_tokens_est):>6,} tokens")
    lines.append("")

    pick_tokens = pick_br.total_tokens_est
    mcp_tokens = mcp_br.total_tokens_est
    cli_tokens = cli_br.total_tokens_est
    lines.append(f"  CLI+pick saves {int(cli_tokens - pick_tokens):,} tokens vs plain CLI ({(1 - pick_tokens/max(cli_tokens,1))*100:.0f}% reduction)")
    lines.append(f"  MCP costs {int(mcp_tokens - pick_tokens):,} more tokens than CLI+pick ({mcp_tokens/max(pick_tokens,1):.1f}x)")
    lines.append("")

    lines.append("  Where --pick helps most:")
    lines.append("    - table_create: only need tableUri + column IDs, not full schema echo")
    lines.append("    - page_create: only need canvasUri + pageUri, not author/icon/metadata")
    lines.append("    - table_add_rows: only need rowCount confirmation, not full row echo")
    lines.append("    - verify phases: pick specific fields instead of reading entire doc/table")
    lines.append("")

    # Errors
    for label, br in zip(labels, all_brs):
        if br.errors:
            lines.append(f"ERRORS ({label}):")
            for e in br.errors:
                lines.append(f"  {e}")
            lines.append("")

    # Links
    lines.append("DOCUMENTS:")
    for label, br in zip(labels, all_brs):
        lines.append(f"  {label}: https://coda.io/d/_d{br.doc_id}")
    lines.append("")

    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    print("SHD CLI vs CLI+pick vs Coda MCP — Doc Build Benchmark")
    print(f"Run ID: {RUN_ID}")

    # Build all three paths with cooldowns between doc creates
    cli_result = build_via_cli()
    time.sleep(5)
    pick_result = build_via_cli_optimized()
    time.sleep(5)
    mcp_result = build_via_mcp()

    # Report
    report = generate_report(cli_result, pick_result, mcp_result)
    print(f"\n{report}")

    # Save
    report_path = "eval/doc_build_report.txt"
    with open(report_path, "w") as f:
        f.write(report)
    print(f"Report saved to {report_path}")

    data_path = "eval/doc_build_data.json"
    with open(data_path, "w") as f:
        json.dump({
            "run_id": RUN_ID,
            "cli": asdict(cli_result),
            "cli_pick": asdict(pick_result),
            "mcp": asdict(mcp_result),
        }, f, indent=2, default=str)
    print(f"Raw data saved to {data_path}")


if __name__ == "__main__":
    main()
