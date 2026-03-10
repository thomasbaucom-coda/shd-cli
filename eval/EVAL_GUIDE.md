# SHD CLI vs Coda MCP — Side-by-Side Evaluation Guide

Run these identical tasks in two Claude Code sessions to compare token usage,
quality, and ease of use for agentic workflows.

**Throwaway doc:** `YHhqsVvh-d`
**Doc URI:** `coda://docs/YHhqsVvh-d`
**Canvas URI:** `coda://docs/YHhqsVvh-d/canvases/canvas-dHqBZew8pT`

---

## Setup

### Session A: SHD CLI path

Open a Claude Code session with the `shd` binary available. The agent uses
Bash tool calls to invoke `shd <tool> --json '...'`.

### Session B: Coda MCP path

Open a Claude Code session with Coda's MCP server connected (either via
`shd mcp` as a stdio transport, or Coda's hosted MCP endpoint). The agent
uses MCP tool calls directly.

---

## Task 1: Discovery

**Goal:** Find all tools related to "rows".

### CLI prompt
```
Using the shd CLI, discover all available tools, then find the ones related
to table rows. Use `shd discover --filter row` to narrow results. Report
the tool names and their required fields.
```

### MCP prompt
```
Using the Coda MCP tools, list all available tools (tools/list), then identify
which ones are related to table rows. Report the tool names and their required
fields.
```

### What to measure
- How many tool calls / Bash calls did the agent make?
- How large were the responses? (CLI: `--filter` reduces output; MCP: full list)
- Did the agent correctly identify `table_read_rows`, `table_add_rows`,
  `table_update_rows`, `table_delete_rows`?

---

## Task 2: Read

**Goal:** Read the doc structure and list its pages.

### CLI prompt
```
Use `shd document_read --json '{"uri":"coda://docs/YHhqsVvh-d"}'` to get the
doc structure. List the page titles and their URIs.
```

### MCP prompt
```
Call the document_read tool with uri "coda://docs/YHhqsVvh-d". List the page
titles and their URIs.
```

### What to measure
- Response size (CLI returns raw JSON; MCP wraps in content[].text)
- Did the agent correctly parse and present the page list?

---

## Task 3: Write

**Goal:** Create a new page with a heading and paragraph.

### CLI prompt
```
Using shd, create a new page titled "Eval Write Test" in doc
coda://docs/YHhqsVvh-d. Then add a heading "Test Results" and a paragraph
"This page was created by the CLI evaluation." using content_modify with
operation "insert_element", blockType "markdown".
```

### MCP prompt
```
Using the Coda MCP tools, create a new page titled "Eval Write Test" in doc
coda://docs/YHhqsVvh-d. Then add a heading "Test Results" and a paragraph
"This page was created by the MCP evaluation." using content_modify with
operation "insert_element", blockType "markdown".
```

### What to measure
- Number of calls (both should be 2: page_create + content_modify)
- Did the agent correctly chain the page URI from create → modify?
- Error handling if the content_modify needs a canvas URI vs page URI

---

## Task 4: Multi-step (Table CRUD)

**Goal:** Create a table, insert 5 rows, read them back.

### CLI prompt
```
Using shd:
1. Create a table called "Eval Scores" on canvas
   coda://docs/YHhqsVvh-d/canvases/canvas-dHqBZew8pT with columns:
   Name (text), Score (number), Passed (checkbox)
2. Wait 2 seconds, then insert 5 rows with test data
3. Wait 2 seconds, then read the rows back and confirm all 5 are present
Use --pick where possible to minimize output.
```

### MCP prompt
```
Using the Coda MCP tools:
1. Create a table called "Eval Scores" on canvas
   coda://docs/YHhqsVvh-d/canvases/canvas-dHqBZew8pT with columns:
   Name (text), Score (number), Passed (checkbox)
2. Wait 2 seconds, then insert 5 rows with test data
3. Wait 2 seconds, then read the rows back and confirm all 5 are present
```

### What to measure
- Total bytes in/out across all calls
- Did the agent handle column IDs correctly (extract from create → use in add_rows)?
- CLI `--pick tableUri` vs MCP full response parsing for chaining
- Final row count verification

---

## Task 5: Error Recovery

**Goal:** Make an intentional error, observe the message, and fix it.

### CLI prompt
```
Using shd, try to create a page without providing a title:
  shd page_create --json '{"uri":"coda://docs/YHhqsVvh-d"}'
Read the error message, then fix the call by adding a title and retry.
```

### MCP prompt
```
Using the Coda MCP tools, try calling page_create with only
{"uri":"coda://docs/YHhqsVvh-d"} (no title). Read the error, then fix
the call by adding a title and retry.
```

### What to measure
- Error message quality: Does it say which field is missing? Does it include
  the field type and description?
- CLI validates client-side (no network round-trip for the error) vs MCP
  which may need a server round-trip
- Recovery: Did the agent correctly fix and retry in one step?

---

## Scoring Rubric

For each task, score both paths on:

| Dimension | 1 (Poor) | 3 (Adequate) | 5 (Excellent) |
|-----------|----------|--------------|---------------|
| **Token efficiency** | Full response parsed, no filtering | Some filtering | `--pick`/`--filter` used, minimal tokens |
| **Quality** | Wrong result or missed data | Correct but verbose | Correct, concise, well-formatted |
| **Ease of use** | Multiple retries, confusion | Straightforward but manual | Single call or obvious chaining |

### Token counting method

For CLI sessions, estimate tokens from the Bash tool output sizes.
For MCP sessions, count the `content[].text` field sizes in tool results.
Rough conversion: **1 token ≈ 4 bytes** for JSON/English text.

---

## Running the Automated Harness

For an automated version of this evaluation:

```bash
# With the throwaway doc
python3 eval/eval_harness.py --doc-id YHhqsVvh-d

# Or let it create its own doc
python3 eval/eval_harness.py
```

This runs all 5 tasks through both paths and produces `eval/eval_report.txt`
with side-by-side metrics plus `eval/eval_data.json` with raw call data.
