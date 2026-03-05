use crate::client::CodaClient;
use crate::error::Result;
use crate::output::{self, OutputFormat};

pub async fn run(
    client: &CodaClient,
    url: &str,
    format: OutputFormat,
    dry_run: bool,
) -> Result<()> {
    let params = vec![("url".to_string(), url.to_string())];
    let req = client.build_request(reqwest::Method::GET, "/resolveBrowserLink", None, params);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    let resp = client.execute(req).await?;
    output::print_response(&resp.body, format)?;
    Ok(())
}
