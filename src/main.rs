mod auth;
mod cell;
mod client;
mod commands;
mod error;
mod fuzzy;
mod output;
mod polish;
mod sanitize;
mod schema_cache;
mod slug;
mod trace;
mod validate;

use clap::{Parser, Subcommand};
use client::ToolCaller;
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

fn print_welcome() {
    let v = env!("CARGO_PKG_VERSION");
    let w = 45; // inner width between │ markers
    let title = format!("      ●      Superhuman Docs CLI  v{v}");
    let sub = "     ╱ ╲     agent-first interface for Coda";
    let arrow = "    ╱   ╲";
    let blank = "";
    eprint!(
        "\
╭{bar}╮
│{blank:<w$}│
│{title:<w$}│
│{sub:<w$}│
│{arrow:<w$}│
│{blank:<w$}│
╰{bar}╯

  Get started:
    shd auth login                  Authenticate with Coda
    shd discover                    List all available tools
    shd <tool> --json '{{...}}'       Call any tool
    shd --help                      Full usage & options
",
        bar = "─".repeat(w),
    );
}

const HELP_TEMPLATE: &str = concat!(
    r#"
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
"#,
    "  agent-first interface for Coda — v{version}\n\n",
    "{about-with-newline}\n",
    "{usage-heading} {usage}\n\n",
    "{all-args}",
    "{after-help}",
);

#[derive(Parser)]
#[command(
    name = "shd",
    version,
    about = "Tools are discovered at runtime — new tools work without a CLI rebuild.\n\
             Run `shd discover` to see available tools and their schemas.\n\
             Auth: set CODA_API_TOKEN or run `shd auth login`.",
    help_template = HELP_TEMPLATE,
    after_help = "\nExamples:\n  \
                  shd <tool_name> --json '{...}'               Call any tool\n  \
                  shd whoami                                    No payload needed\n  \
                  shd discover                                  List all tools\n  \
                  shd discover table_create                     Show tool schema\n  \
                  echo '{...}' | shd table_add_rows --json -    Read payload from stdin\n  \
                  shd content_modify --json @payload.json       Read payload from file"
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

    /// Preview the request without executing it
    #[arg(long, global = true)]
    dry_run: bool,

    /// Sanitize responses to redact prompt injection patterns
    #[arg(long, global = true)]
    sanitize: bool,

    /// Emit NDJSON execution traces to stderr
    #[arg(long, global = true)]
    trace: bool,

    /// RECOMMENDED — extract only needed field(s) to minimize token usage.
    /// Single: "name". Multi: "tableUri,columns" (returns JSON object).
    /// Dot-paths: "items.0.id". Multi-pick keys use each path's last segment.
    #[arg(long, global = true)]
    pick: Option<String>,

    /// Resolve tool name via fuzzy matching against cached tools
    #[arg(long, global = true)]
    fuzzy: bool,

    /// Polish text content through Claude before sending to Coda
    #[arg(long, global = true, env = "SHD_POLISH")]
    polish: bool,

    /// Suppress informational stderr messages (status, cache hints)
    #[arg(long, global = true)]
    quiet: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate with Coda
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },

    /// Discover available tools and their schemas
    Discover {
        /// Tool name to inspect (omit to list all)
        tool_name: Option<String>,
        /// Force refresh from network (ignore cache)
        #[arg(long)]
        refresh: bool,
        /// Filter tools by name or description (case-insensitive substring)
        #[arg(long)]
        filter: Option<String>,
        /// Show compact schema (required fields + types only, agent-friendly)
        #[arg(long)]
        compact: bool,
    },

    /// Start an MCP server over stdio
    Mcp,

    /// Start a persistent shell for agents (JSON-line protocol over stdio)
    Shell,

    /// Sync a Coda document to the local filesystem for agent access
    Sync {
        /// Document URI (coda://docs/{docId})
        #[arg(long)]
        doc_uri: Option<String>,

        /// Coda document browser URL (e.g., https://coda.io/d/My-Doc_dAbCdEf)
        #[arg(long)]
        doc_url: Option<String>,

        /// Output directory
        #[arg(long, default_value = ".coda")]
        root: String,

        /// Re-sync everything, ignore cached state
        #[arg(long)]
        force: bool,

        /// Skip page content, only sync table data
        #[arg(long)]
        tables_only: bool,

        /// Maximum rows per table
        #[arg(long, default_value = "5000")]
        max_rows: usize,
    },

    /// Call any tool dynamically. Usage: coda <tool_name> [--json '{...}']
    #[command(external_subcommand)]
    Tool(Vec<String>),
}

#[derive(Subcommand)]
enum AuthAction {
    /// Store an API token
    Login {
        #[arg(long)]
        token: Option<String>,
    },
    /// Show current auth status
    Status,
    /// Remove stored credentials
    Logout,
}

#[tokio::main]
async fn main() {
    // No arguments → show the compact welcome screen instead of full --help
    if std::env::args_os().len() == 1 {
        print_welcome();
        std::process::exit(0);
    }

    let cli = Cli::parse();

    let result = run(cli).await;

    if let Err(e) = result {
        let error_json = serde_json::json!({
            "error": true,
            "type": e.error_type(),
            "message": e.to_string(),
        });
        eprintln!(
            "{}",
            serde_json::to_string_pretty(&error_json).unwrap_or_else(|_| e.to_string())
        );
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> error::Result<()> {
    let dry_run = cli.dry_run;
    output::set_sanitize(cli.sanitize);
    trace::set_trace(cli.trace);
    output::set_quiet(cli.quiet);

    // Auth doesn't require a token
    if let Commands::Auth { action } = &cli.command {
        return match action {
            AuthAction::Login { token } => commands::auth_cmd::login(token.as_deref()).await,
            AuthAction::Status => commands::auth_cmd::status(),
            AuthAction::Logout => commands::auth_cmd::logout(),
        };
    }

    // All other commands require auth (dry-run can proceed without a real token)
    let token = if dry_run {
        auth::resolve_token(cli.token.as_deref()).unwrap_or_else(|_| "DRY_RUN_NO_TOKEN".into())
    } else {
        auth::resolve_token(cli.token.as_deref())?
    };
    let client = client::CodaClient::new(token)?;

    match cli.command {
        Commands::Discover {
            tool_name,
            refresh,
            filter,
            compact,
        } => match tool_name {
            Some(name) => commands::discover::discover_one(&client, &name, refresh, compact).await,
            None => commands::discover::discover_all(&client, refresh, filter.as_deref()).await,
        },

        Commands::Mcp => commands::mcp::start().await,

        Commands::Shell => commands::shell::start(&client, dry_run).await,

        Commands::Sync {
            doc_uri,
            doc_url,
            root,
            force,
            tables_only,
            max_rows,
        } => {
            let resolved_uri = match (doc_uri, doc_url) {
                (Some(uri), _) => uri,
                (None, Some(url)) => {
                    slug::resolve_doc_input(&url).map_err(error::CodaError::Validation)?
                }
                (None, None) => {
                    return Err(error::CodaError::Validation(
                        "Either --doc-uri or --doc-url is required.\n\
                         Example: shd sync --doc-url \"https://coda.io/d/My-Doc_dAbCdEf\""
                            .into(),
                    ))
                }
            };
            let root_path = expand_tilde(&root);
            commands::sync::run(
                &client,
                commands::sync::SyncOpts {
                    doc_uri: resolved_uri,
                    root: root_path,
                    force,
                    dry_run,
                    tables_only,
                    max_rows,
                },
            )
            .await
        }

        Commands::Tool(args) => {
            dispatch_tool(
                &client,
                &args,
                dry_run,
                cli.pick.as_deref(),
                cli.fuzzy,
                cli.output,
                cli.polish,
            )
            .await
        }

        Commands::Auth { .. } => unreachable!(),
    }
}

/// Expand `~` to the user's home directory.
fn expand_tilde(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    std::path::PathBuf::from(path)
}

/// Parse dynamic tool args: <tool_name> [--json <payload>] [--dry-run] [--sanitize] [--trace] [--pick <field>] [--fuzzy] [--sync]
/// If no --json is provided, sends empty payload.
async fn dispatch_tool(
    client: &client::CodaClient,
    args: &[String],
    mut dry_run: bool,
    mut pick: Option<&str>,
    mut use_fuzzy: bool,
    format: output::OutputFormat,
    mut polish: bool,
) -> error::Result<()> {
    if args.is_empty() {
        return Err(error::CodaError::Validation(
            "Usage: shd <tool_name> [--json '{...}']\nRun `shd discover` to see available tools."
                .into(),
        ));
    }

    let tool_name = &args[0];

    // Check for flags that may appear after the tool name
    // (clap can't parse them as global flags in external_subcommand)
    if args.iter().any(|a| a == "--dry-run") {
        dry_run = true;
    }
    if args.iter().any(|a| a == "--sanitize") {
        output::set_sanitize(true);
    }
    if args.iter().any(|a| a == "--trace") {
        trace::set_trace(true);
    }
    if args.iter().any(|a| a == "--fuzzy") {
        use_fuzzy = true;
    }
    if args.iter().any(|a| a == "--quiet") {
        output::set_quiet(true);
    }
    if args.iter().any(|a| a == "--polish") {
        polish = true;
    }
    let auto_sync = args.iter().any(|a| a == "--sync");

    // Parse --pick from external subcommand args
    let pick_owned: Option<String> = args
        .iter()
        .position(|a| a == "--pick")
        .and_then(|pos| args.get(pos + 1).cloned());
    if pick.is_none() {
        if let Some(ref p) = pick_owned {
            pick = Some(p.as_str());
        }
    }

    // Fuzzy resolve tool name if --fuzzy is set
    let resolved_name = if use_fuzzy {
        let tools = match schema_cache::load()? {
            Some(cached) => cached.tools,
            None => {
                output::info("No cached tools. Fetching from Coda MCP endpoint...\n");
                let tools = client.fetch_tools().await?;
                schema_cache::save(&tools)?;
                tools
            }
        };
        let name = fuzzy::resolve(tool_name, &tools)?;
        if name != *tool_name {
            output::info(&format!("[fuzzy] Resolved '{tool_name}' -> '{name}'\n"));
        }
        name
    } else {
        tool_name.to_string()
    };

    // Find --json value
    let mut payload = if let Some(pos) = args.iter().position(|a| a == "--json") {
        let json_str = args
            .get(pos + 1)
            .ok_or_else(|| error::CodaError::Validation("--json requires a value".into()))?;
        validate::resolve_json_payload(json_str)?
    } else {
        serde_json::json!({})
    };

    // Polish text fields before sending to Coda
    if polish {
        let count = polish::polish_payload(&resolved_name, &mut payload).await?;
        if count > 0 {
            output::info(&format!("[polish] Polished {count} text field(s).\n"));
        }
    }

    // Handle dry-run before dispatching to tools or compound operations
    if dry_run {
        if commands::compound::is_compound(&resolved_name) {
            let preview = commands::compound::dry_run_preview(&resolved_name, &payload);
            output::print_response(&preview, format)?;
        } else {
            output::print_response(&client.dry_run_tool(&resolved_name, &payload)?, format)?;
        }
        return Ok(());
    }

    // Execute the tool and capture the result for --sync
    let result: Option<serde_json::Value> = if commands::compound::is_compound(&resolved_name) {
        commands::compound::dispatch(client, &resolved_name, payload, pick, format).await?
    } else {
        // Client-side schema validation if cache available
        if let Ok(Some(cached)) = schema_cache::load() {
            if let Some(tool_schema) = schema_cache::find_tool(&cached.tools, &resolved_name) {
                schema_cache::validate_payload(tool_schema, &payload)?;
            }
        }
        commands::tools::call(client, &resolved_name, payload, pick, format).await?
    };

    // Auto-sync: if --sync was passed and the result contains a docUri,
    // spawn a background child process to sync the doc.
    // This avoids blocking for ~20s while Coda makes the doc ready.
    if auto_sync {
        if let Some(ref value) = result {
            if let Some(doc_uri) = extract_doc_uri(value) {
                spawn_background_sync(&doc_uri)?;
            } else {
                output::info("[sync] No docUri found in response — nothing to sync.\n");
            }
        }
    }

    Ok(())
}

/// Spawn a background child process to sync a doc.
/// The parent returns immediately; the child waits for Coda readiness, then syncs.
fn spawn_background_sync(doc_uri: &str) -> error::Result<()> {
    let exe = std::env::current_exe().map_err(|e| {
        error::CodaError::Other(format!("Could not determine CLI executable path: {e}"))
    })?;

    let child = std::process::Command::new(exe)
        .args(["sync", "--doc-uri", doc_uri, "--quiet"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    match child {
        Ok(c) => {
            output::info(&format!(
                "\n[sync] Syncing in background (pid {}). Files will appear in .coda/ shortly.\n",
                c.id()
            ));
        }
        Err(e) => {
            output::info(&format!(
                "\n[sync] Could not spawn background sync: {e}\n\
                 Run manually: shd sync --doc-uri \"{doc_uri}\"\n"
            ));
        }
    }

    Ok(())
}

/// Extract a doc URI from a tool response.
/// Checks for `docUri`, `uri`, or constructs one from `docId`.
fn extract_doc_uri(value: &serde_json::Value) -> Option<String> {
    // Direct docUri field
    if let Some(uri) = value.get("docUri").and_then(|v| v.as_str()) {
        if uri.starts_with("coda://") {
            return Some(uri.to_string());
        }
    }

    // URI field that looks like a doc URI
    if let Some(uri) = value.get("uri").and_then(|v| v.as_str()) {
        if uri.starts_with("coda://docs/") && !uri.contains("/pages/") {
            return Some(uri.to_string());
        }
    }

    // Construct from docId
    if let Some(doc_id) = value.get("docId").and_then(|v| v.as_str()) {
        return Some(format!("coda://docs/{doc_id}"));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_doc_uri_from_doc_uri_field() {
        let val = json!({"docUri": "coda://docs/abc", "title": "Test"});
        assert_eq!(extract_doc_uri(&val), Some("coda://docs/abc".into()));
    }

    #[test]
    fn extract_doc_uri_from_uri_field() {
        let val = json!({"uri": "coda://docs/abc"});
        assert_eq!(extract_doc_uri(&val), Some("coda://docs/abc".into()));
    }

    #[test]
    fn extract_doc_uri_skips_page_uri() {
        let val = json!({"uri": "coda://docs/abc/pages/xyz"});
        assert_eq!(extract_doc_uri(&val), None);
    }

    #[test]
    fn extract_doc_uri_from_doc_id() {
        let val = json!({"docId": "abc123"});
        assert_eq!(extract_doc_uri(&val), Some("coda://docs/abc123".into()));
    }

    #[test]
    fn extract_doc_uri_none_when_missing() {
        let val = json!({"name": "hello"});
        assert_eq!(extract_doc_uri(&val), None);
    }

    #[test]
    fn expand_tilde_home() {
        let expanded = expand_tilde("~/test");
        assert!(expanded.to_string_lossy().contains("test"));
        assert!(!expanded.to_string_lossy().starts_with("~"));
    }

    #[test]
    fn expand_tilde_no_tilde() {
        let expanded = expand_tilde(".coda");
        assert_eq!(expanded, std::path::PathBuf::from(".coda"));
    }
}
