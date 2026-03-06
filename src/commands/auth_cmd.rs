use crate::auth;
use crate::client::CodaClient;
use crate::error::Result;

pub async fn login(token: Option<&str>) -> Result<()> {
    let token = match token {
        Some(t) => t.to_string(),
        None => {
            eprint!("Enter your Coda API token: ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            input.trim().to_string()
        }
    };

    if token.is_empty() {
        return Err(crate::error::CodaError::Validation(
            "Token cannot be empty".into(),
        ));
    }

    // Verify the token works via the tool endpoint
    let client = CodaClient::new(token.clone())?;
    let resp = client.call_tool("whoami", serde_json::json!({})).await?;

    let name = resp
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    eprintln!("Authenticated as: {name}");

    auth::store_token(&token)?;
    eprintln!("Token saved to {}", auth::credential_path_display());

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
            println!("Not authenticated. Run `coda auth login` or set CODA_API_TOKEN.");
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
