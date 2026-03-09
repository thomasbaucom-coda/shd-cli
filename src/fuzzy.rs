use crate::error::{CodaError, Result};
use serde_json::Value;

/// Score how well a query matches a tool name and description.
/// Returns 0.0 to 1.0.
pub fn score(query: &str, tool_name: &str, description: &str) -> f64 {
    let query_lower = query.to_lowercase();
    let name_lower = tool_name.to_lowercase();

    // Exact name match
    if query_lower == name_lower {
        return 1.0;
    }

    // Name contains query as substring
    if name_lower.contains(&query_lower) {
        return 0.8;
    }

    // Word overlap scoring
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();
    if query_words.is_empty() {
        return 0.0;
    }

    let name_words: Vec<String> = name_lower
        .split('_')
        .chain(name_lower.split('-'))
        .map(|s| s.to_string())
        .collect();
    let desc_lower = description.to_lowercase();
    let desc_words: Vec<&str> = desc_lower.split_whitespace().collect();

    let mut matches = 0usize;
    for qw in &query_words {
        let in_name = name_words.iter().any(|nw| nw.contains(qw));
        let in_desc = desc_words.iter().any(|dw| dw.contains(qw));
        if in_name || in_desc {
            matches += 1;
        }
    }

    let ratio = matches as f64 / query_words.len() as f64;
    ratio * 0.6
}

/// Find top matches for a query across all tools.
/// Returns (name, score, description) sorted descending by score.
pub fn find_matches(query: &str, tools: &[Value], limit: usize) -> Vec<(String, f64, String)> {
    let mut scored: Vec<(String, f64, String)> = tools
        .iter()
        .filter_map(|t| {
            let name = t.get("name")?.as_str()?;
            let desc = t.get("description").and_then(|d| d.as_str()).unwrap_or("");
            let s = score(query, name, desc);
            if s > 0.0 {
                Some((name.to_string(), s, desc.to_string()))
            } else {
                None
            }
        })
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);
    scored
}

/// Resolve a fuzzy query to a single tool name.
/// Succeeds if the top match is confident (score > 0.5 and gap > 0.2 to second).
/// Otherwise prints top 5 candidates to stderr and returns an error.
pub fn resolve(query: &str, tools: &[Value]) -> Result<String> {
    let matches = find_matches(query, tools, 5);

    if matches.is_empty() {
        return Err(CodaError::Validation(format!(
            "No tools matching '{query}'. Run `shd discover` to see available tools."
        )));
    }

    let (ref top_name, top_score, _) = matches[0];
    let second_score = matches.get(1).map(|m| m.1).unwrap_or(0.0);

    // Confidence thresholds:
    // - top_score > 0.5: ensures at least partial word overlap (0.6 * ratio) or better.
    //   Pure word-overlap with all words matching scores 0.6, so 0.5 filters out weak
    //   partial matches while still accepting good word-overlap hits.
    // - gap > 0.2: ensures the top match is meaningfully better than the runner-up.
    //   The scoring tiers are: exact (1.0), substring (0.8), word-overlap (up to 0.6).
    //   A 0.2 gap separates these tiers, so a substring match (0.8) confidently beats
    //   a word-overlap match (0.6), but two substring matches (both 0.8) are ambiguous.
    if top_score > 0.5 && (top_score - second_score) > 0.2 {
        return Ok(top_name.clone());
    }

    // Ambiguous — show candidates
    crate::output::info(&format!("Ambiguous query '{query}'. Did you mean:\n"));
    for (name, s, desc) in &matches {
        let desc_short = if desc.chars().count() > 50 {
            let truncated: String = desc.chars().take(47).collect();
            format!("{truncated}...")
        } else {
            desc.clone()
        };
        crate::output::info(&format!("  {name:30} (score: {s:.2}) {desc_short}\n"));
    }

    Err(CodaError::Validation(format!(
        "Ambiguous tool query '{query}'. Use a more specific name or run `shd discover`."
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_tools() -> Vec<Value> {
        vec![
            json!({"name": "table_create", "description": "Create a new table in a doc"}),
            json!({"name": "table_add_rows", "description": "Add rows to a table"}),
            json!({"name": "doc_create", "description": "Create a new doc"}),
            json!({"name": "whoami", "description": "Get current user info"}),
        ]
    }

    #[test]
    fn exact_match_scores_highest() {
        let s = score("whoami", "whoami", "Get current user info");
        assert!((s - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn substring_match() {
        let s = score("create", "table_create", "Create a new table");
        assert!(s >= 0.7, "substring match should score high: {s}");
    }

    #[test]
    fn word_overlap() {
        let s = score("add rows", "table_add_rows", "Add rows to a table");
        assert!(s > 0.0, "word overlap should score > 0: {s}");
    }

    #[test]
    fn create_table_matches_table_create() {
        let tools = test_tools();
        let matches = find_matches("create table", &tools, 5);
        assert!(!matches.is_empty());
        // table_create should be in results
        assert!(matches.iter().any(|(name, _, _)| name == "table_create"));
    }

    #[test]
    fn resolve_exact_succeeds() {
        let tools = test_tools();
        let result = resolve("whoami", &tools);
        assert_eq!(result.unwrap(), "whoami");
    }

    #[test]
    fn resolve_ambiguous_fails() {
        let tools = test_tools();
        // "create" is ambiguous between table_create and doc_create
        let result = resolve("create", &tools);
        assert!(result.is_err());
    }
}
