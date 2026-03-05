use crate::client::{ApiRequest, CodaClient};
use crate::error::Result;
use serde_json::Value;

/// Fetch all pages for a list endpoint, streaming NDJSON items to stdout.
/// If `fields` is provided, only those column names are kept in each row's `values`.
/// Returns the total number of items fetched.
pub async fn fetch_all_pages(
    client: &CodaClient,
    initial_req: ApiRequest,
    max_pages: u32,
    fields: Option<&str>,
) -> Result<u32> {
    let mut total = 0u32;
    let mut pages_fetched = 0u32;
    let url_base = initial_req.url.split('?').next().unwrap_or(&initial_req.url).to_string();

    let field_list: Option<Vec<&str>> = fields.map(|f| f.split(',').map(|s| s.trim()).collect());

    let resp = client.execute(initial_req).await?;
    total += emit_items(&resp.body, &field_list)?;
    pages_fetched += 1;

    let mut next_token = resp.body
        .get("nextPageToken")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    while let Some(token) = next_token {
        if pages_fetched >= max_pages {
            break;
        }

        let req = ApiRequest {
            method: reqwest::Method::GET,
            url: url_base.clone(),
            body: None,
            query_params: vec![("pageToken".to_string(), token)],
        };

        let resp = client.execute(req).await?;
        total += emit_items(&resp.body, &field_list)?;
        pages_fetched += 1;

        next_token = resp.body
            .get("nextPageToken")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
    }

    eprintln!("[page-all] Fetched {total} items across {pages_fetched} pages.");
    Ok(total)
}

fn emit_items(body: &Value, field_list: &Option<Vec<&str>>) -> Result<u32> {
    let mut count = 0u32;
    if let Some(items) = body.get("items").and_then(|v| v.as_array()) {
        for item in items {
            let output = match field_list {
                Some(fields) => filter_item_fields(item, fields),
                None => item.clone(),
            };
            println!("{}", serde_json::to_string(&output)?);
            count += 1;
        }
    }
    Ok(count)
}

fn filter_item_fields(item: &Value, fields: &[&str]) -> Value {
    let mut filtered = item.clone();
    if let Some(values) = filtered.get_mut("values").and_then(|v| v.as_object_mut()) {
        let keys: Vec<String> = values.keys().cloned().collect();
        for key in keys {
            if !fields.iter().any(|f| f.eq_ignore_ascii_case(&key)) {
                values.remove(&key);
            }
        }
    }
    filtered
}
