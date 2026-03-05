use crate::client::CodaClient;
use crate::error::Result;
use crate::output::{self, OutputFormat};
use crate::validate;

pub async fn list(
    client: &CodaClient,
    doc_id: &str,
    format: OutputFormat,
    limit: Option<u32>,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    let path = format!("/docs/{}/controls", validate::encode_path_segment(doc_id));
    let mut params = Vec::new();
    if let Some(l) = limit {
        params.push(("limit".to_string(), l.to_string()));
    }

    let req = client.build_request(reqwest::Method::GET, &path, None, params);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    let resp = client.execute(req).await?;
    output::print_list_response(&resp.body, format)?;
    Ok(())
}

pub async fn get(
    client: &CodaClient,
    doc_id: &str,
    control_id: &str,
    format: OutputFormat,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(control_id, "controlId")?;
    let path = format!(
        "/docs/{}/controls/{}",
        validate::encode_path_segment(doc_id),
        validate::encode_path_segment(control_id),
    );
    let req = client.build_request(reqwest::Method::GET, &path, None, vec![]);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    let resp = client.execute(req).await?;
    output::print_response(&resp.body, format)?;
    Ok(())
}
