//! Sync command — materialize Coda documents to a local filesystem tree.
//!
//! This enables AI agents to read Coda data from disk using standard file
//! tools (Read, Glob, Grep) instead of orchestrating API calls.
//!
//! Filesystem layout:
//! ```text
//! <root>/
//! ├── .gitignore
//! ├── .sync_manifest.json
//! └── docs/
//!     └── <doc-slug>/
//!         ├── __doc.json
//!         ├── __sync.json
//!         ├── CONTEXT.md
//!         └── pages/
//!             ├── <page-slug>.md
//!             ├── <page-slug>.json
//!             └── tables/
//!                 └── <table-slug>/
//!                     ├── __schema.json
//!                     └── rows.ndjson
//! ```

use crate::cell;
use crate::client::ToolCaller;
use crate::error::{CodaError, Result};
use crate::output;
use crate::slug;
use crate::trace;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Public options & entry point
// ---------------------------------------------------------------------------

pub struct SyncOpts {
    pub doc_uri: String,
    pub root: PathBuf,
    pub force: bool,
    pub dry_run: bool,
    pub tables_only: bool,
    pub max_rows: usize,
}

pub async fn run(client: &dyn ToolCaller, opts: SyncOpts) -> Result<()> {
    let root = &opts.root;

    if opts.dry_run {
        output::info(&format!(
            "[sync] Dry run: would sync {} to {}\n",
            opts.doc_uri,
            root.display()
        ));
        return dry_run_preview(client, &opts).await;
    }

    // Ensure root/docs exists
    let docs_dir = root.join("docs");
    std::fs::create_dir_all(&docs_dir)?;

    // Auto-create .gitignore to prevent committing synced data
    let gitignore_path = root.join(".gitignore");
    if !gitignore_path.exists() {
        std::fs::write(&gitignore_path, "# Synced Coda data — do not commit\n*\n")?;
    }

    // Load or create manifest
    let mut manifest = load_manifest(root)?;

    let stats = sync_document(client, &opts, &docs_dir, &mut manifest).await?;

    // Save manifest and generate INDEX.md
    save_manifest(root, &manifest)?;
    write_index_md(root, &manifest)?;

    // Print JSON result to stdout
    let summary = json!({
        "synced": true,
        "docUri": stats.doc_uri,
        "root": root.display().to_string(),
        "docDir": stats.doc_dir.display().to_string(),
        "pages": stats.pages_synced,
        "tables": stats.tables_synced,
        "rows": stats.rows_synced,
        "errors": stats.errors,
    });
    output::print_response(&summary, crate::output::OutputFormat::Json)?;

    // Print actionable summary to stderr
    let root_rel = root.display();
    output::info(&format!(
        "\n--- Sync complete ---\n\
         Doc:    {title}\n\
         Pages:  {pages} synced\n\
         Tables: {tables} synced ({rows} rows)\n\
         Path:   {path}\n\
         \n\
         Add to your CLAUDE.md:\n\
         \n\
           ## Coda Data\n\
           - Start with `{root_rel}/INDEX.md` to see all synced docs, then read `{root_rel}/docs/*/CONTEXT.md` for details.\n\
           - Not every Coda doc has been synced. If the doc you need is missing, run `shd sync --doc-url \"<url>\"` first.\n\
           - To create a new doc from scratch: `shd doc_scaffold --json '{{...}}' --sync`\n\
           - To create a doc based on an existing one: read the relevant CONTEXT.md first, then scaffold with --sync\n\
           - The --sync flag auto-syncs created docs to `{root_rel}/` so CONTEXT.md is available for next time\n\
         \n\
         Tip: {root_rel}/.gitignore already excludes synced data from git.\n",
        title = stats.doc_title,
        pages = stats.pages_synced,
        tables = stats.tables_synced,
        rows = stats.rows_synced,
        path = stats.doc_dir.display(),
    ));

    Ok(())
}

// ---------------------------------------------------------------------------
// Manifest & sync state types
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default, Clone)]
struct SyncManifest {
    version: u32,
    synced_at: String,
    docs: HashMap<String, ManifestDocEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
struct ManifestDocEntry {
    slug: String,
    title: String,
    synced_at: String,
    page_count: usize,
    table_count: usize,
    #[serde(default = "default_status")]
    status: String,
}

fn default_status() -> String {
    "complete".to_string()
}

#[derive(Serialize, Deserialize, Default)]
struct DocSync {
    version: u32,
    doc_uri: String,
    synced_at: String,
    pages: HashMap<String, PageSyncEntry>,
    tables: HashMap<String, TableSyncEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
struct PageSyncEntry {
    slug: String,
    synced_at: String,
    tables: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct TableSyncEntry {
    slug: String,
    row_count: usize,
    synced_at: String,
}

// ---------------------------------------------------------------------------
// Detail types for CONTEXT.md generation
// ---------------------------------------------------------------------------

struct PageDetail {
    slug: String,
    title: String,
    has_content: bool,
}

struct TableDetail {
    slug: String,
    name: String,
    column_names: Vec<String>,
    row_count: usize,
}

struct SyncStats {
    pages_synced: usize,
    tables_synced: usize,
    rows_synced: usize,
    errors: Vec<String>,
    doc_title: String,
    doc_uri: String,
    doc_dir: PathBuf,
}

// ---------------------------------------------------------------------------
// Atomic sync helpers
// ---------------------------------------------------------------------------

/// Three-phase directory promotion: backup existing, move tmp to final, clean up backup.
/// On failure to move tmp, restores from backup so the old data is preserved.
fn promote_sync_dir(tmp_dir: &Path, final_dir: &Path) -> crate::error::Result<()> {
    let backup_dir = tmp_dir.parent().unwrap().join(format!(
        "{}_old",
        tmp_dir.file_name().unwrap().to_string_lossy()
    ));

    // Phase 1: Move existing to backup
    if final_dir.exists() {
        std::fs::rename(final_dir, &backup_dir).map_err(|e| {
            crate::error::CodaError::Other(format!("Failed to backup existing dir: {e}"))
        })?;
    }

    // Phase 2: Move tmp to final
    std::fs::create_dir_all(final_dir.parent().unwrap())?;
    if let Err(e) = std::fs::rename(tmp_dir, final_dir) {
        // Restore backup on failure
        if backup_dir.exists() {
            let _ = std::fs::rename(&backup_dir, final_dir);
        }
        return Err(crate::error::CodaError::Other(format!(
            "Failed to promote sync dir: {e}"
        )));
    }

    // Phase 3: Clean up backup
    if backup_dir.exists() {
        let _ = std::fs::remove_dir_all(&backup_dir);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Core sync logic
// ---------------------------------------------------------------------------

async fn sync_document(
    client: &dyn ToolCaller,
    opts: &SyncOpts,
    docs_dir: &Path,
    manifest: &mut SyncManifest,
) -> Result<SyncStats> {
    let mut stats = SyncStats {
        pages_synced: 0,
        tables_synced: 0,
        rows_synced: 0,
        errors: Vec::new(),
        doc_title: String::new(),
        doc_uri: opts.doc_uri.clone(),
        doc_dir: PathBuf::new(),
    };

    // Step 1: Read document structure
    output::info(&format!("[sync] Reading document: {}\n", opts.doc_uri));
    trace::emit_compound_step("sync", 1, "document_read", &json!({"uri": &opts.doc_uri}));

    let doc_result = call_with_retry(
        client,
        "document_read",
        json!({
            "uri": &opts.doc_uri,
            "contentTypesToInclude": ["tables"]
        }),
        3,
    )
    .await?;

    let doc_meta = doc_result
        .get("document")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let doc_name = doc_meta
        .get("title")
        .or_else(|| doc_meta.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("untitled");
    let doc_id = slug::id_from_uri(&opts.doc_uri);
    let doc_slug = slug::slugify(doc_name, doc_id);

    stats.doc_title = doc_name.to_string();

    // Final destination for this doc
    let doc_dir = docs_dir.join(&doc_slug);
    stats.doc_dir = doc_dir.clone();

    // Use a temp directory for atomic writes: root/.sync_tmp/<slug>/
    let root = docs_dir.parent().unwrap();
    let sync_tmp_root = root.join(".sync_tmp");
    let sync_tmp_dir = sync_tmp_root.join(&doc_slug);

    // Concurrent sync guard: fail if another sync is already in progress
    if sync_tmp_dir.exists() {
        return Err(CodaError::Other(format!(
            "Sync already in progress for '{}'. If this is stale, delete {} and retry.",
            doc_slug,
            sync_tmp_dir.display()
        )));
    }

    std::fs::create_dir_all(&sync_tmp_dir)?;

    // Write into the temp dir; on failure, clean up and mark partial
    let result = sync_document_inner(
        client,
        opts,
        &sync_tmp_dir,
        &mut stats,
        &doc_meta,
        doc_name,
        &doc_result,
    )
    .await;

    match result {
        Ok(()) => {
            // Promote temp dir to final location atomically
            promote_sync_dir(&sync_tmp_dir, &doc_dir)?;

            // Update manifest with success
            manifest.docs.insert(
                opts.doc_uri.clone(),
                ManifestDocEntry {
                    slug: doc_slug,
                    title: doc_name.to_string(),
                    synced_at: now_rfc3339(),
                    page_count: stats.pages_synced,
                    table_count: stats.tables_synced,
                    status: "complete".to_string(),
                },
            );
        }
        Err(e) => {
            // Clean up temp dir on failure
            let _ = std::fs::remove_dir_all(&sync_tmp_dir);

            // Record partial status in manifest
            manifest.docs.insert(
                opts.doc_uri.clone(),
                ManifestDocEntry {
                    slug: doc_slug,
                    title: doc_name.to_string(),
                    synced_at: now_rfc3339(),
                    page_count: stats.pages_synced,
                    table_count: stats.tables_synced,
                    status: "partial".to_string(),
                },
            );

            return Err(e);
        }
    }

    Ok(stats)
}

/// Inner sync logic that writes all files into the given directory.
/// Separated so the caller can handle atomic promotion and cleanup.
async fn sync_document_inner(
    client: &dyn ToolCaller,
    opts: &SyncOpts,
    doc_dir: &Path,
    stats: &mut SyncStats,
    doc_meta: &Value,
    doc_name: &str,
    doc_result: &Value,
) -> Result<()> {
    // Write trimmed __doc.json (only non-null essential fields)
    let mut doc_json = serde_json::Map::new();
    doc_json.insert("title".into(), json!(doc_name));
    doc_json.insert("docUri".into(), json!(&opts.doc_uri));
    for key in &["browserLink", "owner", "createdAt"] {
        if let Some(val) = doc_meta.get(*key) {
            if !val.is_null() {
                doc_json.insert(key.to_string(), val.clone());
            }
        }
    }
    write_json(&doc_dir.join("__doc.json"), &Value::Object(doc_json))?;

    // Initialize doc sync state
    let mut doc_sync = DocSync {
        version: 1,
        doc_uri: opts.doc_uri.clone(),
        synced_at: now_rfc3339(),
        pages: HashMap::new(),
        tables: HashMap::new(),
    };

    // Step 2: Process pages
    let pages = doc_result
        .get("pages")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();

    let pages_dir = doc_dir.join("pages");
    std::fs::create_dir_all(&pages_dir)?;

    let tables_dir = pages_dir.join("tables");

    output::info(&format!("[sync] Found {} pages\n", pages.len()));

    let mut step = 2usize;
    let mut page_details: Vec<PageDetail> = Vec::new();
    let mut table_details: HashMap<String, TableDetail> = HashMap::new();

    for page in &pages {
        match sync_page(
            client,
            page,
            &pages_dir,
            &tables_dir,
            opts,
            &mut doc_sync,
            &mut step,
            &mut table_details,
        )
        .await
        {
            Ok((page_count, table_count, row_count, page_detail)) => {
                stats.pages_synced += page_count;
                stats.tables_synced += table_count;
                stats.rows_synced += row_count;
                page_details.push(page_detail);
            }
            Err(e) => {
                let page_title = page.get("title").and_then(|v| v.as_str()).unwrap_or("?");
                let msg = format!("Page '{}': {}", page_title, e);
                output::info(&format!("[sync] Error: {}\n", msg));
                stats.errors.push(msg);
            }
        }
    }

    // Generate CONTEXT.md
    write_context_md(
        doc_dir,
        doc_name,
        &opts.doc_uri,
        &page_details,
        &table_details,
    )?;

    // Save __sync.json
    doc_sync.synced_at = now_rfc3339();
    write_json(
        &doc_dir.join("__sync.json"),
        &serde_json::to_value(&doc_sync)?,
    )?;

    Ok(())
}

/// Sync a single page. Returns (pages_synced, tables_synced, rows_synced, PageDetail).
#[allow(clippy::too_many_arguments)]
async fn sync_page(
    client: &dyn ToolCaller,
    page: &Value,
    pages_dir: &Path,
    tables_dir: &Path,
    opts: &SyncOpts,
    doc_sync: &mut DocSync,
    step: &mut usize,
    table_details: &mut HashMap<String, TableDetail>,
) -> Result<(usize, usize, usize, PageDetail)> {
    let page_title = page
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled");
    let canvas_uri = page.get("canvasUri").and_then(|v| v.as_str());
    let page_uri = page.get("pageUri").and_then(|v| v.as_str());
    let read_uri = canvas_uri.or(page_uri).unwrap_or("");

    if read_uri.is_empty() {
        return Err(CodaError::Other(format!(
            "Page '{}' has no URI",
            page_title
        )));
    }

    let page_id = slug::id_from_uri(read_uri);
    let page_slug = slug::slugify(page_title, page_id);

    output::info(&format!("[sync]   Page: {}\n", page_title));

    // Read page content
    let read_payload = json!({
        "uri": read_uri,
        "contentTypesToInclude": ["markdown", "tables"],
        "markdownBlockLimit": 500
    });
    trace::emit_compound_step("sync", *step, "page_read", &read_payload);
    *step += 1;

    let page_data = call_with_retry(client, "page_read", read_payload, 3).await?;

    let content = page_data
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("");
    let has_content = !content.trim().is_empty();

    if !opts.tables_only {
        // Only write .md if content is non-empty
        if has_content {
            let md_path = pages_dir.join(format!("{}.md", page_slug));
            std::fs::write(&md_path, content)?;
        }

        // Always write page metadata
        let page_meta = json!({
            "title": page_title,
            "canvasUri": canvas_uri,
            "pageUri": page_uri,
            "slug": &page_slug,
        });
        let meta_path = pages_dir.join(format!("{}.json", page_slug));
        write_json(&meta_path, &page_meta)?;
    }

    let page_detail = PageDetail {
        slug: page_slug.clone(),
        title: page_title.to_string(),
        has_content,
    };

    // Process tables on this page
    let child_tables = page_data
        .get("tables")
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();

    let mut tables_synced = 0;
    let mut rows_synced = 0;
    let mut table_uris = Vec::new();

    for table in &child_tables {
        match sync_table(client, table, tables_dir, opts, doc_sync, step).await {
            Ok((t, r, detail)) => {
                tables_synced += t;
                rows_synced += r;
                if let Some(uri) = table.get("tableUri").and_then(|v| v.as_str()) {
                    table_uris.push(uri.to_string());
                    table_details.insert(uri.to_string(), detail);
                }
            }
            Err(e) => {
                let tbl_name = table.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                output::info(&format!("[sync]     Table '{}' error: {}\n", tbl_name, e));
            }
        }
    }

    // Record page in sync state
    doc_sync.pages.insert(
        read_uri.to_string(),
        PageSyncEntry {
            slug: page_slug,
            synced_at: now_rfc3339(),
            tables: table_uris,
        },
    );

    Ok((1, tables_synced, rows_synced, page_detail))
}

/// Sync a single table. Returns (tables_synced, rows_synced, TableDetail).
async fn sync_table(
    client: &dyn ToolCaller,
    table: &Value,
    tables_dir: &Path,
    opts: &SyncOpts,
    doc_sync: &mut DocSync,
    step: &mut usize,
) -> Result<(usize, usize, TableDetail)> {
    let table_name = table
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled");
    let table_uri = table.get("tableUri").and_then(|v| v.as_str()).unwrap_or("");
    if table_uri.is_empty() {
        return Err(CodaError::Other(format!(
            "Table '{}' has no URI",
            table_name
        )));
    }

    let table_id = slug::id_from_uri(table_uri);
    let table_slug = slug::slugify(table_name, table_id);

    output::info(&format!("[sync]     Table: {}\n", table_name));

    // Extract columns metadata
    let columns = table.get("columns").cloned().unwrap_or_else(|| json!([]));
    let columns_arr = columns.as_array().cloned().unwrap_or_default();
    let column_map = cell::build_column_map(&columns_arr);
    let column_names: Vec<String> = columns_arr
        .iter()
        .filter_map(|c| {
            c.get("name")
                .and_then(|n| n.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    // Fetch rows
    let row_limit = std::cmp::min(opts.max_rows, 100) as u64;
    let read_payload = json!({
        "uri": table_uri,
        "rowLimit": row_limit,
    });
    trace::emit_compound_step("sync", *step, "table_read_rows", &read_payload);
    *step += 1;

    let rows_result = call_with_retry(client, "table_read_rows", read_payload, 3).await?;

    let rows = rows_result
        .get("rows")
        .or_else(|| rows_result.get("items"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let row_count = rows.len();

    let detail = TableDetail {
        slug: table_slug.clone(),
        name: table_name.to_string(),
        column_names,
        row_count,
    };

    // Skip entirely empty tables (no columns AND no rows)
    if columns_arr.is_empty() && row_count == 0 {
        return Ok((0, 0, detail));
    }

    // Create table directory
    let table_dir = tables_dir.join(&table_slug);
    std::fs::create_dir_all(&table_dir)?;

    // Always write __schema.json
    let schema = json!({
        "tableName": table_name,
        "tableUri": table_uri,
        "columns": columns,
    });
    write_json(&table_dir.join("__schema.json"), &schema)?;

    // Only write rows.ndjson if there are rows — flatten before writing
    if row_count > 0 {
        let flattened: Vec<Value> = rows
            .iter()
            .map(|r| cell::flatten_row(r, &column_map))
            .collect();
        let rows_path = table_dir.join("rows.ndjson");
        write_ndjson(&rows_path, &flattened)?;
    }

    // Record table in sync state
    doc_sync.tables.insert(
        table_uri.to_string(),
        TableSyncEntry {
            slug: table_slug,
            row_count,
            synced_at: now_rfc3339(),
        },
    );

    Ok((1, row_count, detail))
}

// ---------------------------------------------------------------------------
// CONTEXT.md generation
// ---------------------------------------------------------------------------

fn write_context_md(
    doc_dir: &Path,
    doc_title: &str,
    doc_uri: &str,
    page_details: &[PageDetail],
    table_details: &HashMap<String, TableDetail>,
) -> Result<()> {
    use std::fmt::Write;
    let mut md = String::new();

    writeln!(md, "# {}", doc_title).unwrap();
    writeln!(md, "Synced: {}", now_rfc3339()).unwrap();
    writeln!(md, "Source: {}", doc_uri).unwrap();
    writeln!(md).unwrap();

    // Pages section
    writeln!(md, "## Pages").unwrap();
    for page in page_details {
        if page.has_content {
            writeln!(md, "- {}.md — \"{}\"", page.slug, page.title).unwrap();
        } else {
            writeln!(md, "- {}.json — \"{}\" (no content)", page.slug, page.title).unwrap();
        }
    }
    writeln!(md).unwrap();

    // Tables section
    if !table_details.is_empty() {
        writeln!(md, "## Tables").unwrap();
        for detail in table_details.values() {
            writeln!(
                md,
                "- tables/{}/ — \"{}\" ({}, {})",
                detail.slug,
                detail.name,
                plural(detail.column_names.len(), "column"),
                plural(detail.row_count, "row")
            )
            .unwrap();
            if !detail.column_names.is_empty() {
                let display_cols = if detail.column_names.len() > 20 {
                    format!(
                        "{}, ...and {} more",
                        detail.column_names[..20].join(", "),
                        detail.column_names.len() - 20
                    )
                } else {
                    detail.column_names.join(", ")
                };
                writeln!(md, "  Columns: {}", display_cols).unwrap();
            }
        }
    }

    std::fs::write(doc_dir.join("CONTEXT.md"), &md)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// INDEX.md generation — root-level index of all synced docs
// ---------------------------------------------------------------------------

fn write_index_md(root: &Path, manifest: &SyncManifest) -> Result<()> {
    use std::fmt::Write;
    let mut md = String::new();

    writeln!(md, "# Synced Coda Docs").unwrap();
    writeln!(md, "Last updated: {}", now_rfc3339()).unwrap();
    writeln!(md).unwrap();

    if manifest.docs.is_empty() {
        writeln!(
            md,
            "No docs synced yet. Run `shd sync --doc-url \"<url>\"` to add one."
        )
        .unwrap();
    } else {
        writeln!(md, "## Docs").unwrap();
        // Sort by title for stable output
        let mut entries: Vec<_> = manifest.docs.iter().collect();
        entries.sort_by(|a, b| a.1.title.cmp(&b.1.title));

        for (uri, entry) in &entries {
            writeln!(
                md,
                "- docs/{}/ — \"{}\" ({}, {})",
                entry.slug,
                entry.title,
                plural(entry.page_count, "page"),
                plural(entry.table_count, "table")
            )
            .unwrap();
            writeln!(md, "  Source: {}", uri).unwrap();
        }
        writeln!(md).unwrap();
    }

    writeln!(
        md,
        "Not every Coda doc is synced here. To add one: `shd sync --doc-url \"<url>\"`"
    )
    .unwrap();

    std::fs::write(root.join("INDEX.md"), &md)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Dry run
// ---------------------------------------------------------------------------

async fn dry_run_preview(client: &dyn ToolCaller, opts: &SyncOpts) -> Result<()> {
    let doc_result = client
        .call_tool(
            "document_read",
            json!({
                "uri": &opts.doc_uri,
                "contentTypesToInclude": ["tables"]
            }),
        )
        .await?;

    let doc_meta = doc_result
        .get("document")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let doc_name = doc_meta
        .get("title")
        .or_else(|| doc_meta.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("untitled");
    let doc_id = slug::id_from_uri(&opts.doc_uri);
    let doc_slug = slug::slugify(doc_name, doc_id);

    let pages = doc_result
        .get("pages")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();

    let mut page_previews = Vec::new();
    for page in &pages {
        let title = page.get("title").and_then(|v| v.as_str()).unwrap_or("?");
        let canvas_uri = page.get("canvasUri").and_then(|v| v.as_str());
        let page_uri = page.get("pageUri").and_then(|v| v.as_str());
        let uri = canvas_uri.or(page_uri).unwrap_or("");
        let pid = slug::id_from_uri(uri);
        page_previews.push(json!({
            "title": title,
            "slug": slug::slugify(title, pid),
        }));
    }

    let preview = json!({
        "dryRun": true,
        "docUri": &opts.doc_uri,
        "docSlug": doc_slug,
        "root": opts.root.display().to_string(),
        "pageCount": pages.len(),
        "pages": page_previews,
        "paths": {
            "docDir": format!("{}/docs/{}", opts.root.display(), doc_slug),
            "pagesDir": format!("{}/docs/{}/pages/", opts.root.display(), doc_slug),
            "tablesDir": format!("{}/docs/{}/pages/tables/", opts.root.display(), doc_slug),
        }
    });

    output::print_response(&preview, crate::output::OutputFormat::Json)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Manifest I/O
// ---------------------------------------------------------------------------

fn load_manifest(root: &Path) -> Result<SyncManifest> {
    let path = root.join(".sync_manifest.json");
    if !path.exists() {
        return Ok(SyncManifest {
            version: 1,
            ..Default::default()
        });
    }
    let contents = std::fs::read_to_string(&path)?;
    match serde_json::from_str(&contents) {
        Ok(m) => Ok(m),
        Err(e) => {
            output::info(&format!(
                "[sync] Warning: manifest corrupted ({}). Treating as fresh sync.\n",
                e
            ));
            Ok(SyncManifest {
                version: 1,
                ..Default::default()
            })
        }
    }
}

fn save_manifest(root: &Path, manifest: &SyncManifest) -> Result<()> {
    let mut m = manifest.clone();
    m.version = 1;
    m.synced_at = now_rfc3339();
    let path = root.join(".sync_manifest.json");
    write_json(&path, &serde_json::to_value(&m)?)
}

// ---------------------------------------------------------------------------
// File I/O helpers
// ---------------------------------------------------------------------------

fn write_json(path: &Path, value: &Value) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    std::fs::write(path, json)?;
    Ok(())
}

fn write_ndjson(path: &Path, rows: &[Value]) -> Result<()> {
    use std::io::Write;
    let file = std::fs::File::create(path)?;
    let mut writer = std::io::BufWriter::new(file);
    for row in rows {
        serde_json::to_writer(&mut writer, row)?;
        writeln!(writer)?;
    }
    Ok(())
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Simple pluralization: "1 page" vs "3 pages".
fn plural(n: usize, singular: &str) -> String {
    if n == 1 {
        format!("{n} {singular}")
    } else {
        format!("{n} {singular}s")
    }
}

// ---------------------------------------------------------------------------
// Retry helper (same pattern as compound.rs)
// ---------------------------------------------------------------------------

/// Call a tool with retry logic. Only retries on transient errors (429/5xx/network).
/// Allows extra retries for 409 (doc not ready after creation) with longer delays.
async fn call_with_retry(
    client: &dyn ToolCaller,
    tool_name: &str,
    payload: Value,
    max_retries: u32,
) -> Result<Value> {
    let mut last_err = None;
    // Allow extra retries for 409 (doc not ready after creation)
    let effective_retries = max_retries + 2;
    for attempt in 0..=effective_retries {
        match client.call_tool(tool_name, payload.clone()).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if !e.is_retriable() || attempt == effective_retries {
                    return Err(e);
                }
                // Use longer delay for 409 (doc not ready yet)
                let base_ms = if matches!(&e, CodaError::Api { status: 409, .. }) {
                    3000
                } else {
                    1000
                };
                let delay = base_ms * (attempt as u64 + 1);
                output::info(&format!(
                    "[sync] Retrying {} (attempt {}/{}): {}. Retrying in {delay}ms...\n",
                    tool_name,
                    attempt + 1,
                    effective_retries,
                    e,
                ));
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let mut manifest = SyncManifest {
            version: 1,
            synced_at: now_rfc3339(),
            docs: HashMap::new(),
        };
        manifest.docs.insert(
            "coda://docs/abc".to_string(),
            ManifestDocEntry {
                slug: "test-doc-abc123".to_string(),
                title: "Test Doc".to_string(),
                synced_at: now_rfc3339(),
                page_count: 3,
                table_count: 1,
                status: "complete".to_string(),
            },
        );

        save_manifest(root, &manifest).unwrap();
        let loaded = load_manifest(root).unwrap();
        assert_eq!(loaded.docs.len(), 1);
        assert!(loaded.docs.contains_key("coda://docs/abc"));
    }

    #[test]
    fn load_missing_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = load_manifest(dir.path()).unwrap();
        assert_eq!(manifest.version, 1);
        assert!(manifest.docs.is_empty());
    }

    #[test]
    fn write_ndjson_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rows.ndjson");
        let rows = vec![
            json!({"_rowId": "r1", "Name": "Alice"}),
            json!({"_rowId": "r2", "Name": "Bob"}),
        ];
        write_ndjson(&path, &rows).unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.trim().split('\n').collect();
        assert_eq!(lines.len(), 2);

        // Each line should be valid JSON with flattened keys
        let first: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["Name"], "Alice");
    }

    #[test]
    fn write_json_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        let value = json!({"key": "value"});
        write_json(&path, &value).unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        let parsed: Value = serde_json::from_str(&contents).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn context_md_format() {
        let dir = tempfile::tempdir().unwrap();
        let doc_dir = dir.path();

        let page_details = vec![
            PageDetail {
                slug: "overview-abc".to_string(),
                title: "Overview".to_string(),
                has_content: true,
            },
            PageDetail {
                slug: "empty-page-xyz".to_string(),
                title: "Empty Page".to_string(),
                has_content: false,
            },
        ];

        let mut table_details = HashMap::new();
        table_details.insert(
            "coda://docs/d/tables/t1".to_string(),
            TableDetail {
                slug: "tasks-t1".to_string(),
                name: "Tasks".to_string(),
                column_names: vec!["Name".into(), "Status".into(), "Owner".into()],
                row_count: 42,
            },
        );

        write_context_md(
            doc_dir,
            "Test Doc",
            "coda://docs/abc",
            &page_details,
            &table_details,
        )
        .unwrap();

        let content = std::fs::read_to_string(doc_dir.join("CONTEXT.md")).unwrap();
        assert!(content.contains("# Test Doc"));
        assert!(content.contains("Source: coda://docs/abc"));
        assert!(content.contains("overview-abc.md — \"Overview\""));
        assert!(content.contains("empty-page-xyz.json — \"Empty Page\" (no content)"));
        assert!(content.contains("tables/tasks-t1/"));
        assert!(content.contains("3 columns, 42 rows"));
        assert!(content.contains("Columns: Name, Status, Owner"));
    }

    #[test]
    fn context_md_truncates_many_columns() {
        let dir = tempfile::tempdir().unwrap();
        let doc_dir = dir.path();

        let mut table_details = HashMap::new();
        let many_cols: Vec<String> = (0..25).map(|i| format!("Col{}", i)).collect();
        table_details.insert(
            "uri".to_string(),
            TableDetail {
                slug: "big-table".to_string(),
                name: "Big Table".to_string(),
                column_names: many_cols,
                row_count: 10,
            },
        );

        write_context_md(doc_dir, "Doc", "uri", &[], &table_details).unwrap();

        let content = std::fs::read_to_string(doc_dir.join("CONTEXT.md")).unwrap();
        assert!(content.contains("...and 5 more"));
    }

    #[test]
    fn gitignore_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let gitignore = root.join(".gitignore");

        // Write a custom .gitignore
        std::fs::write(&gitignore, "custom content\n").unwrap();

        // Simulate what run() does
        if !gitignore.exists() {
            std::fs::write(&gitignore, "*\n").unwrap();
        }

        // Should still have custom content
        let content = std::fs::read_to_string(&gitignore).unwrap();
        assert_eq!(content, "custom content\n");
    }

    #[test]
    fn index_md_with_docs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let mut manifest = SyncManifest {
            version: 1,
            synced_at: now_rfc3339(),
            docs: HashMap::new(),
        };
        manifest.docs.insert(
            "coda://docs/abc".to_string(),
            ManifestDocEntry {
                slug: "q2-planning-abc".to_string(),
                title: "Q2 Planning".to_string(),
                synced_at: now_rfc3339(),
                page_count: 5,
                table_count: 2,
                status: "complete".to_string(),
            },
        );
        manifest.docs.insert(
            "coda://docs/xyz".to_string(),
            ManifestDocEntry {
                slug: "project-tracker-xyz".to_string(),
                title: "Project Tracker".to_string(),
                synced_at: now_rfc3339(),
                page_count: 3,
                table_count: 1,
                status: "complete".to_string(),
            },
        );

        write_index_md(root, &manifest).unwrap();

        let content = std::fs::read_to_string(root.join("INDEX.md")).unwrap();
        assert!(content.contains("# Synced Coda Docs"));
        assert!(content.contains("\"Q2 Planning\" (5 pages, 2 tables)"));
        assert!(content.contains("\"Project Tracker\" (3 pages, 1 table)"));
        assert!(content.contains("Source: coda://docs/abc"));
        assert!(content.contains("Not every Coda doc is synced here"));
    }

    #[test]
    fn index_md_empty() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let manifest = SyncManifest {
            version: 1,
            synced_at: now_rfc3339(),
            docs: HashMap::new(),
        };

        write_index_md(root, &manifest).unwrap();

        let content = std::fs::read_to_string(root.join("INDEX.md")).unwrap();
        assert!(content.contains("No docs synced yet"));
    }

    #[test]
    fn manifest_missing_status_defaults_to_complete() {
        let json = r#"{"version":1,"synced_at":"2026-01-01T00:00:00Z","docs":{"coda://docs/abc":{"slug":"test","title":"Test","synced_at":"2026-01-01T00:00:00Z","page_count":1,"table_count":0}}}"#;
        let manifest: SyncManifest = serde_json::from_str(json).unwrap();
        let entry = &manifest.docs["coda://docs/abc"];
        assert_eq!(entry.status, "complete");
    }

    #[test]
    fn promote_sync_dir_moves_tmp_to_final() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let sync_tmp = root.join(".sync_tmp").join("test-doc");
        let docs_dir = root.join("docs").join("test-doc");

        std::fs::create_dir_all(&sync_tmp).unwrap();
        std::fs::write(sync_tmp.join("test.md"), "hello").unwrap();

        promote_sync_dir(&sync_tmp, &docs_dir).unwrap();

        assert!(!sync_tmp.exists());
        assert!(docs_dir.join("test.md").exists());
        assert_eq!(
            std::fs::read_to_string(docs_dir.join("test.md")).unwrap(),
            "hello"
        );
    }

    #[test]
    fn promote_sync_dir_replaces_existing() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let sync_tmp = root.join(".sync_tmp").join("test-doc");
        let docs_dir = root.join("docs").join("test-doc");

        // Create existing final dir with old content
        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::write(docs_dir.join("old.md"), "old data").unwrap();

        // Create tmp dir with new content
        std::fs::create_dir_all(&sync_tmp).unwrap();
        std::fs::write(sync_tmp.join("new.md"), "new data").unwrap();

        promote_sync_dir(&sync_tmp, &docs_dir).unwrap();

        assert!(!sync_tmp.exists());
        assert!(!docs_dir.join("old.md").exists());
        assert!(docs_dir.join("new.md").exists());
        assert_eq!(
            std::fs::read_to_string(docs_dir.join("new.md")).unwrap(),
            "new data"
        );
    }

    #[test]
    fn corrupted_manifest_treated_as_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let manifest_path = tmp.path().join(".sync_manifest.json");
        std::fs::write(&manifest_path, "not valid json{{{").unwrap();
        let manifest = load_manifest(tmp.path()).unwrap();
        assert!(manifest.docs.is_empty());
    }
}
