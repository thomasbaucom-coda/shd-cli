mod auth;
mod client;
mod commands;
mod error;
mod output;
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
    about = "Superhuman Docs CLI — agent-first interface for Coda",
    before_help = BANNER,
    long_about = "All commands call the Coda tool endpoint dynamically.\n\
                  Tools are discovered at runtime — new tools work without a CLI rebuild.\n\
                  Run `coda discover` to see available tools and their schemas.\n\n\
                  Auth: set CODA_API_TOKEN or run `coda auth login`.",
    after_help = "TOOL USAGE:\n  \
                  coda <tool_name> --json '{...}'                    Call any tool\n  \
                  coda table_create --json '{\"docId\":\"...\",\"canvasId\":\"...\",\"name\":\"...\",\"columns\":[...]}'\n  \
                  coda whoami                                        No payload needed\n  \
                  coda discover                                      List all tools\n  \
                  coda discover table_create                         Show tool schema\n  \
                  echo '{...}' | coda table_add_rows --json -        Read payload from stdin"
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
    },

    /// Import rows from stdin, auto-batched (convenience wrapper)
    Import {
        #[arg(allow_hyphen_values = true)]
        doc_id: String,
        table_id: String,
        /// Column IDs as JSON array
        #[arg(long)]
        columns: String,
        /// Rows per batch (max 100, default 100)
        #[arg(long, default_value = "100")]
        batch_size: usize,
    },

    /// Start an MCP server over stdio
    Mcp,

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
            "type": match &e {
                error::CodaError::ContractChanged { .. } => "contract_changed",
                error::CodaError::Api { .. } => "api_error",
                error::CodaError::Validation(_) => "validation_error",
                error::CodaError::NoToken => "auth_required",
                _ => "error",
            },
            "message": e.to_string(),
        });
        eprintln!("{}", serde_json::to_string_pretty(&error_json).unwrap_or_else(|_| e.to_string()));
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> error::Result<()> {
    let dry_run = cli.dry_run;
    output::set_sanitize(cli.sanitize);

    // Auth doesn't require a token
    if let Commands::Auth { action } = &cli.command {
        return match action {
            AuthAction::Login { token } => {
                commands::auth_cmd::login(token.as_deref()).await
            }
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
        Commands::Discover { tool_name } => {
            match tool_name {
                Some(name) => commands::discover::discover_one(&client, &name).await,
                None => commands::discover::discover_all(&client).await,
            }
        }

        Commands::Import { doc_id, table_id, columns, batch_size } => {
            let size = batch_size.min(100);
            commands::tools::import_rows(&client, &doc_id, &table_id, &columns, size, dry_run).await
        }

        Commands::Mcp => {
            commands::mcp::start().await
        }

        Commands::Tool(args) => {
            dispatch_tool(&client, &args, dry_run).await
        }

        Commands::Auth { .. } => unreachable!(),
    }
}

/// Parse dynamic tool args: <tool_name> [--json <payload>] [--dry-run] [--sanitize]
/// If no --json is provided, sends empty payload.
async fn dispatch_tool(
    client: &client::CodaClient,
    args: &[String],
    mut dry_run: bool,
) -> error::Result<()> {
    if args.is_empty() {
        return Err(error::CodaError::Validation(
            "Usage: coda <tool_name> [--json '{...}']\nRun `coda discover` to see available tools.".into(),
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

    // Find --json value
    let payload = if let Some(pos) = args.iter().position(|a| a == "--json") {
        let json_str = args.get(pos + 1).ok_or_else(|| {
            error::CodaError::Validation("--json requires a value".into())
        })?;
        validate::resolve_json_payload(json_str)?
    } else {
        serde_json::json!({})
    };

    commands::tools::call(client, tool_name, payload, dry_run).await
}
