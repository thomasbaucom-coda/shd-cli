/// Registry of known Coda tools with descriptions and required fields.
/// Shared between the MCP server and the discover command.
/// This list bootstraps discovery — tools not listed here still work
/// via dynamic dispatch, they just won't appear in `coda discover`
/// or `coda mcp tools/list` until added here.
pub const TOOLS: &[Tool] = &[
    Tool { name: "whoami", description: "Get info about the authenticated user", required: &[] },
    Tool { name: "document_create", description: "Create a new Coda doc", required: &[("title", "string")] },
    Tool { name: "document_delete", description: "Delete a doc (DESTRUCTIVE)", required: &[("docId", "string")] },
    Tool { name: "document_read", description: "Read full document structure", required: &[("docId", "string")] },
    Tool { name: "search", description: "Search across docs for pages, tables, and rows", required: &[("query", "string")] },
    Tool { name: "url_decode", description: "Decode a Coda URL into resource IDs", required: &[("url", "string")] },
    Tool { name: "tool_guide", description: "Get usage guidance for a topic", required: &[("topic", "string")] },
    Tool { name: "page_create", description: "Create a new page", required: &[("docId", "string"), ("title", "string")] },
    Tool { name: "page_read", description: "Read page content and metadata", required: &[("docId", "string")] },
    Tool { name: "page_update", description: "Update page properties", required: &[("docId", "string"), ("pageId", "string"), ("updateFields", "object")] },
    Tool { name: "page_delete", description: "Delete a page (DESTRUCTIVE)", required: &[("docId", "string"), ("pageId", "string")] },
    Tool { name: "page_duplicate", description: "Duplicate a page with all content", required: &[("docId", "string"), ("pageId", "string")] },
    Tool { name: "table_create", description: "Create a table with typed columns", required: &[("docId", "string"), ("canvasId", "string"), ("name", "string"), ("columns", "array")] },
    Tool { name: "table_add_rows", description: "Add rows to a table (bulk)", required: &[("docId", "string"), ("tableId", "string"), ("columns", "array"), ("rows", "array")] },
    Tool { name: "table_add_columns", description: "Add columns to a table", required: &[("docId", "string"), ("tableId", "string"), ("columns", "array")] },
    Tool { name: "table_read_rows", description: "Read rows from a table", required: &[("docId", "string"), ("tableId", "string")] },
    Tool { name: "table_delete", description: "Delete a table (DESTRUCTIVE)", required: &[("docId", "string"), ("tableId", "string")] },
    Tool { name: "table_delete_rows", description: "Delete rows from a table", required: &[("docId", "string"), ("tableId", "string"), ("data", "object")] },
    Tool { name: "table_delete_columns", description: "Delete columns from a table", required: &[("docId", "string"), ("tableId", "string"), ("columnIds", "array")] },
    Tool { name: "table_update_rows", description: "Update rows in a table", required: &[("docId", "string"), ("tableId", "string"), ("rows", "array")] },
    Tool { name: "table_update_columns", description: "Update column properties", required: &[("docId", "string"), ("tableId", "string"), ("columns", "array")] },
    Tool { name: "table_view_configure", description: "Configure view: filter, layout, name", required: &[("docId", "string"), ("tableId", "string"), ("tableViewId", "string")] },
    Tool { name: "content_modify", description: "Write page content: markdown, callouts, code", required: &[("docId", "string"), ("canvasId", "string"), ("operations", "array")] },
    Tool { name: "content_image_upload", description: "Upload an image to a page", required: &[("docId", "string"), ("blobId", "string"), ("imageUrl", "string")] },
    Tool { name: "comment_manage", description: "Add, reply to, or delete comments", required: &[("docId", "string"), ("data", "object")] },
    Tool { name: "formula_create", description: "Create a named formula", required: &[("docId", "string"), ("canvasId", "string"), ("formula", "string")] },
    Tool { name: "formula_execute", description: "Evaluate a CFL expression", required: &[("docId", "string"), ("formula", "string")] },
    Tool { name: "formula_update", description: "Update a formula", required: &[("docId", "string"), ("formulaId", "string"), ("updatedFields", "object")] },
    Tool { name: "formula_delete", description: "Delete a formula", required: &[("docId", "string"), ("formulaId", "string")] },
];

pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub required: &'static [(&'static str, &'static str)], // (field_name, type)
}
