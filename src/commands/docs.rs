use crate::client::CodaClient;
use crate::error::Result;
use crate::output::{self, OutputFormat};
use crate::validate;
use serde_json::Value;

pub async fn list(
    client: &CodaClient,
    format: OutputFormat,
    limit: Option<u32>,
    query: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    let mut params = Vec::new();
    if let Some(l) = limit {
        params.push(("limit".to_string(), l.to_string()));
    }
    if let Some(q) = query {
        params.push(("query".to_string(), q.to_string()));
    }

    let req = client.build_request(reqwest::Method::GET, "/docs", None, params);

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
    format: OutputFormat,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    let path = format!("/docs/{}", validate::encode_path_segment(doc_id));
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
    title: Option<&str>,
    json_payload: Option<&str>,
    format: OutputFormat,
    dry_run: bool,
) -> Result<()> {
    let body = match json_payload {
        Some(raw) => validate::resolve_json_payload(raw)?,
        None => {
            let title = title.ok_or_else(|| {
                crate::error::CodaError::Validation(
                    "Either --title or --json is required for doc creation".into(),
                )
            })?;
            serde_json::json!({ "title": title })
        }
    };

    let req = client.build_request(reqwest::Method::POST, "/docs", Some(body), vec![]);

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
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    let path = format!("/docs/{}", validate::encode_path_segment(doc_id));
    let req = client.build_request(reqwest::Method::DELETE, &path, None, vec![]);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    let resp = client.execute(req).await?;
    if resp.body.is_null() || resp.body == Value::Object(serde_json::Map::new()) {
        eprintln!("Document {doc_id} deleted.");
    } else {
        output::print_response(&resp.body, OutputFormat::Json)?;
    }
    Ok(())
}
