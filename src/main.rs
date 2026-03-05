mod auth;
mod client;
mod commands;
mod error;
mod output;
mod paginate;
mod sanitize;
mod validate;

use clap::{Parser, Subcommand};
use output::OutputFormat;

const BANNER: &str = r#"
  ____  _   _ ____  _____ ____  _   _ _   _ __  __    _    _   _
 / ___|| | | |  _ \| ____|  _ \| | | | | | |  \/  |  / \  | \ | |
 \___ \| | | | |_) |  _| | |_) | |_| | | | | |\/| | / _ \ |  \| |
  ___) | |_| |  __/| |___|  _ <|  _  | |_| | |  | |/ ___ \| |\  |
 |____/ \___/|_|   |_____|_| \_\_| |_|\___/|_|  |_/_/   \_\_| \_|
  ____   ___   ____ ____
 |  _ \ / _ \ / ___/ ___|
 | | | | | | | |   \___ \
 | |_| | |_| | |___ ___) |
 |____/ \___/ \____|____/
"#;

#[derive(Parser)]
#[command(
    name = "coda",
    version,
    about = "Superhuman Docs CLI — agent-first command-line interface for Coda",
    before_help = BANNER,
    long_about = "Superhuman Docs CLI provides programmatic access to Coda docs, pages, tables, rows, and more.\n\n\
                  Designed for AI agents: structured JSON output, --dry-run safety, schema introspection,\n\
                  dynamic command registration, and input validation against hallucinated parameters.\n\n\
                  Auth: set CODA_API_TOKEN or run `coda auth login`.",
    after_help = "EXAMPLES:\n  \
                  coda docs list --output json\n  \
                  coda rows list <docId> <tableId> --fields \"Name,Status\" --limit 10\n  \
                  coda tool table-create <docId> <canvasId> --name \"Tasks\" --columns '[...]'\n  \
                  coda tool search <docId> --json '{\"query\": \"meetings\"}'\n  \
                  coda schema rows.list\n  \
                  coda docs create --json '{\"title\": \"My Doc\"}' --dry-run"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// API token (overrides CODA_API_TOKEN and stored credentials)
    #[arg(long, global = true, env = "CODA_API_TOKEN", hide_env = true)]
    token: Option<String>,

    /// Output format: json, table, ndjson
    #[arg(long, global = true, default_value = "json", value_parser = OutputFormat::from_str_opt)]
    output: OutputFormat,

    /// Preview the API request without executing it
    #[arg(long, global = true)]
    dry_run: bool,

    /// Sanitize API responses to redact potential prompt injection patterns
    #[arg(long, global = true)]
    sanitize: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate with Coda
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },

    /// Get info about the authenticated user
    Whoami,

    /// Manage docs
    Docs {
        #[command(subcommand)]
        action: DocsAction,
    },

    /// Manage pages within a doc
    Pages {
        #[command(subcommand)]
        action: PagesAction,
    },

    /// List and inspect tables within a doc
    Tables {
        #[command(subcommand)]
        action: TablesAction,
    },

    /// List and inspect columns within a table
    Columns {
        #[command(subcommand)]
        action: ColumnsAction,
    },

    /// Manage rows within a table
    Rows {
        #[command(subcommand)]
        action: RowsAction,
    },

    /// List and inspect formulas within a doc
    Formulas {
        #[command(subcommand)]
        action: FormulasAction,
    },

    /// List and inspect controls within a doc
    Controls {
        #[command(subcommand)]
        action: ControlsAction,
    },

    /// Manage folders
    Folders {
        #[command(subcommand)]
        action: FoldersAction,
    },

    /// Manage doc permissions and sharing
    Permissions {
        #[command(subcommand)]
        action: PermissionsAction,
    },

    /// Decode a Coda URL into structured resource IDs
    ResolveUrl {
        /// Coda URL (e.g. https://coda.io/d/_dAbCdEf/Page_suXYZ)
        url: String,
    },

    /// Internal tools: table create, content modify, and more (uses internal API)
    Tool {
        #[command(subcommand)]
        action: ToolAction,
    },

    /// Start an MCP server over stdio (for AI agent integrations)
    Mcp,

    /// Inspect API schema for a resource or method (no network call)
    Schema {
        /// Path: 'list', '<resource>', or '<resource>.<method>'
        path: String,
    },
}

// --- Auth ---

#[derive(Subcommand)]
enum AuthAction {
    /// Store an API token
    Login {
        /// API token to store (or enter interactively)
        #[arg(long)]
        token: Option<String>,
    },
    /// Show current auth status
    Status,
    /// Remove stored credentials
    Logout,
}

// --- Docs ---

#[derive(Subcommand)]
enum DocsAction {
    /// List accessible docs
    List {
        #[arg(long)]
        limit: Option<u32>,
        /// Search query to filter docs
        #[arg(long)]
        query: Option<String>,
        /// Auto-paginate and stream all results as NDJSON
        #[arg(long)]
        page_all: bool,
    },
    /// Get a doc by ID
    Get {
        /// Document ID
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
    },
    /// Create a new doc
    Create {
        /// Doc title (convenience flag)
        #[arg(long)]
        title: Option<String>,
        /// Full API request body as JSON
        #[arg(long)]
        json: Option<String>,
    },
    /// Delete a doc
    Delete {
        /// Document ID
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
    },
}

// --- Pages ---

#[derive(Subcommand)]
enum PagesAction {
    /// List pages in a doc
    List {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Get a page by ID
    Get {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        page_id: String,
    },
    /// Create a new page
    Create {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        json: Option<String>,
    },
    /// Update a page
    Update {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        page_id: String,
        /// Full API request body as JSON
        #[arg(long)]
        json: String,
    },
    /// Delete a page
    Delete {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        page_id: String,
    },
    /// Get page content (child objects)
    Content {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        page_id: String,
    },
}

// --- Tables ---

#[derive(Subcommand)]
enum TablesAction {
    /// List tables in a doc
    List {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Get a table by ID
    Get {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        table_id: String,
    },
}

// --- Columns ---

#[derive(Subcommand)]
enum ColumnsAction {
    /// List columns in a table
    List {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        table_id: String,
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Get a column by ID
    Get {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        table_id: String,
        column_id: String,
    },
}

// --- Rows ---

#[derive(Subcommand)]
enum RowsAction {
    /// List rows in a table
    List {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        table_id: String,
        #[arg(long)]
        limit: Option<u32>,
        /// Search query for rows
        #[arg(long)]
        query: Option<String>,
        /// Sort by column (natural, createdAt, updatedAt)
        #[arg(long)]
        sort_by: Option<String>,
        /// Comma-separated column names to include in output
        #[arg(long)]
        fields: Option<String>,
        /// Auto-paginate and stream all rows as NDJSON
        #[arg(long)]
        page_all: bool,
    },
    /// Get a single row by ID
    Get {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        table_id: String,
        row_id: String,
    },
    /// Insert or upsert rows
    Upsert {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        table_id: String,
        /// Full API request body as JSON
        #[arg(long)]
        json: String,
    },
    /// Update a single row
    Update {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        table_id: String,
        row_id: String,
        /// Full API request body as JSON
        #[arg(long)]
        json: String,
    },
    /// Delete a single row
    Delete {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        table_id: String,
        row_id: String,
    },
    /// Delete multiple rows
    DeleteRows {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        table_id: String,
        /// JSON body with rowIds to delete
        #[arg(long)]
        json: String,
    },
    /// Push a button column on a row
    PushButton {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        table_id: String,
        row_id: String,
        column_id: String,
    },
    /// Import rows from stdin (NDJSON or JSON array), auto-batched
    Import {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        table_id: String,
        /// Comma-separated key columns for upsert matching
        #[arg(long)]
        key_columns: Option<String>,
        /// Rows per API batch (max 500, default 500)
        #[arg(long, default_value = "500")]
        batch_size: usize,
    },
}

// --- Formulas ---

#[derive(Subcommand)]
enum FormulasAction {
    /// List formulas in a doc
    List {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Get a formula by ID
    Get {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        formula_id: String,
    },
}

// --- Controls ---

#[derive(Subcommand)]
enum ControlsAction {
    /// List controls in a doc
    List {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Get a control by ID
    Get {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        control_id: String,
    },
}

// --- Permissions ---

#[derive(Subcommand)]
enum PermissionsAction {
    /// List permissions on a doc
    List {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
    },
    /// Get sharing metadata for a doc
    Metadata {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
    },
    /// Add a permission to a doc
    Add {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        /// Full API request body as JSON
        #[arg(long)]
        json: String,
    },
    /// Remove a permission
    Remove {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        permission_id: String,
    },
}

// --- Folders ---

#[derive(Subcommand)]
enum FoldersAction {
    /// List folders
    List,
    /// Get a folder by ID
    Get {
        folder_id: String,
    },
    /// Create a new folder
    Create {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        json: Option<String>,
    },
    /// Delete a folder
    Delete {
        folder_id: String,
    },
}

// --- Tool (Internal API) ---

#[derive(Subcommand)]
#[command(
    after_help = "Any unrecognized subcommand is dispatched dynamically to the tool endpoint.\n\
                  Example: coda tool page_duplicate <docId> --json '{...}'\n\
                  Run `coda tool list <docId>` to discover all available tools."
)]
enum ToolAction {
    /// Discover available tools from the server (requires MCP-scoped token)
    List {
        /// Document ID (needed to reach the tool endpoint)
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        /// Filter by topic: getting_started, table, content, comment, formula, page, document, navigation
        #[arg(long)]
        topic: Option<String>,
    },
    /// Create a table with typed columns on a page
    TableCreate {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        /// Canvas/page ID where the table will be placed
        canvas_id: String,
        /// Table name
        #[arg(long)]
        name: String,
        /// Columns as JSON array: [{"name":"Col","format":{"type":"none"}}]
        #[arg(long)]
        columns: String,
        /// Initial rows as JSON array of arrays (values in column order)
        #[arg(long)]
        rows: Option<String>,
    },
    /// Add rows to an existing table (bulk)
    TableAddRows {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        table_id: String,
        /// Column IDs as JSON array: ["c-abc","c-def"]
        #[arg(long)]
        columns: String,
        /// Rows as JSON array of arrays (values in column order)
        #[arg(long)]
        rows: String,
    },
    /// Add columns to an existing table
    TableAddColumns {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        table_id: String,
        /// Columns as JSON array
        #[arg(long)]
        columns: String,
    },
    /// Delete rows from a table
    TableDeleteRows {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        table_id: String,
        /// JSON payload with row IDs or filter
        #[arg(long)]
        json: String,
    },
    /// Update rows in a table
    TableUpdateRows {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        table_id: String,
        /// JSON payload with row updates
        #[arg(long)]
        json: String,
    },
    /// Import rows from stdin via internal API, auto-batched
    ImportRows {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        table_id: String,
        /// Column IDs as JSON array: ["c-abc","c-def"]
        #[arg(long)]
        columns: String,
        /// Rows per batch (max 100 for internal API, default 100)
        #[arg(long, default_value = "100")]
        batch_size: usize,
    },
    /// Modify page content (add text, headings, lists)
    ContentModify {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        canvas_id: String,
        /// Operations as JSON (see tool_guide "content" topic)
        #[arg(long)]
        operations: String,
    },
    /// Add, reply to, or delete comments
    CommentManage {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        /// JSON payload with action and comment data
        #[arg(long)]
        json: String,
    },
    /// Create a named formula on a page
    FormulaCreate {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        canvas_id: String,
        /// Formula name
        #[arg(long)]
        name: String,
        /// Coda Formula Language expression
        #[arg(long)]
        formula: String,
    },
    /// Execute a Coda Formula Language expression
    FormulaExecute {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        /// CFL expression to evaluate
        #[arg(long)]
        formula: String,
    },
    /// Configure a table view (rename, filter, change layout)
    ViewConfigure {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        table_id: String,
        /// View ID (use "default" for the default view)
        #[arg(long, default_value = "default")]
        view_id: String,
        /// View name
        #[arg(long)]
        name: Option<String>,
        /// Layout: grid, card, timeline, calendar
        #[arg(long)]
        layout: Option<String>,
        /// Filter formula (CFL expression, or "none" to clear)
        #[arg(long)]
        filter: Option<String>,
    },
    /// Call any internal tool by name with a raw JSON payload
    Raw {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        /// Tool name (e.g. table_create, content_modify, comment_manage)
        tool_name: String,
        /// Full payload as JSON
        #[arg(long)]
        json: String,
    },

    /// Dynamic dispatch: any unrecognized tool name is called directly.
    /// Usage: coda tool <tool_name> <doc_id> --json '{...}'
    #[command(external_subcommand)]
    Dynamic(Vec<String>),
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = run(cli).await;

    if let Err(e) = result {
        // Structured error output for agents
        let error_json = serde_json::json!({
            "error": true,
            "message": e.to_string(),
        });
        eprintln!("{}", serde_json::to_string_pretty(&error_json).unwrap_or_else(|_| e.to_string()));
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> error::Result<()> {
    let format = cli.output;
    let dry_run = cli.dry_run;
    output::set_sanitize(cli.sanitize);

    // Commands that don't require auth
    match &cli.command {
        Commands::Auth { action } => {
            return match action {
                AuthAction::Login { token } => {
                    commands::auth_cmd::login(token.as_deref()).await
                }
                AuthAction::Status => commands::auth_cmd::status(),
                AuthAction::Logout => commands::auth_cmd::logout(),
            };
        }
        Commands::Schema { path } => {
            return commands::schema::handle(path);
        }
        _ => {}
    }

    // All other commands require auth (dry-run can proceed without a real token)
    let token = if dry_run {
        auth::resolve_token(cli.token.as_deref()).unwrap_or_else(|_| "DRY_RUN_NO_TOKEN".into())
    } else {
        auth::resolve_token(cli.token.as_deref())?
    };
    let client = client::CodaClient::new(token)?;

    match cli.command {
        Commands::Whoami => {
            commands::whoami::run(&client, format).await
        }

        Commands::Docs { action } => match action {
            DocsAction::List { limit, query, page_all } => {
                if page_all {
                    let mut params = Vec::new();
                    if let Some(l) = limit { params.push(("limit".to_string(), l.to_string())); }
                    if let Some(q) = &query { params.push(("query".to_string(), q.clone())); }
                    let req = client.build_request(reqwest::Method::GET, "/docs", None, params);
                    paginate::fetch_all_pages(&client, req, 100, None).await?;
                    return Ok(());
                }
                commands::docs::list(&client, format, limit, query.as_deref(), dry_run).await
            }
            DocsAction::Get { doc_id } => {
                commands::docs::get(&client, &doc_id, format, dry_run).await
            }
            DocsAction::Create { title, json } => {
                commands::docs::create(&client, title.as_deref(), json.as_deref(), format, dry_run).await
            }
            DocsAction::Delete { doc_id } => {
                commands::docs::delete(&client, &doc_id, dry_run).await
            }
        },

        Commands::Pages { action } => match action {
            PagesAction::List { doc_id, limit } => {
                commands::pages::list(&client, &doc_id, format, limit, dry_run).await
            }
            PagesAction::Get { doc_id, page_id } => {
                commands::pages::get(&client, &doc_id, &page_id, format, dry_run).await
            }
            PagesAction::Create { doc_id, name, json } => {
                commands::pages::create(&client, &doc_id, name.as_deref(), json.as_deref(), format, dry_run).await
            }
            PagesAction::Update { doc_id, page_id, json } => {
                commands::pages::update(&client, &doc_id, &page_id, &json, format, dry_run).await
            }
            PagesAction::Delete { doc_id, page_id } => {
                commands::pages::delete(&client, &doc_id, &page_id, dry_run).await
            }
            PagesAction::Content { doc_id, page_id } => {
                commands::pages::content(&client, &doc_id, &page_id, format, dry_run).await
            }
        },

        Commands::Tables { action } => match action {
            TablesAction::List { doc_id, limit } => {
                commands::tables::list(&client, &doc_id, format, limit, dry_run).await
            }
            TablesAction::Get { doc_id, table_id } => {
                commands::tables::get(&client, &doc_id, &table_id, format, dry_run).await
            }
        },

        Commands::Columns { action } => match action {
            ColumnsAction::List { doc_id, table_id, limit } => {
                commands::columns::list(&client, &doc_id, &table_id, format, limit, dry_run).await
            }
            ColumnsAction::Get { doc_id, table_id, column_id } => {
                commands::columns::get(&client, &doc_id, &table_id, &column_id, format, dry_run).await
            }
        },

        Commands::Rows { action } => match action {
            RowsAction::List { doc_id, table_id, limit, query, sort_by, fields, page_all } => {
                if page_all {
                    let mut params = vec![
                        ("useColumnNames".to_string(), "true".to_string()),
                        ("valueFormat".to_string(), "simpleWithArrays".to_string()),
                    ];
                    if let Some(l) = limit { params.push(("limit".to_string(), l.to_string())); }
                    if let Some(q) = &query { params.push(("query".to_string(), q.clone())); }
                    if let Some(s) = &sort_by { params.push(("sortBy".to_string(), s.clone())); }
                    validate::validate_resource_id(&doc_id, "docId")?;
                    validate::validate_resource_id(&table_id, "tableId")?;
                    let path = format!("/docs/{}/tables/{}/rows", validate::encode_path_segment(&doc_id), validate::encode_path_segment(&table_id));
                    let req = client.build_request(reqwest::Method::GET, &path, None, params);
                    paginate::fetch_all_pages(&client, req, 100, fields.as_deref()).await?;
                    return Ok(());
                }
                commands::rows::list(&client, &doc_id, &table_id, format, limit, query.as_deref(), sort_by.as_deref(), fields.as_deref(), dry_run).await
            }
            RowsAction::Get { doc_id, table_id, row_id } => {
                commands::rows::get(&client, &doc_id, &table_id, &row_id, format, dry_run).await
            }
            RowsAction::Upsert { doc_id, table_id, json } => {
                commands::rows::upsert(&client, &doc_id, &table_id, &json, format, dry_run).await
            }
            RowsAction::Update { doc_id, table_id, row_id, json } => {
                commands::rows::update(&client, &doc_id, &table_id, &row_id, &json, format, dry_run).await
            }
            RowsAction::Delete { doc_id, table_id, row_id } => {
                commands::rows::delete(&client, &doc_id, &table_id, &row_id, dry_run).await
            }
            RowsAction::DeleteRows { doc_id, table_id, json } => {
                commands::rows::delete_rows(&client, &doc_id, &table_id, &json, dry_run).await
            }
            RowsAction::PushButton { doc_id, table_id, row_id, column_id } => {
                commands::rows::push_button(&client, &doc_id, &table_id, &row_id, &column_id, dry_run).await
            }
            RowsAction::Import { doc_id, table_id, key_columns, batch_size } => {
                let size = batch_size.min(500); // Coda API max is 500
                commands::rows::import(&client, &doc_id, &table_id, key_columns.as_deref(), size, dry_run).await
            }
        },

        Commands::Formulas { action } => match action {
            FormulasAction::List { doc_id, limit } => {
                commands::formulas::list(&client, &doc_id, format, limit, dry_run).await
            }
            FormulasAction::Get { doc_id, formula_id } => {
                commands::formulas::get(&client, &doc_id, &formula_id, format, dry_run).await
            }
        },

        Commands::Controls { action } => match action {
            ControlsAction::List { doc_id, limit } => {
                commands::controls::list(&client, &doc_id, format, limit, dry_run).await
            }
            ControlsAction::Get { doc_id, control_id } => {
                commands::controls::get(&client, &doc_id, &control_id, format, dry_run).await
            }
        },

        Commands::Folders { action } => match action {
            FoldersAction::List => {
                commands::folders::list(&client, format, dry_run).await
            }
            FoldersAction::Get { folder_id } => {
                commands::folders::get(&client, &folder_id, format, dry_run).await
            }
            FoldersAction::Create { name, json } => {
                commands::folders::create(&client, name.as_deref(), json.as_deref(), format, dry_run).await
            }
            FoldersAction::Delete { folder_id } => {
                commands::folders::delete(&client, &folder_id, dry_run).await
            }
        },

        Commands::Permissions { action } => match action {
            PermissionsAction::List { doc_id } => {
                commands::permissions::list(&client, &doc_id, format, dry_run).await
            }
            PermissionsAction::Metadata { doc_id } => {
                commands::permissions::get_metadata(&client, &doc_id, format, dry_run).await
            }
            PermissionsAction::Add { doc_id, json } => {
                commands::permissions::add(&client, &doc_id, &json, format, dry_run).await
            }
            PermissionsAction::Remove { doc_id, permission_id } => {
                commands::permissions::remove(&client, &doc_id, &permission_id, dry_run).await
            }
        },

        Commands::ResolveUrl { url } => {
            commands::resolve_url::run(&client, &url, format, dry_run).await
        }

        Commands::Tool { action } => match action {
            ToolAction::List { doc_id, topic } => {
                commands::tools::list_tools(&client, &doc_id, topic.as_deref()).await
            }
            ToolAction::TableCreate { doc_id, canvas_id, name, columns, rows } => {
                commands::tools::table_create(&client, &doc_id, &canvas_id, &name, &columns, rows.as_deref(), dry_run).await
            }
            ToolAction::TableAddRows { doc_id, table_id, columns, rows } => {
                commands::tools::table_add_rows(&client, &doc_id, &table_id, &columns, &rows, dry_run).await
            }
            ToolAction::TableAddColumns { doc_id, table_id, columns } => {
                commands::tools::table_add_columns(&client, &doc_id, &table_id, &columns, dry_run).await
            }
            ToolAction::TableDeleteRows { doc_id, table_id, json } => {
                commands::tools::table_delete_rows(&client, &doc_id, &table_id, &json, dry_run).await
            }
            ToolAction::TableUpdateRows { doc_id, table_id, json } => {
                commands::tools::table_update_rows(&client, &doc_id, &table_id, &json, dry_run).await
            }
            ToolAction::ImportRows { doc_id, table_id, columns, batch_size } => {
                let size = batch_size.min(100);
                commands::tools::import_rows(&client, &doc_id, &table_id, &columns, size, dry_run).await
            }
            ToolAction::ContentModify { doc_id, canvas_id, operations } => {
                commands::tools::content_modify(&client, &doc_id, &canvas_id, &operations, dry_run).await
            }
            ToolAction::CommentManage { doc_id, json } => {
                commands::tools::comment_manage(&client, &doc_id, &json, dry_run).await
            }
            ToolAction::FormulaCreate { doc_id, canvas_id, name, formula } => {
                commands::tools::formula_create(&client, &doc_id, &canvas_id, &name, &formula, dry_run).await
            }
            ToolAction::FormulaExecute { doc_id, formula } => {
                commands::tools::formula_execute(&client, &doc_id, &formula, dry_run).await
            }
            ToolAction::ViewConfigure { doc_id, table_id, view_id, name, layout, filter } => {
                commands::tools::view_configure(&client, &doc_id, &table_id, &view_id, name.as_deref(), layout.as_deref(), filter.as_deref(), dry_run).await
            }
            ToolAction::Raw { doc_id, tool_name, json } => {
                commands::tools::raw(&client, &doc_id, &tool_name, &json, dry_run).await
            }
            ToolAction::Dynamic(args) => {
                commands::tools::dynamic_dispatch(&client, &args, dry_run).await
            }
        },

        Commands::Mcp => {
            commands::mcp::start().await
        }

        // Already handled above
        Commands::Auth { .. } | Commands::Schema { .. } => unreachable!(),
    }
}
