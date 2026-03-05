use crate::client::CodaClient;
use crate::error::Result;
use crate::output::{self, OutputFormat};
use crate::validate;

pub async fn list(
    client: &CodaClient,
    doc_id: &str,
    format: OutputFormat,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    let path = format!("/docs/{}/acl/permissions", validate::encode_path_segment(doc_id));
    let req = client.build_request(reqwest::Method::GET, &path, None, vec![]);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    let resp = client.execute(req).await?;
    output::print_list_response(&resp.body, format)?;
    Ok(())
}

pub async fn get_metadata(
    client: &CodaClient,
    doc_id: &str,
    format: OutputFormat,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    let path = format!("/docs/{}/acl/metadata", validate::encode_path_segment(doc_id));
    let req = client.build_request(reqwest::Method::GET, &path, None, vec![]);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    let resp = client.execute(req).await?;
    output::print_response(&resp.body, format)?;
    Ok(())
}

pub async fn add(
    client: &CodaClient,
    doc_id: &str,
    json_payload: &str,
    format: OutputFormat,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    let path = format!("/docs/{}/acl/permissions", validate::encode_path_segment(doc_id));
    let body = validate::resolve_json_payload(json_payload)?;
    let req = client.build_request(reqwest::Method::POST, &path, Some(body), vec![]);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    let resp = client.execute(req).await?;
    output::print_response(&resp.body, format)?;
    Ok(())
}

pub async fn remove(
    client: &CodaClient,
    doc_id: &str,
    permission_id: &str,
    dry_run: bool,
) -> Result<()> {
    validate::validate_resource_id(doc_id, "docId")?;
    validate::validate_resource_id(permission_id, "permissionId")?;
    let path = format!(
        "/docs/{}/acl/permissions/{}",
        validate::encode_path_segment(doc_id),
        validate::encode_path_segment(permission_id),
    );
    let req = client.build_request(reqwest::Method::DELETE, &path, None, vec![]);

    if dry_run {
        output::print_response(&req.to_dry_run_json(), OutputFormat::Json)?;
        return Ok(());
    }

    client.execute(req).await?;
    eprintln!("Permission {permission_id} removed.");
    Ok(())
}
