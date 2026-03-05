use crate::client::CodaClient;
use crate::error::Result;
use crate::output::{self, OutputFormat};
use crate::validate;

pub async fn list(
    client: &CodaClient,
    format: OutputFormat,
    dry_run: bool,
) -> Result<()> {
    let req = client.build_request(reqwest::Method::GET, "/folders", None, vec![]);

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
    folder_id: &str,
    format: OutputFormat,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(folder_id, "folderId")?;
    let path = format!("/folders/{}", validate::encode_path_segment(folder_id));
    let req = client.build_request(reqwest::Method::GET, &path, None, vec![]);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    let resp = client.execute(req).await?;
    output::print_response(&resp.body, format)?;
    Ok(())
}

pub async fn create(
    client: &CodaClient,
    name: Option<&str>,
    json_payload: Option<&str>,
    format: OutputFormat,
    dry_run: bool,
) -> Result<()> {
    let body = match json_payload {
        Some(raw) => validate::resolve_json_payload(raw)?,
        None => {
            let name = name.ok_or_else(|| {
                crate::error::CodaError::Validation(
                    "Either --name or --json is required for folder creation".into(),
                )
            })?;
            serde_json::json!({ "name": name })
        }
    };

    let req = client.build_request(reqwest::Method::POST, "/folders", Some(body), vec![]);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    let resp = client.execute(req).await?;
    output::print_response(&resp.body, format)?;
    Ok(())
}

pub async fn delete(
    client: &CodaClient,
    folder_id: &str,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(folder_id, "folderId")?;
    let path = format!("/folders/{}", validate::encode_path_segment(folder_id));
    let req = client.build_request(reqwest::Method::DELETE, &path, None, vec![]);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    client.execute(req).await?;
    eprintln!("Folder {folder_id} deleted.");
    Ok(())
}
