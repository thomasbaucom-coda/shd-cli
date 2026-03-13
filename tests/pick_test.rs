use serde_json::json;

#[test]
fn pick_multi_no_collision_uses_short_keys() {
    let paths = ["name", "email"];
    let vals = [json!("Alice"), json!("alice@example.com")];
    let refs: Vec<&serde_json::Value> = vals.iter().collect();
    let result = coda_cli::output::build_picked_object(&paths, &refs);
    assert_eq!(result["name"], "Alice");
    assert_eq!(result["email"], "alice@example.com");
}

#[test]
fn pick_multi_collision_uses_full_paths() {
    let paths = ["pages.0.title", "pages.1.title"];
    let vals = [json!("Goals"), json!("Tasks")];
    let refs: Vec<&serde_json::Value> = vals.iter().collect();
    let result = coda_cli::output::build_picked_object(&paths, &refs);
    assert_eq!(result["pages.0.title"], "Goals");
    assert_eq!(result["pages.1.title"], "Tasks");
}
