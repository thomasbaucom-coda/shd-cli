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
    let path = format!("/docs/{}/pages", validate::encode_path_segment(doc_id));
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
    page_id: &str,
    format: OutputFormat,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(page_id, "pageId")?;
    let path = format!(
        "/docs/{}/pages/{}",
        validate::encode_path_segment(doc_id),
        validate::encode_path_segment(page_id),
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

pub async fn create(
    client: &CodaClient,
    doc_id: &str,
    name: Option<&str>,
    json_payload: Option<&str>,
    format: OutputFormat,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    let path = format!("/docs/{}/pages", validate::encode_path_segment(doc_id));

    let body = match json_payload {
        Some(raw) => validate::resolve_json_payload(raw)?,
        None => {
            let name = name.ok_or_else(|| {
                crate::error::CodaError::Validation(
                    "Either --name or --json is required for page creation".into(),
                )
            })?;
            serde_json::json!({ "name": name })
        }
    };

    let req = client.build_request(reqwest::Method::POST, &path, Some(body), vec![]);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    let resp = client.execute(req).await?;
    output::print_response(&resp.body, format)?;
    Ok(())
}

pub async fn update(
    client: &CodaClient,
    doc_id: &str,
    page_id: &str,
    json_payload: &str,
    format: OutputFormat,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(page_id, "pageId")?;
    let path = format!(
        "/docs/{}/pages/{}",
        validate::encode_path_segment(doc_id),
        validate::encode_path_segment(page_id),
    );
    let body = validate::resolve_json_payload(json_payload)?;
    let req = client.build_request(reqwest::Method::PUT, &path, Some(body), vec![]);

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
    doc_id: &str,
    page_id: &str,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(page_id, "pageId")?;
    let path = format!(
        "/docs/{}/pages/{}",
        validate::encode_path_segment(doc_id),
        validate::encode_path_segment(page_id),
    );
    let req = client.build_request(reqwest::Method::DELETE, &path, None, vec![]);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    client.execute(req).await?;
    eprintln!("Page {page_id} deleted.");
    Ok(())
}

pub async fn content(
    client: &CodaClient,
    doc_id: &str,
    page_id: &str,
    format: OutputFormat,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(page_id, "pageId")?;
    let path = format!(
        "/docs/{}/pages/{}/content",
        validate::encode_path_segment(doc_id),
        validate::encode_path_segment(page_id),
    );
    let req = client.build_request(reqwest::Method::GET, &path, None, vec![]);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    let resp = client.execute(req).await?;
    output::print_list_response(&resp.body, format)?;
    Ok(())
}
