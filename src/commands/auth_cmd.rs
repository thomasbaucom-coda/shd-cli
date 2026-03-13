use crate::auth;
use crate::client::{CodaClient, ToolCaller};
use crate::error::Result;

const TOKEN_URL: &str = "https://coda.io/account#apiSettings";
const MCP_TOKEN_URL: &str =
    "https://coda.io/account?openDialog=CREATE_API_TOKEN&scopeType=mcp#apiSettings";

/// Opens a URL in the default browser. Returns true if successful.
fn open_browser(url: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn().is_ok()
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .is_ok()
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()
            .is_ok()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = url;
        false
    }
}

pub async fn login(token: Option<&str>) -> Result<()> {
    let token = match token {
        Some(t) => t.to_string(),
        None => {
            eprintln!();
            eprintln!("  Generate an API token (MCP-scoped for full access):");
            eprintln!();
            eprintln!("  1. Your browser will open to Coda's token page");
            eprintln!("  2. Click \"Generate API token\"");
            eprintln!("  3. Copy the token and paste it below");
            eprintln!();

            // Open the MCP-scoped token page directly
            if open_browser(MCP_TOKEN_URL) {
                eprintln!("  Opening {MCP_TOKEN_URL}");
            } else {
                eprintln!("  Go to: {MCP_TOKEN_URL}");
            }
            eprintln!();

            eprint!("  Paste your token: ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            input.trim().to_string()
        }
    };

    if token.is_empty() {
        return Err(crate::error::CodaError::Validation(
            "Token cannot be empty. Run `shd auth login` to try again.".into(),
        ));
    }

    // Verify the token works via the tool endpoint
    eprintln!();
    eprint!("  Verifying token...");
    let client = CodaClient::new(token.clone())?;
    let resp = client.call_tool("whoami", serde_json::json!({})).await?;

    let name = resp
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    eprintln!(" done!");
    eprintln!();
    eprintln!("  Authenticated as: {name}");

    auth::store_token(&token)?;
    eprintln!("  Token saved to {}", auth::credential_path_display());
    eprintln!();

    Ok(())
}

pub fn status() -> Result<()> {
    match auth::resolve_token(None) {
        Ok(_) => {
            let source = if std::env::var("CODA_API_TOKEN").is_ok() {
                "CODA_API_TOKEN environment variable"
            } else {
                &format!("credential file ({})", auth::credential_path_display())
            };
            println!("Authenticated via {source}");
            Ok(())
        }
        Err(_) => {
            println!("Not authenticated. Run `shd auth login` or set CODA_API_TOKEN.");
            Ok(())
        }
    }
}

pub fn logout() -> Result<()> {
    if auth::remove_token()? {
        println!("Token removed from {}", auth::credential_path_display());
    } else {
        println!("No stored token found.");
    }
    Ok(())
}
