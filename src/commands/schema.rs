use crate::error::{CodaError, Result};
use serde_json::Value;

/// The OpenAPI spec is embedded at compile time for zero-network-cost introspection.
const OPENAPI_SPEC: &str = include_str!("../../openapi/v1.json");

/// Handle `coda schema <path>` command.
/// Path format: `<resource>.<method>` e.g. `rows.list`, `docs.create`, `pages.get`
///
/// Maps resource.method to the OpenAPI endpoint and returns parameter info,
/// request body schema, and response schema as structured JSON.
pub fn handle(path: &str) -> Result<()> {
    let spec: Value = serde_json::from_str(OPENAPI_SPEC)
        .map_err(|e| CodaError::Other(format!("Failed to parse embedded OpenAPI spec: {e}")))?;

    let parts: Vec<&str> = path.split('.').collect();

    if parts.len() == 1 && parts[0] == "list" {
        return list_resources(&spec);
    }

    if parts.len() == 1 {
        return describe_resource(&spec, parts[0]);
    }

    if parts.len() == 2 {
        return describe_method(&spec, parts[0], parts[1]);
    }

    Err(CodaError::Validation(format!(
        "Schema path must be 'list', '<resource>', or '<resource>.<method>'. Got: '{path}'"
    )))
}

fn list_resources(spec: &Value) -> Result<()> {
    let paths = spec.get("paths").and_then(|p| p.as_object())
        .ok_or_else(|| CodaError::Other("No paths in OpenAPI spec".into()))?;

    let mut resources: Vec<String> = Vec::new();
    for path in paths.keys() {
        // Extract resource name from path like /docs, /docs/{docId}/tables, etc.
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty() && !s.starts_with('{')).collect();
        if let Some(resource) = segments.last() {
            let name = resource.to_string();
            if !resources.contains(&name) {
                resources.push(name);
            }
        }
    }

    resources.sort();
    let output = serde_json::json!({
        "resources": resources,
        "usage": "coda schema <resource> — list methods for a resource\ncoda schema <resource>.<method> — show parameters and schemas"
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn describe_resource(spec: &Value, resource: &str) -> Result<()> {
    let paths = spec.get("paths").and_then(|p| p.as_object())
        .ok_or_else(|| CodaError::Other("No paths in OpenAPI spec".into()))?;

    let mut methods: Vec<Value> = Vec::new();

    for (path, path_item) in paths {
        // Match paths that end with the resource name or contain it
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let matches = segments.iter().any(|s| {
            let clean = s.trim_start_matches('{').trim_end_matches('}');
            clean.eq_ignore_ascii_case(resource)
        });

        if !matches {
            continue;
        }

        if let Some(obj) = path_item.as_object() {
            for (http_method, operation) in obj {
                if !["get", "post", "put", "patch", "delete"].contains(&http_method.as_str()) {
                    continue;
                }
                let op_id = operation.get("operationId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let summary = operation.get("summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                methods.push(serde_json::json!({
                    "method": http_method.to_uppercase(),
                    "path": path,
                    "operationId": op_id,
                    "summary": summary,
                }));
            }
        }
    }

    if methods.is_empty() {
        return Err(CodaError::Validation(format!(
            "No methods found for resource '{resource}'. Run `coda schema list` to see available resources."
        )));
    }

    let output = serde_json::json!({
        "resource": resource,
        "methods": methods,
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn describe_method(spec: &Value, resource: &str, method_name: &str) -> Result<()> {
    let paths = spec.get("paths").and_then(|p| p.as_object())
        .ok_or_else(|| CodaError::Other("No paths in OpenAPI spec".into()))?;

    let target_op = build_operation_id(resource, method_name);

    // Search for matching operation
    for (path, path_item) in paths {
        if let Some(obj) = path_item.as_object() {
            for (http_method, operation) in obj {
                let op_id = operation.get("operationId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if !op_id.eq_ignore_ascii_case(&target_op) {
                    continue;
                }

                // Found the operation — build the schema output
                let mut result = serde_json::json!({
                    "operationId": op_id,
                    "httpMethod": http_method.to_uppercase(),
                    "path": path,
                });

                // Parameters
                if let Some(params) = operation.get("parameters").and_then(|v| v.as_array()) {
                    let param_info: Vec<Value> = params.iter().map(|p| {
                        let resolved = resolve_ref(spec, p);
                        serde_json::json!({
                            "name": resolved.get("name").and_then(|v| v.as_str()).unwrap_or("?"),
                            "in": resolved.get("in").and_then(|v| v.as_str()).unwrap_or("?"),
                            "required": resolved.get("required").and_then(|v| v.as_bool()).unwrap_or(false),
                            "description": resolved.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                            "schema": resolved.get("schema"),
                        })
                    }).collect();
                    result["parameters"] = Value::Array(param_info);
                }

                // Request body
                if let Some(body) = operation.get("requestBody") {
                    let resolved = resolve_ref(spec, body);
                    if let Some(content) = resolved.get("content") {
                        if let Some(json_schema) = content.get("application/json").and_then(|c| c.get("schema")) {
                            let resolved_schema = resolve_ref(spec, json_schema);
                            result["requestBody"] = resolved_schema;
                        }
                    }
                }

                // Response
                if let Some(responses) = operation.get("responses").and_then(|v| v.as_object()) {
                    if let Some(success) = responses.get("200").or(responses.get("201")).or(responses.get("202")) {
                        if let Some(content) = success.get("content") {
                            if let Some(json_schema) = content.get("application/json").and_then(|c| c.get("schema")) {
                                let resolved_schema = resolve_ref(spec, json_schema);
                                result["responseSchema"] = resolved_schema;
                            }
                        }
                    }
                }

                // Summary/description
                if let Some(summary) = operation.get("summary").and_then(|v| v.as_str()) {
                    result["summary"] = Value::String(summary.to_string());
                }

                println!("{}", serde_json::to_string_pretty(&result)?);
                return Ok(());
            }
        }
    }

    Err(CodaError::Validation(format!(
        "No operation found for '{resource}.{method_name}'. Run `coda schema {resource}` to see available methods."
    )))
}

/// Resolve a JSON $ref pointer one level deep.
fn resolve_ref<'a>(spec: &'a Value, value: &'a Value) -> Value {
    if let Some(ref_path) = value.get("$ref").and_then(|v| v.as_str()) {
        // Parse #/components/schemas/Foo style refs
        let parts: Vec<&str> = ref_path.trim_start_matches('#').split('/').filter(|s| !s.is_empty()).collect();
        let mut current = spec;
        for part in &parts {
            current = current.get(*part).unwrap_or(&Value::Null);
        }
        if current != &Value::Null {
            return current.clone();
        }
    }
    value.clone()
}

/// Build the expected operationId from a resource name and method name.
/// Used by both the schema command and the MCP server's schema tool.
pub fn build_operation_id(resource: &str, method_name: &str) -> String {
    match method_name {
        "list" => format!("list{}", capitalize(resource)),
        "get" => format!("get{}", capitalize(&singularize(resource))),
        "create" => format!("create{}", capitalize(&singularize(resource))),
        "update" => format!("update{}", capitalize(&singularize(resource))),
        "delete" => format!("delete{}", capitalize(&singularize(resource))),
        "upsert" => format!("upsert{}", capitalize(resource)),
        other => other.to_string(),
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

fn singularize(s: &str) -> String {
    if let Some(prefix) = s.strip_suffix("ies") {
        format!("{prefix}y")
    } else if s.ends_with('s') && !s.ends_with("ss") {
        s[..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}
