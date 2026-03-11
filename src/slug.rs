//! Slugification utilities for converting Coda names to filesystem-safe paths.

/// Convert a name and an ID suffix into a filesystem-safe slug.
///
/// Rules:
/// - Lowercase
/// - Replace non-alphanumeric characters with hyphens
/// - Collapse consecutive hyphens
/// - Strip leading/trailing hyphens
/// - Truncate to 80 characters before appending the ID suffix
/// - Append `-{suffix}` (first 6 chars of the ID) to prevent collisions
///
/// Example: slugify("Q2 Planning Doc", "AbCdEfGhIj") → "q2-planning-doc-abcdef"
pub fn slugify(name: &str, id: &str) -> String {
    let base: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();

    // Collapse consecutive hyphens and trim
    let mut slug = String::with_capacity(base.len());
    let mut prev_hyphen = true; // treat start as hyphen to strip leading
    for c in base.chars() {
        if c == '-' {
            if !prev_hyphen {
                slug.push('-');
            }
            prev_hyphen = true;
        } else {
            slug.push(c);
            prev_hyphen = false;
        }
    }

    // Strip trailing hyphen
    while slug.ends_with('-') {
        slug.pop();
    }

    // Truncate to 80 chars
    if slug.len() > 80 {
        slug.truncate(80);
        // Don't end on a hyphen after truncation
        while slug.ends_with('-') {
            slug.pop();
        }
    }

    // Fallback for empty slugs
    if slug.is_empty() {
        slug = "untitled".to_string();
    }

    // Append ID suffix (first 6 chars, lowercased)
    let suffix: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(6)
        .collect::<String>()
        .to_lowercase();

    if suffix.is_empty() {
        slug
    } else {
        format!("{slug}-{suffix}")
    }
}

/// Parse a Coda browser URL and extract the doc ID.
///
/// Accepts:
///   `https://coda.io/d/Title_dAbCdEf`
///   `https://coda.io/d/Title_dAbCdEf/Page-Name_suXyZ`
///   `https://coda.io/d/_dAbCdEf`
///
/// Returns the doc ID (e.g., `"AbCdEf"`) or `None` if not a valid Coda URL.
pub fn parse_coda_url(url: &str) -> Option<String> {
    let url = url.trim().trim_end_matches('/');

    let path_after_d = url
        .strip_prefix("https://coda.io/d/")
        .or_else(|| url.strip_prefix("http://coda.io/d/"))?;

    // Take the first path segment (before any '/' for sub-page)
    let first_segment = path_after_d.split('/').next()?;

    // Find the last occurrence of "_d" — the doc ID follows
    let pos = first_segment.rfind("_d")?;
    let doc_id = &first_segment[pos + 2..];

    // Validate: non-empty, alphanumeric only
    if doc_id.is_empty() || !doc_id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }

    Some(doc_id.to_string())
}

/// Convert a browser URL or `coda://` URI to a `coda://` URI.
///
/// If already a `coda://` URI, returns as-is.
/// If a browser URL, parses the doc ID and constructs `coda://docs/{docId}`.
pub fn resolve_doc_input(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.starts_with("coda://") {
        return Ok(input.to_string());
    }
    match parse_coda_url(input) {
        Some(doc_id) => Ok(format!("coda://docs/{doc_id}")),
        None => Err(format!(
            "Could not parse Coda document URL: {input}\n\
             Expected: https://coda.io/d/Title_dDocId or coda://docs/DocId"
        )),
    }
}

/// Extract a short ID from a Coda URI, stripping common prefixes.
///
/// e.g. "coda://docs/AbCdEfGhIj" → "AbCdEfGhIj"
/// e.g. "coda://docs/abc/canvases/canvas-KOsNISRf_L" → "KOsNISRf_L"
/// e.g. "coda://docs/abc/tables/grid-k4rNzi0nC9" → "k4rNzi0nC9"
pub fn id_from_uri(uri: &str) -> &str {
    let raw = uri.rsplit('/').next().unwrap_or(uri);
    // Strip common Coda URI type prefixes to get the unique portion
    raw.strip_prefix("canvas-")
        .or_else(|| raw.strip_prefix("grid-"))
        .or_else(|| raw.strip_prefix("section-"))
        .unwrap_or(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_slugify() {
        assert_eq!(
            slugify("Q2 Planning Doc", "AbCdEf"),
            "q2-planning-doc-abcdef"
        );
    }

    #[test]
    fn special_characters() {
        assert_eq!(
            slugify("My Doc!!! (v2) — final", "X1Y2Z3"),
            "my-doc-v2-final-x1y2z3"
        );
    }

    #[test]
    fn empty_name() {
        assert_eq!(slugify("", "abc123"), "untitled-abc123");
    }

    #[test]
    fn empty_id() {
        assert_eq!(slugify("Hello World", ""), "hello-world");
    }

    #[test]
    fn long_name_truncated() {
        let long_name = "a".repeat(100);
        let slug = slugify(&long_name, "abc123");
        // 80 chars + hyphen + 6 char suffix = 87
        assert!(slug.len() <= 87, "slug too long: {} chars", slug.len());
        assert!(slug.ends_with("-abc123"));
    }

    #[test]
    fn consecutive_special_chars() {
        assert_eq!(slugify("hello---world   test", "ab"), "hello-world-test-ab");
    }

    #[test]
    fn id_from_uri_doc() {
        assert_eq!(id_from_uri("coda://docs/AbCdEf"), "AbCdEf");
    }

    #[test]
    fn id_from_uri_page() {
        assert_eq!(id_from_uri("coda://docs/abc/pages/xyz"), "xyz");
    }

    #[test]
    fn id_from_uri_strips_canvas_prefix() {
        assert_eq!(
            id_from_uri("coda://docs/abc/canvases/canvas-KOsNISRf_L"),
            "KOsNISRf_L"
        );
    }

    #[test]
    fn id_from_uri_strips_grid_prefix() {
        assert_eq!(
            id_from_uri("coda://docs/abc/tables/grid-k4rNzi0nC9"),
            "k4rNzi0nC9"
        );
    }

    #[test]
    fn id_from_uri_strips_section_prefix() {
        assert_eq!(
            id_from_uri("coda://docs/abc/pages/section-3Z3gQv17hj"),
            "3Z3gQv17hj"
        );
    }

    // -- URL parsing tests --

    #[test]
    fn parse_url_basic() {
        assert_eq!(
            parse_coda_url("https://coda.io/d/My-Doc_dAbCdEf"),
            Some("AbCdEf".into())
        );
    }

    #[test]
    fn parse_url_with_page() {
        assert_eq!(
            parse_coda_url("https://coda.io/d/My-Doc_dAbCdEf/Page_suXyZ"),
            Some("AbCdEf".into())
        );
    }

    #[test]
    fn parse_url_no_title() {
        assert_eq!(
            parse_coda_url("https://coda.io/d/_dAbCdEf"),
            Some("AbCdEf".into())
        );
    }

    #[test]
    fn parse_url_trailing_slash() {
        assert_eq!(
            parse_coda_url("https://coda.io/d/My-Doc_dAbCdEf/"),
            Some("AbCdEf".into())
        );
    }

    #[test]
    fn parse_url_http() {
        assert_eq!(
            parse_coda_url("http://coda.io/d/My-Doc_dAbCdEf"),
            Some("AbCdEf".into())
        );
    }

    #[test]
    fn parse_url_not_coda() {
        assert_eq!(parse_coda_url("https://google.com/d/My-Doc_dAbCdEf"), None);
    }

    #[test]
    fn parse_url_no_d_prefix() {
        assert_eq!(parse_coda_url("https://coda.io/d/Bad_URL"), None);
    }

    #[test]
    fn parse_url_empty_id() {
        assert_eq!(parse_coda_url("https://coda.io/d/My-Doc_d"), None);
    }

    #[test]
    fn resolve_input_uri_passthrough() {
        assert_eq!(
            resolve_doc_input("coda://docs/abc").unwrap(),
            "coda://docs/abc"
        );
    }

    #[test]
    fn resolve_input_url_conversion() {
        assert_eq!(
            resolve_doc_input("https://coda.io/d/My-Doc_dAbCdEf").unwrap(),
            "coda://docs/AbCdEf"
        );
    }

    #[test]
    fn resolve_input_invalid_url() {
        assert!(resolve_doc_input("https://google.com/stuff").is_err());
    }
}
