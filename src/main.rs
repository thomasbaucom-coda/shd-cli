mod auth;
mod client;
mod commands;
mod error;
mod fuzzy;
mod output;
mod sanitize;
mod schema_cache;
mod trace;
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
    name = "shd",
    version,
    about = "Superhuman Docs CLI — agent-first interface for Coda",
    before_help = BANNER,
    long_about = "All commands call the Coda tool endpoint dynamically.\n\
                  Tools are discovered at runtime — new tools work without a CLI rebuild.\n\
                  Run `shd discover` to see available tools and their schemas.\n\n\
                  Auth: set CODA_API_TOKEN or run `shd auth login`.",
    after_help = "TOOL USAGE:\n  \
                  shd <tool_name> --json '{...}'                    Call any tool\n  \
                  shd table_create --json '{\"docId\":\"...\",\"canvasId\":\"...\",\"name\":\"...\",\"columns\":[...]}'\n  \
                  shd whoami                                        No payload needed\n  \
                  shd discover                                      List all tools\n  \
                  shd discover table_create                         Show tool schema\n  \
                  echo '{...}' | shd table_add_rows --json -        Read payload from stdin\n  \
                  shd content_modify --json @payload.json           Read payload from file"
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

    /// Extract field(s) from the response. Single: "name". Multi: "tableUri,columns" (returns JSON object).
    /// Dot-paths: "items.0.id". Multi-pick keys use each path's last segment.
    #[arg(long, global = true)]
    pick: Option<String>,

    /// Resolve tool name via fuzzy matching against cached tools
    #[arg(long, global = true)]
    fuzzy: bool,

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

        Commands::Tool(args) => {
            dispatch_tool(
                &client,
                &args,
                dry_run,
                cli.pick.as_deref(),
                cli.fuzzy,
                cli.output,
            )
            .await
        }

        Commands::Auth { .. } => unreachable!(),
    }
}

/// Parse dynamic tool args: <tool_name> [--json <payload>] [--dry-run] [--sanitize] [--trace] [--pick <field>] [--fuzzy]
/// If no --json is provided, sends empty payload.
async fn dispatch_tool(
    client: &client::CodaClient,
    args: &[String],
    mut dry_run: bool,
    mut pick: Option<&str>,
    mut use_fuzzy: bool,
    format: output::OutputFormat,
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
    let payload = if let Some(pos) = args.iter().position(|a| a == "--json") {
        let json_str = args
            .get(pos + 1)
            .ok_or_else(|| error::CodaError::Validation("--json requires a value".into()))?;
        validate::resolve_json_payload(json_str)?
    } else {
        serde_json::json!({})
    };

    // Client-side schema validation if cache available
    if let Ok(Some(cached)) = schema_cache::load() {
        if let Some(tool_schema) = schema_cache::find_tool(&cached.tools, &resolved_name) {
            schema_cache::validate_payload(tool_schema, &payload)?;
        }
    }

    commands::tools::call(client, &resolved_name, payload, dry_run, pick, format).await
}
