use crate::client::CodaClient;
use crate::error::Result;
use crate::output::{self, OutputFormat};

pub async fn run(client: &CodaClient, format: OutputFormat) -> Result<()> {
    let req = client.build_request(reqwest::Method::GET, "/whoami", None, vec![]);
    let resp = client.execute(req).await?;
    output::print_response(&resp.body, format)?;
    Ok(())
}
