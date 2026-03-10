#!/usr/bin/env python3
"""Build the same Project Atlas doc via MCP, measuring real token costs."""

import json
import subprocess
import time
import sys

CLI = "./target/release/shd"

calls = []  # (phase, tool, req_bytes, resp_bytes, duration_ms)

def mcp_call(tool, arguments=None):
    """Call tool via MCP server, return (data, req_bytes, resp_bytes, ms)."""
    rpc = json.dumps({
        "jsonrpc": "2.0", "id": 1,
        "method": "tools/call",
        "params": {"name": tool, "arguments": arguments or {}}
    })
    init = json.dumps({
        "jsonrpc": "2.0", "id": 0,
        "method": "initialize",
        "params": {"protocolVersion": "2024-11-05",
                    "clientInfo": {"name": "live-build", "version": "0.1"}}
    })
    notif = json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"})
    stdin_data = f"{init}\n{notif}\n{rpc}\n"
    req_bytes = len(rpc.encode())

    start = time.monotonic()
    proc = subprocess.run([CLI, "mcp"], capture_output=True, text=True,
                          input=stdin_data, timeout=60)
    elapsed_ms = int((time.monotonic() - start) * 1000)

    resp_lines = [l for l in proc.stdout.strip().split("\n") if l.strip()]
    resp_bytes = sum(len(l.encode()) for l in resp_lines)

    for line in resp_lines:
        try:
            msg = json.loads(line)
            if msg.get("id") == 1:
                if "error" in msg:
                    return None, req_bytes, resp_bytes, elapsed_ms
                result = msg.get("result", {})
                content = result.get("content", [])
                if content and content[0].get("text"):
                    try:
                        return json.loads(content[0]["text"]), req_bytes, resp_bytes, elapsed_ms
                    except json.JSONDecodeError:
                        return content[0]["text"], req_bytes, resp_bytes, elapsed_ms
                return result, req_bytes, resp_bytes, elapsed_ms
        except json.JSONDecodeError:
            continue

    return None, req_bytes, resp_bytes, elapsed_ms


def track(phase, tool, arguments=None):
    data, rb, rpb, ms = mcp_call(tool, arguments)
    calls.append((phase, tool, rb, rpb, ms))
    if data is None:
        print(f"  ERROR on {tool}")
    return data


# ============================================================
# BUILD
# ============================================================

print("Building Project Atlas doc via MCP...\n")

# 1. Create doc
print("[1/6] Creating doc...")
data = track("create_doc", "document_create", {"title": "[MCP] Project Atlas — AI Mobile Tool"})
if not data:
    print("FATAL: doc creation failed")
    sys.exit(1)
doc_uri = data.get("docUri")
first_canvas = data.get("pages", [{}])[0].get("canvasUri")
first_page = data.get("pages", [{}])[0].get("pageUri")
doc_id = doc_uri.split("/")[-1]
print(f"  Doc: {doc_id}")
time.sleep(2)

# 2. Pages
print("\n[2/6] Creating 10 pages...")
track("create_pages", "page_update", {
    "uri": first_page,
    "updateFields": {"title": "Overview", "subtitle": "Project Atlas — AI-Powered Mobile Companion"}
})

PAGE_TITLES = [
    "Research & Discovery",
    "PRD: On-Device AI Engine",
    "PRD: Conversational UX",
    "PRD: Privacy & Edge Computing",
    "PRD: Multimodal Input",
    "Tasks",
    "Team",
    "Project Metrics",
    "Meeting Notes & Decisions",
]

page_canvases = [first_canvas]
for title in PAGE_TITLES:
    data = track("create_pages", "page_create", {"uri": doc_uri, "title": title})
    if data and isinstance(data, dict):
        page_canvases.append(data.get("canvasUri"))
    else:
        page_canvases.append(None)
    time.sleep(0.5)

print(f"  Created {len(page_canvases)} pages")

# 3. Content
print("\n[3/6] Adding page content...")

CONTENT = {
    0: "# Project Atlas\n\n**Mission:** Build the most advanced AI-powered mobile companion that runs primarily on-device, delivering real-time intelligence while preserving user privacy.\n\n**Status:** Active — Pre-Alpha\n**Target Launch:** Q4 2026\n**Executive Sponsor:** VP of Product\n**Program Lead:** Sarah Chen\n\n## Strategic Context\n\nThe mobile AI market is projected to reach $80B by 2028. Current solutions rely heavily on cloud inference, creating latency and privacy concerns. Project Atlas differentiates by running 90%+ of inference on-device using optimized transformer models.\n\n## Key Bets\n\n1. **On-device inference** — Sub-100ms response times via quantized models\n2. **Multimodal input** — Voice, camera, screen context, and gesture\n3. **Privacy-first architecture** — No user data leaves the device by default\n4. **Conversational UX** — Persistent context across sessions, proactive suggestions",

    1: "# Research & Discovery\n\n## Competitive Landscape\n\n| Competitor | Approach | On-Device % | Latency (p50) | Privacy Model |\n|---|---|---|---|---|\n| Google Gemini Nano | Hybrid cloud/edge | ~40% | 200ms | Data shared with Google |\n| Apple Intelligence | On-device first | ~70% | 150ms | On-device, opt-in cloud |\n| Samsung Galaxy AI | Cloud-primary | ~20% | 350ms | Data processed in cloud |\n| **Project Atlas** | **Edge-native** | **90%+** | **<100ms** | **Zero data egress** |\n\n## User Research Findings (n=2,400)\n\n- **78%** of users concerned about AI privacy on mobile\n- **65%** would pay premium for on-device AI\n- **82%** want AI that works offline\n- **71%** frustrated by cloud AI latency\n\n## Technical Feasibility\n\n- **Model size target:** 2B parameters (quantized to 4-bit = ~1GB on device)\n- **Hardware requirement:** 6GB+ RAM, NPU/GPU with 8+ TOPS\n- **Compatible devices:** ~340M phones worldwide",

    2: "# PRD: On-Device AI Engine\n\n**Owner:** Dr. Wei Zhang | **Status:** In Progress | **Target:** Alpha Q2 2026\n\n## Problem Statement\n\nExisting mobile AI solutions depend on cloud inference, resulting in 200-500ms latency, data privacy exposure, and no offline capability.\n\n## Goals\n\n1. Run 90%+ of inference on-device with <100ms p50 latency\n2. Support 2B parameter transformer model in <1GB memory\n3. Achieve GPT-3.5-level quality on core tasks\n4. Work fully offline for all core features\n\n## Technical Approach\n\n- Custom decoder-only transformer, 2B parameters\n- 4-bit GPTQ quantization with mixed-precision attention\n- Grouped-query attention (GQA) for memory efficiency\n- Sliding window attention (4096 tokens)\n- Custom C++ runtime with Metal (iOS) and Vulkan (Android)\n- NPU delegation via CoreML / NNAPI\n- Speculative decoding for 2x throughput",

    3: "# PRD: Conversational UX\n\n**Owner:** Maya Johnson | **Status:** In Progress | **Target:** Alpha Q2 2026\n\n## Problem Statement\n\nMobile AI assistants today are stateless. Users want an AI that remembers context, learns preferences, and proactively surfaces relevant information.\n\n## Goals\n\n1. Persistent conversation memory across sessions (on-device)\n2. Proactive suggestions based on user patterns\n3. Multi-turn dialogue with context window >20 turns\n4. Personality customization\n\n## Key Features\n\n- Conversation summaries in encrypted local DB\n- User preference graph (topics, contacts, routines)\n- Morning briefing: calendar, weather, commute, news\n- Multi-turn coherence with sliding context window\n- Emotional intelligence — detect frustration, adjust tone",

    4: "# PRD: Privacy & Edge Computing\n\n**Owner:** Carlos Mendez | **Status:** At Risk | **Target:** Alpha Q3 2026\n\n## Goals\n\n1. Zero data egress by default\n2. End-to-end encryption for optional cloud features\n3. User-controlled data retention with one-tap deletion\n4. Compliance with GDPR, CCPA, EU AI Act, SOC 2 Type II\n\n## Architecture\n\n- All user inputs processed locally via on-device model\n- Conversation memory encrypted with device-bound key\n- Optional cloud: E2E encryption with user-held keys\n- Telemetry: differential privacy, no PII\n\n## Risk\n\n**AT RISK:** SOC 2 Type II audit requires 6-month observation. Must begin audit prep immediately.",

    5: "# PRD: Multimodal Input\n\n**Owner:** Aisha Okafor | **Status:** On Track | **Target:** Beta Q3 2026\n\n## Supported Modalities\n\n### Voice\n- On-device Whisper-based ASR (multilingual)\n- Streaming transcription, <500ms latency\n- Custom wake word (not always-listening)\n\n### Vision\n- Camera scene understanding via on-device vision model\n- Screenshot/screen context extraction\n- Document scanning with OCR + layout understanding\n\n### Gesture\n- Shake to undo, long-press for context, swipe for quick actions\n\n| Modality | Model Size | Latency | Offline |\n|---|---|---|---|\n| Voice (ASR) | 150MB | <500ms | Yes |\n| Vision (scene) | 400MB | <1s | Yes |\n| Vision (OCR) | 80MB | <300ms | Yes |",

    6: "# Task Board\n\nAll tasks across workstreams. Filter by workstream, status, or sprint.\n\n**Current Sprint:** S1 (Mar 3-17, 2026)\n**Velocity:** 62 pts (target: 80)",

    7: "# Team\n\n14 members across 6 workstreams. Core team in San Francisco with distributed members in New York, Austin, London, Bangalore, and Seattle.",

    8: "# Project Metrics\n\nKey performance indicators tracked weekly. Red items require immediate escalation.\n\n**Last Updated:** Mar 9, 2026\n**Overall Health:** Yellow (2 metrics at risk)",

    9: "# Meeting Notes & Decisions\n\n## Week 1 — Mar 9, 2026\n\n**Attendees:** Sarah Chen, Wei Zhang, Maya Johnson, Carlos Mendez, Aisha Okafor, Raj Patel\n\n### Decisions\n- Model architecture: decoder-only transformer (2B params) — APPROVED\n- Privacy: zero-egress by default, opt-in cloud — APPROVED\n- Launch target: Q4 2026 internal beta, Q1 2027 public — APPROVED\n\n### Action Items\n- Wei: Deliver model architecture doc by Mar 15\n- Carlos: Begin SOC 2 audit prep immediately (AT RISK)\n- Aisha: Prototype camera integration on iOS by Mar 22\n- Raj: Set up CI/CD pipeline — DONE",
}

for i, content in CONTENT.items():
    canvas = page_canvases[i]
    if not canvas:
        continue
    track("add_content", "content_modify", {
        "uri": canvas,
        "operations": [{"operation": "insert_element", "blockType": "markdown", "content": content}]
    })
    time.sleep(0.5)

print(f"  Added {len(CONTENT)} content blocks")

# 4. Tables
print("\n[4/6] Creating tables...")

# Team Directory
time.sleep(1)
data = track("create_tables", "table_create", {
    "uri": page_canvases[7],
    "name": "Team Directory",
    "columns": [{"name":"Name"},{"name":"Role"},{"name":"Workstream"},{"name":"Focus Area"},{"name":"Location"}]
})
team_tbl = data.get("tableUri") if data else None
team_cols = [c.get("columnId") or c.get("id") for c in data.get("columns", [])] if data else []

# Metrics
time.sleep(1)
data = track("create_tables", "table_create", {
    "uri": page_canvases[8],
    "name": "Key Metrics",
    "columns": [{"name":"Metric"},{"name":"Target"},{"name":"Current"},{"name":"Status"},{"name":"Owner"},{"name":"Notes"}]
})
metrics_tbl = data.get("tableUri") if data else None
metrics_cols = [c.get("columnId") or c.get("id") for c in data.get("columns", [])] if data else []

# Research
time.sleep(1)
data = track("create_tables", "table_create", {
    "uri": page_canvases[1],
    "name": "Research Findings",
    "columns": [{"name":"Finding"},{"name":"Source"},{"name":"Impact"},{"name":"Action"},{"name":"Status"}]
})
research_tbl = data.get("tableUri") if data else None
research_cols = [c.get("columnId") or c.get("id") for c in data.get("columns", [])] if data else []

# Tasks
time.sleep(1)
data = track("create_tables", "table_create", {
    "uri": page_canvases[6],
    "name": "All Tasks",
    "columns": [{"name":"Task"},{"name":"Workstream"},{"name":"Priority"},{"name":"Status"},{"name":"Assignee"},{"name":"Sprint"},{"name":"Due Date"}]
})
tasks_tbl = data.get("tableUri") if data else None
tasks_cols = [c.get("columnId") or c.get("id") for c in data.get("columns", [])] if data else []

# PRD Tracker
time.sleep(1)
data = track("create_tables", "table_create", {
    "uri": page_canvases[0],
    "name": "PRD Tracker",
    "columns": [{"name":"PRD"},{"name":"Owner"},{"name":"Status"},{"name":"Target"},{"name":"Tasks"},{"name":"Risks"}]
})
prd_tbl = data.get("tableUri") if data else None
prd_cols = [c.get("columnId") or c.get("id") for c in data.get("columns", [])] if data else []

print(f"  Created 5 tables")

# 5. Insert rows
print("\n[5/6] Inserting rows...")

time.sleep(2)
if team_tbl:
    track("insert_rows", "table_add_rows", {
        "uri": team_tbl, "columns": team_cols,
        "rows": [
            ["Dr. Wei Zhang","Principal ML Engineer","AI Engine","Model architecture, quantization","San Francisco"],
            ["Sarah Chen","Program Lead","All","Cross-functional coordination","San Francisco"],
            ["Maya Johnson","Senior Product Designer","Conversational UX","Dialogue design, proactive UX","New York"],
            ["Carlos Mendez","Security Architect","Privacy & Edge","Encryption, compliance","Austin"],
            ["Aisha Okafor","Senior ML Engineer","Multimodal","Vision models, ASR","London"],
            ["Raj Patel","Staff Engineer","Platform","CI/CD, build systems","San Francisco"],
            ["Lisa Kim","QA Lead","QA & Perf","Performance benchmarks","Seattle"],
            ["Tom Harris","ML Engineer","AI Engine","Inference optimization","San Francisco"],
            ["Nina Volkov","UX Researcher","Conversational UX","User studies","Remote"],
            ["Jake Rivera","iOS Engineer","Platform","Metal backend, CoreML","San Francisco"],
            ["Priya Sharma","Android Engineer","Platform","Vulkan backend, NNAPI","Bangalore"],
            ["Marcus Lee","Data Scientist","AI Engine","Model evaluation","New York"],
            ["Emma Wilson","Product Manager","All","Roadmap, stakeholders","San Francisco"],
            ["David Park","Technical Writer","All","API docs, guides","Remote"],
        ]
    })
    print("  Team: +14 rows")

time.sleep(2)
if metrics_tbl:
    track("insert_rows", "table_add_rows", {
        "uri": metrics_tbl, "columns": metrics_cols,
        "rows": [
            ["Inference latency (p50)","<100ms","145ms","In Progress","Wei Zhang","Blocked on NPU delegation"],
            ["Model quality (MMLU)",">58%","55.2%","In Progress","Wei Zhang","Exploring MoE"],
            ["On-device memory","<1GB","1.3GB","At Risk","Tom Harris","KV-cache compression WIP"],
            ["Battery drain (1hr)","<8%","12%","At Risk","Raj Patel","Thermal throttle handling"],
            ["Offline capability","100% core","85%","In Progress","Carlos Mendez","Voice offline done"],
            ["ASR accuracy (WER)","<5%","6.2%","In Progress","Aisha Okafor","Fine-tuning"],
            ["Context recall (20 turns)",">90%","—","Not Started","Maya Johnson","Depends on memory system"],
            ["Compatible devices",">300M","~340M","On Track","Raj Patel","Flagship + mid-range"],
            ["App size","<500MB","—","Not Started","Raj Patel","Models on first run"],
            ["SOC 2 readiness","Q4 2026","Not started","At Risk","Carlos Mendez","Begin audit NOW"],
            ["Team velocity",">80 pts","62","In Progress","Sarah Chen","2 hires pending"],
            ["Bug escape rate","<2%","—","Not Started","Lisa Kim","QA framework setup"],
        ]
    })
    print("  Metrics: +12 rows")

time.sleep(2)
if research_tbl:
    track("insert_rows", "table_add_rows", {
        "uri": research_tbl, "columns": research_cols,
        "rows": [
            ["78% users concerned about AI privacy","User survey (n=2400)","High","Core positioning: privacy-first","Incorporated"],
            ["65% would pay premium for on-device AI","User survey","High","Premium pricing justified","Incorporated"],
            ["82% want AI that works offline","User survey","Critical","Offline-first confirmed","Incorporated"],
            ["Apple Intelligence sets 70% on-device bar","Competitive analysis","High","Target 90%+ to differentiate","Incorporated"],
            ["2B param models approach GPT-3.5","ML literature review","Critical","Validates model size","Incorporated"],
            ["NPU throughput 3-5x vs GPU on mobile","Hardware benchmarks","High","Prioritize NPU path","In Progress"],
            ["Users abandon after 3s wait","UX research","Critical","<100ms target validated","Incorporated"],
            ["Differential privacy adds <5% noise","Privacy engineering","Medium","Adopt for telemetry","Planned"],
            ["GQA reduces memory 40% vs MHA","ML literature","High","Adopted in architecture","Incorporated"],
            ["Wake word false positive <0.5/day","User interviews","Medium","Sets threshold","Planned"],
        ]
    })
    print("  Research: +10 rows")

time.sleep(2)
if tasks_tbl:
    track("insert_rows", "table_add_rows", {
        "uri": tasks_tbl, "columns": tasks_cols,
        "rows": [
            ["Implement GQA attention layer","AI Engine","Critical","In Progress","Wei Zhang","S1","2026-03-20"],
            ["4-bit GPTQ quantization pipeline","AI Engine","Critical","In Progress","Tom Harris","S1","2026-03-22"],
            ["Metal inference backend (iOS)","AI Engine","High","To Do","Jake Rivera","S2","2026-04-05"],
            ["Vulkan inference backend (Android)","AI Engine","High","To Do","Priya Sharma","S2","2026-04-05"],
            ["KV-cache compression","AI Engine","High","In Progress","Tom Harris","S1","2026-03-25"],
            ["Speculative decoding prototype","AI Engine","Medium","To Do","Wei Zhang","S3","2026-04-15"],
            ["MMLU benchmark harness","AI Engine","High","Done","Marcus Lee","S1","2026-03-10"],
            ["NPU delegation via CoreML","AI Engine","High","To Do","Jake Rivera","S3","2026-04-20"],
            ["NPU delegation via NNAPI","AI Engine","High","To Do","Priya Sharma","S3","2026-04-20"],
            ["Sliding window attention impl","AI Engine","Critical","In Progress","Wei Zhang","S1","2026-03-18"],
            ["Model training pipeline setup","AI Engine","High","Done","Marcus Lee","S1","2026-03-08"],
            ["Inference latency profiling","AI Engine","Medium","To Do","Tom Harris","S2","2026-04-01"],
            ["Conversation memory schema","Conversational UX","Critical","In Progress","Maya Johnson","S1","2026-03-20"],
            ["Encrypted local DB for context","Conversational UX","High","To Do","Carlos Mendez","S2","2026-04-01"],
            ["Multi-turn dialogue prototype","Conversational UX","High","In Progress","Maya Johnson","S1","2026-03-22"],
            ["Morning briefing feature spec","Conversational UX","Medium","To Do","Maya Johnson","S3","2026-04-15"],
            ["User preference graph design","Conversational UX","Medium","To Do","Nina Volkov","S2","2026-04-05"],
            ["Personality customization UI","Conversational UX","Low","To Do","Maya Johnson","S4","2026-05-01"],
            ["Conversation quality eval","Conversational UX","High","To Do","Nina Volkov","S2","2026-04-08"],
            ["Device-bound key management","Privacy & Edge","Critical","In Progress","Carlos Mendez","S1","2026-03-18"],
            ["E2E encryption for cloud fallback","Privacy & Edge","High","To Do","Carlos Mendez","S2","2026-04-10"],
            ["Differential privacy telemetry","Privacy & Edge","Medium","To Do","Carlos Mendez","S3","2026-04-20"],
            ["SOC 2 audit preparation","Privacy & Edge","Critical","Blocked","Carlos Mendez","S1","2026-03-15"],
            ["Data retention UI","Privacy & Edge","High","To Do","Maya Johnson","S3","2026-04-22"],
            ["Certificate pinning","Privacy & Edge","Medium","To Do","Carlos Mendez","S2","2026-04-05"],
            ["Whisper ASR on-device port","Multimodal","Critical","In Progress","Aisha Okafor","S1","2026-03-22"],
            ["Camera scene understanding","Multimodal","High","To Do","Aisha Okafor","S2","2026-04-10"],
            ["OCR + layout integration","Multimodal","Medium","To Do","Aisha Okafor","S3","2026-04-25"],
            ["Wake word detection prototype","Multimodal","High","To Do","Tom Harris","S2","2026-04-08"],
            ["Gesture recognition framework","Multimodal","Low","To Do","Jake Rivera","S4","2026-05-05"],
            ["CI/CD pipeline setup","Platform","Critical","Done","Raj Patel","S1","2026-03-05"],
            ["Device test farm provisioning","Platform","High","In Progress","Raj Patel","S1","2026-03-20"],
            ["Model download + update system","Platform","High","To Do","Raj Patel","S2","2026-04-05"],
            ["App size optimization","Platform","Medium","To Do","Raj Patel","S3","2026-04-20"],
            ["Crash reporting integration","Platform","Medium","To Do","Raj Patel","S2","2026-04-01"],
            ["Performance benchmark suite","QA & Perf","High","In Progress","Lisa Kim","S1","2026-03-22"],
            ["Battery drain test automation","QA & Perf","High","To Do","Lisa Kim","S2","2026-04-08"],
            ["Thermal throttle test scenarios","QA & Perf","Medium","To Do","Lisa Kim","S3","2026-04-20"],
            ["Model quality regression tests","QA & Perf","High","To Do","Marcus Lee","S2","2026-04-05"],
            ["E2E integration test harness","QA & Perf","Medium","To Do","Lisa Kim","S3","2026-04-22"],
        ]
    })
    print("  Tasks: +40 rows")

time.sleep(2)
if prd_tbl:
    track("insert_rows", "table_add_rows", {
        "uri": prd_tbl, "columns": prd_cols,
        "rows": [
            ["On-Device AI Engine","Dr. Wei Zhang","In Progress","Alpha Q2 2026","12 tasks (2 done)","Memory budget, battery drain"],
            ["Conversational UX","Maya Johnson","In Progress","Alpha Q2 2026","7 tasks (0 done)","Context recall accuracy"],
            ["Privacy & Edge Computing","Carlos Mendez","At Risk","Alpha Q3 2026","6 tasks (0 done, 1 blocked)","SOC 2 audit timeline"],
            ["Multimodal Input","Aisha Okafor","On Track","Beta Q3 2026","5 tasks (0 done)","Model size budget"],
        ]
    })
    print("  PRDs: +4 rows")

# 6. Verify
print("\n[6/6] Verifying...")
time.sleep(1)
data = track("verify", "document_read", {"uri": doc_uri})
if data and isinstance(data, dict):
    print(f"  Pages: {len(data.get('pages', []))}")

print(f"\n  Doc: https://coda.io/d/_d{doc_id}")

# ============================================================
# REPORT
# ============================================================

print("\n" + "=" * 70)
print("  MCP PATH — Actual Token Measurement")
print("=" * 70)

total_req = sum(c[2] for c in calls)
total_resp = sum(c[3] for c in calls)
total_bytes = total_req + total_resp
total_calls = len(calls)
total_ms = sum(c[4] for c in calls)

# Phase breakdown
phases = {}
for phase, tool, rb, rpb, ms in calls:
    if phase not in phases:
        phases[phase] = {"calls": 0, "req": 0, "resp": 0, "ms": 0}
    phases[phase]["calls"] += 1
    phases[phase]["req"] += rb
    phases[phase]["resp"] += rpb
    phases[phase]["ms"] += ms

print(f"\nTotal calls: {total_calls}")
print(f"Total time: {total_ms/1000:.1f}s")
print()
print(f"{'Phase':<20} {'Calls':>6} {'Req KB':>9} {'Resp KB':>9} {'Total KB':>9}")
print("-" * 60)
for phase, d in phases.items():
    req_kb = d["req"] / 1024
    resp_kb = d["resp"] / 1024
    total_kb = (d["req"] + d["resp"]) / 1024
    print(f"{phase:<20} {d['calls']:>6} {req_kb:>8.1f}k {resp_kb:>8.1f}k {total_kb:>8.1f}k")

print("-" * 60)
print(f"{'TOTAL':<20} {total_calls:>6} {total_req/1024:>8.1f}k {total_resp/1024:>8.1f}k {total_bytes/1024:>8.1f}k")
print()
print(f"Est. tokens (bytes/4): ~{total_bytes // 4:,}")
print()

# Save raw data
with open("eval/live_mcp_data.json", "w") as f:
    json.dump({
        "calls": [{"phase": c[0], "tool": c[1], "req_bytes": c[2], "resp_bytes": c[3], "duration_ms": c[4]} for c in calls],
        "total_req_bytes": total_req,
        "total_resp_bytes": total_resp,
        "total_calls": total_calls,
        "est_tokens": total_bytes // 4,
    }, f, indent=2)
print("Raw data saved to eval/live_mcp_data.json")
