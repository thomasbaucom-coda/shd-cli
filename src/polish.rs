//! Text polishing — run content through Claude before sending to Coda.
//!
//! When `--polish` is set, text fields in the payload are sent to Anthropic's
//! Haiku model for editorial cleanup (grammar, punctuation, style guide rules).
//! The model makes surgical edits only — it does not rewrite voice or tone.

use crate::error::{CodaError, Result};
use serde_json::Value;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const POLISH_MODEL: &str = "claude-haiku-4-5-20251001";

/// Minimum text length to bother polishing (skip titles, short labels, etc.)
const MIN_POLISH_LEN: usize = 20;

const DEFAULT_STYLE_GUIDE: &str = r#"You are a precise text editor. Apply these rules surgically — change ONLY what violates them:

- Use the Oxford comma (e.g., "red, white, and blue")
- American English spelling (e.g., "color" not "colour")
- Spell out numbers under 10; use numerals for 10 and above
- Use em dashes (—) for parenthetical asides, not hyphens or double hyphens
- Use en dashes (–) for ranges (e.g., "10–20")
- No double spaces after periods
- Capitalize the first word after a colon only if it begins a complete sentence
- Use title case for headings (capitalize major words)
- Ensure subject-verb agreement
- Fix run-on sentences and comma splices
- Preserve all markdown formatting, links, code blocks, and structure exactly
- Do NOT rewrite for style, tone, or brevity — only fix mechanical errors
- Do NOT add or remove content, headings, or bullet points
- If the text has no errors, return it unchanged

Return ONLY the corrected text. No commentary, no explanations, no wrapping."#;

/// Inspect a tool payload and polish any text fields.
/// Mutates `payload` in place. Returns the number of fields polished.
pub async fn polish_payload(tool_name: &str, payload: &mut Value) -> Result<u32> {
    let paths = collect_polish_paths(tool_name, payload);
    if paths.is_empty() {
        return Ok(0);
    }

    let style_guide = load_style_guide();
    let mut count = 0u32;

    for path in &paths {
        let text = match payload.pointer(path).and_then(|v| v.as_str()) {
            Some(t) if t.len() >= MIN_POLISH_LEN => t.to_string(),
            _ => continue,
        };

        let polished = call_anthropic(&text, &style_guide).await?;

        // Only update if the model actually changed something
        if polished != text && !polished.is_empty() {
            if let Some(target) = payload.pointer_mut(path) {
                *target = Value::String(polished);
                count += 1;
            }
        }
    }

    Ok(count)
}

/// Collect JSON pointer paths to text fields that should be polished.
fn collect_polish_paths(tool_name: &str, payload: &Value) -> Vec<String> {
    match tool_name {
        "content_modify" => collect_content_modify_paths(payload),
        "page_create_with_content" => {
            if payload.get("content").and_then(|v| v.as_str()).is_some() {
                vec!["/content".to_string()]
            } else {
                vec![]
            }
        }
        "doc_scaffold" => collect_doc_scaffold_paths(payload),
        _ => {
            if payload
                .get("content")
                .and_then(|v| v.as_str())
                .map_or(false, |s| s.len() >= MIN_POLISH_LEN)
            {
                vec!["/content".to_string()]
            } else {
                vec![]
            }
        }
    }
}

/// For content_modify: polish operations[].content where blockType is "markdown".
fn collect_content_modify_paths(payload: &Value) -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(ops) = payload.get("operations").and_then(|v| v.as_array()) {
        for (i, op) in ops.iter().enumerate() {
            let is_markdown = op
                .get("blockType")
                .and_then(|v| v.as_str())
                .map(|bt| bt == "markdown")
                .unwrap_or(false);
            if is_markdown && op.get("content").and_then(|v| v.as_str()).is_some() {
                paths.push(format!("/operations/{}/content", i));
            }
        }
    }
    paths
}

/// For doc_scaffold: polish pages[].content fields.
fn collect_doc_scaffold_paths(payload: &Value) -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(pages) = payload.get("pages").and_then(|v| v.as_array()) {
        for (i, page) in pages.iter().enumerate() {
            if page.get("content").and_then(|v| v.as_str()).is_some() {
                paths.push(format!("/pages/{}/content", i));
            }
        }
    }
    paths
}

/// Load style guide: .coda/style-guide.md if it exists, else DEFAULT_STYLE_GUIDE.
fn load_style_guide() -> String {
    let path = std::path::Path::new(".coda/style-guide.md");
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(path) {
            if !content.trim().is_empty() {
                return content;
            }
        }
    }
    DEFAULT_STYLE_GUIDE.to_string()
}

/// Send text to Anthropic Haiku for polishing.
async fn call_anthropic(text: &str, style_guide: &str) -> Result<String> {
    let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
        CodaError::Polish(
            "ANTHROPIC_API_KEY not set. Required for --polish.\n\
             Set it: export ANTHROPIC_API_KEY=sk-ant-..."
                .into(),
        )
    })?;

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": POLISH_MODEL,
        "max_tokens": 4096,
        "messages": [{
            "role": "user",
            "content": format!("{style_guide}\n\n---\n\nText to polish:\n\n{text}")
        }]
    });

    let resp = client
        .post(ANTHROPIC_API_URL)
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| CodaError::Polish(format!("Anthropic API request failed: {e}")))?;

    let status = resp.status().as_u16();
    let resp_body: Value = resp
        .json()
        .await
        .map_err(|e| CodaError::Polish(format!("Failed to parse Anthropic response: {e}")))?;

    if status >= 400 {
        let msg = resp_body
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");
        return Err(CodaError::Polish(format!(
            "Anthropic API error ({status}): {msg}"
        )));
    }

    // Extract text from content[0].text
    let polished = resp_body
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|block| block.get("text"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| CodaError::Polish("Unexpected Anthropic response format".into()))?;

    Ok(polished.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn paths_content_modify_markdown_only() {
        let payload = json!({
            "uri": "coda://docs/abc/pages/xyz",
            "operations": [
                {"operation": "insert_element", "blockType": "markdown", "content": "Hello world, this is a test sentence."},
                {"operation": "insert_element", "blockType": "image", "content": "https://example.com/img.png"},
                {"operation": "insert_element", "blockType": "markdown", "content": "Another paragraph with some text here."}
            ]
        });
        let paths = collect_polish_paths("content_modify", &payload);
        assert_eq!(
            paths,
            vec!["/operations/0/content", "/operations/2/content"]
        );
    }

    #[test]
    fn paths_content_modify_no_operations() {
        let payload = json!({"uri": "coda://docs/abc"});
        let paths = collect_polish_paths("content_modify", &payload);
        assert!(paths.is_empty());
    }

    #[test]
    fn paths_page_create_with_content() {
        let payload = json!({
            "uri": "coda://docs/abc",
            "title": "My Page",
            "content": "Some markdown content for the page body."
        });
        let paths = collect_polish_paths("page_create_with_content", &payload);
        assert_eq!(paths, vec!["/content"]);
    }

    #[test]
    fn paths_page_create_no_content() {
        let payload = json!({"uri": "coda://docs/abc", "title": "My Page"});
        let paths = collect_polish_paths("page_create_with_content", &payload);
        assert!(paths.is_empty());
    }

    #[test]
    fn paths_doc_scaffold_multiple_pages() {
        let payload = json!({
            "title": "My Doc",
            "pages": [
                {"title": "Intro", "content": "Welcome to this document, it has great info."},
                {"title": "Empty Page"},
                {"title": "Details", "content": "Here are the details about the project plan."}
            ]
        });
        let paths = collect_polish_paths("doc_scaffold", &payload);
        assert_eq!(paths, vec!["/pages/0/content", "/pages/2/content"]);
    }

    #[test]
    fn paths_doc_scaffold_no_pages() {
        let payload = json!({"title": "My Doc"});
        let paths = collect_polish_paths("doc_scaffold", &payload);
        assert!(paths.is_empty());
    }

    #[test]
    fn paths_unknown_tool_empty() {
        let payload = json!({"uri": "coda://docs/abc", "title": "Test"});
        assert!(collect_polish_paths("whoami", &payload).is_empty());
        assert!(collect_polish_paths("table_create", &payload).is_empty());
        assert!(collect_polish_paths("page_create", &payload).is_empty());
    }

    #[test]
    fn paths_generic_content_field() {
        let payload = json!({"uri": "coda://docs/abc", "content": "Some long enough content to be polished here"});
        let paths = collect_polish_paths("some_unknown_tool", &payload);
        assert_eq!(paths, vec!["/content"]);
    }

    #[test]
    fn paths_generic_no_content_field() {
        let payload = json!({"uri": "coda://docs/abc", "title": "Short"});
        let paths = collect_polish_paths("some_unknown_tool", &payload);
        assert!(paths.is_empty());
    }

    #[test]
    fn paths_generic_short_content_skipped() {
        let payload = json!({"content": "Too short"});
        let paths = collect_polish_paths("unknown_tool", &payload);
        assert!(paths.is_empty());
    }

    #[test]
    fn load_style_guide_returns_default() {
        // Run from a temp dir where .coda/style-guide.md doesn't exist
        let _dir = tempfile::tempdir().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(_dir.path()).unwrap();

        let guide = load_style_guide();
        assert!(guide.contains("Oxford comma"));
        assert!(guide.contains("surgical"));

        std::env::set_current_dir(original).unwrap();
    }

    #[tokio::test]
    async fn polish_payload_skips_non_text_tools() {
        let mut payload = json!({"some": "data"});
        let count = polish_payload("whoami", &mut payload).await.unwrap();
        assert_eq!(count, 0);
        // Payload unchanged
        assert_eq!(payload, json!({"some": "data"}));
    }

    #[tokio::test]
    async fn polish_payload_skips_short_text() {
        // Text under MIN_POLISH_LEN should be skipped (no API call attempted)
        let mut payload = json!({
            "uri": "coda://docs/abc",
            "title": "Page",
            "content": "Short."
        });
        let count = polish_payload("page_create_with_content", &mut payload)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}
