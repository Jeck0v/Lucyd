//! Walks an OpenAPI document's `paths` object into a flat list of
//! [`ImportOperation`]s ready for code generation.
//!
//! `$ref`s are resolved here only for parameter/request-body/response
//! *containers* (i.e. reusable `#/components/parameters/...`,
//! `#/components/requestBodies/...`, `#/components/responses/...` entries) —
//! resolving these is what lets a single operation's request/response
//! *schema* `Value` be handed to [`super::rust_type::TypeGenerator`]
//! unresolved; the generator resolves `#/components/schemas/...` refs itself
//! so it can cache and dedup by component name.

use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashSet};

/// HTTP verbs recognized as Path Item Object keys, in the order OpenAPI 3.1
/// lists them.
const HTTP_METHODS: &[&str] = &[
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

/// A single OpenAPI operation, flattened and ready for [`super::codegen`].
///
/// `request_schema`/`response_schema` are raw, unresolved-at-the-leaf JSON
/// Schema `Value`s (they may themselves be or contain `$ref`s into
/// `#/components/schemas/...`) — [`super::rust_type::TypeGenerator`] is the
/// one place that maps those to Rust types.
pub struct ImportOperation {
    /// From `operationId`, or derived as `{method}_{path_slug}` when absent.
    pub operation_id: String,
    /// Uppercase HTTP verb (`"GET"`, `"POST"`, ...), matching what
    /// `#[lucyd_http(method = ...)]` expects.
    pub method: String,
    /// The OpenAPI path template, e.g. `"/api/users/{id}"`.
    pub path: String,
    /// `description` (falling back to `summary`), if either is present.
    pub description: Option<String>,
    /// `tags`, in document order.
    pub tags: Vec<String>,
    /// Names of `in: path` parameters — surfaced as a doc comment only;
    /// `#[lucyd_http]` has no argument that binds them, since the path template
    /// already names them.
    pub path_params: Vec<String>,
    /// The `in: query` parameters folded into one object schema, which
    /// [`super::codegen`] turns into a struct bound by `#[lucyd_http(query = T)]`.
    pub query_schema: Option<Value>,
    /// The `application/json` request body schema, if any.
    pub request_schema: Option<Value>,
    /// The `application/json` schema of the first 2xx (or `default`)
    /// response, if any.
    pub response_schema: Option<Value>,
    /// `Some(reason)` when this operation can't be represented and must be
    /// skipped by `codegen.rs`. Skipped operations are still reported (by
    /// `summary.rs`), just not turned into code.
    pub skip_reason: Option<String>,
}

/// Extracts every operation from `document["paths"]`, in document order.
pub fn extract_operations(document: &Value) -> Vec<ImportOperation> {
    let mut operations = Vec::new();
    let mut used_ids: HashSet<String> = HashSet::new();

    let Some(paths) = document.get("paths").and_then(Value::as_object) else {
        return operations;
    };

    for (path, path_item) in paths {
        let Some(path_item_obj) = path_item.as_object() else {
            continue;
        };
        let path_level_params = path_item_obj.get("parameters");

        for method in HTTP_METHODS {
            let Some(operation) = path_item_obj.get(*method).and_then(Value::as_object) else {
                continue;
            };
            operations.push(build_operation(
                document,
                path,
                method,
                operation,
                path_level_params,
                &mut used_ids,
            ));
        }
    }

    operations
}

/// Builds a single [`ImportOperation`], flagging `skip_reason` rather than
/// aborting when the operation uses an unsupported shape.
fn build_operation(
    document: &Value,
    path: &str,
    method: &str,
    operation: &Map<String, Value>,
    path_level_params: Option<&Value>,
    used_ids: &mut HashSet<String>,
) -> ImportOperation {
    let operation_id = unique_operation_id(
        operation.get("operationId").and_then(Value::as_str),
        method,
        path,
        used_ids,
    );
    let description = operation
        .get("description")
        .and_then(Value::as_str)
        .or_else(|| operation.get("summary").and_then(Value::as_str))
        .map(String::from);
    let tags = operation
        .get("tags")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    let base = ImportOperation {
        operation_id,
        method: method.to_uppercase(),
        path: path.to_string(),
        description,
        tags,
        path_params: Vec::new(),
        query_schema: None,
        request_schema: None,
        response_schema: None,
        skip_reason: None,
    };

    if operation.contains_key("callbacks") || operation.contains_key("links") {
        return ImportOperation {
            skip_reason: Some("uses `callbacks`/`links`, which are not supported".to_string()),
            ..base
        };
    }

    let (path_params, query_schema) =
        match collect_parameters(document, path_level_params, operation.get("parameters")) {
            Ok(params) => params,
            Err(reason) => {
                return ImportOperation {
                    skip_reason: Some(reason),
                    ..base
                }
            }
        };

    let request_schema = match request_schema(document, operation.get("requestBody")) {
        Ok(schema) => schema,
        Err(reason) => {
            return ImportOperation {
                skip_reason: Some(reason),
                ..base
            }
        }
    };
    let response_schema = match response_schema(document, operation.get("responses")) {
        Ok(schema) => schema,
        Err(reason) => {
            return ImportOperation {
                skip_reason: Some(reason),
                ..base
            }
        }
    };

    let mut visited = HashSet::new();
    let uses_unsupported = [
        query_schema.as_ref(),
        request_schema.as_ref(),
        response_schema.as_ref(),
    ]
    .into_iter()
    .flatten()
    .any(|schema| schema_uses_unsupported_composition(document, schema, &mut visited));

    if uses_unsupported {
        return ImportOperation {
            skip_reason: Some(
                "uses `oneOf`/`allOf`/`anyOf`/`not`, which don't map to a single Rust type"
                    .to_string(),
            ),
            ..base
        };
    }

    ImportOperation {
        path_params,
        query_schema,
        request_schema,
        response_schema,
        ..base
    }
}

/// Resolves a `$ref` chain against `document`, erring on a `$ref` outside
/// `#/...` (cross-file refs are an explicit, documented limitation) or one
/// that doesn't resolve to anything.
fn resolve_ref(document: &Value, value: &Value) -> Result<Value, String> {
    let mut current = value.clone();
    let mut hops = 0;
    while let Some(Value::String(reference)) = current.get("$ref").cloned() {
        hops += 1;
        if hops > 32 {
            return Err(format!(
                "`$ref` cycle detected while resolving: {reference}"
            ));
        }
        if !reference.starts_with("#/") {
            return Err(format!(
                "external $ref is not supported (only same-document refs are): {reference}"
            ));
        }
        current = document
            .pointer(&reference[1..])
            .cloned()
            .ok_or_else(|| format!("$ref not found in document: {reference}"))?;
    }
    Ok(current)
}

/// Accumulates `in: query` Parameter Objects into the single object schema
/// that `#[lucyd_http(query = T)]` binds to.
///
/// The result is deliberately the same shape schemars would emit for the
/// equivalent Rust struct — `properties` plus `required` — so
/// [`super::rust_type::TypeGenerator`] can turn it into a struct with no
/// query-specific code path of its own.
#[derive(Default)]
struct QuerySchema {
    properties: Map<String, Value>,
    required: BTreeSet<String>,
}

impl QuerySchema {
    /// Adds one Parameter Object, a later declaration of the same name winning
    /// — which is how OpenAPI defines an operation-level parameter overriding
    /// the path-level one it repeats.
    fn insert(&mut self, name: &str, parameter: &Value) {
        // A parameter declared through `content` rather than `schema` still
        // arrives as text in the URL, so text is the honest default.
        let mut schema = parameter
            .get("schema")
            .cloned()
            .unwrap_or_else(|| json!({ "type": "string" }));

        // Carried onto the property so the generated field keeps the
        // documentation the OpenAPI document gave the parameter.
        if let (Some(object), Some(description)) =
            (schema.as_object_mut(), parameter.get("description"))
        {
            object.insert("description".to_string(), description.clone());
        }

        match parameter.get("required").and_then(Value::as_bool) {
            Some(true) => self.required.insert(name.to_string()),
            _ => self.required.remove(name),
        };
        self.properties.insert(name.to_string(), schema);
    }

    /// The finished schema, or `None` when the operation declared no query
    /// parameter at all — in which case no `query =` argument is emitted.
    fn build(self) -> Option<Value> {
        if self.properties.is_empty() {
            return None;
        }
        Some(json!({
            "type": "object",
            "properties": Value::Object(self.properties),
            "required": self.required.into_iter().collect::<Vec<_>>(),
        }))
    }
}

/// Collects the distinct `in: path` parameter names, and folds the `in: query`
/// ones into a single object schema, from the path-level and operation-level
/// `parameters` arrays combined.
fn collect_parameters(
    document: &Value,
    path_level: Option<&Value>,
    operation_level: Option<&Value>,
) -> Result<(Vec<String>, Option<Value>), String> {
    let mut path_params = Vec::new();
    let mut seen_path = HashSet::new();
    let mut query = QuerySchema::default();

    for params in [path_level, operation_level].into_iter().flatten() {
        let array = params
            .as_array()
            .ok_or_else(|| "`parameters` must be an array".to_string())?;
        for param in array {
            let resolved = resolve_ref(document, param)?;
            let Some(name) = resolved
                .get("name")
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty())
                .map(String::from)
            else {
                continue;
            };
            match resolved.get("in").and_then(Value::as_str) {
                Some("path") if seen_path.insert(name.clone()) => path_params.push(name),
                Some("query") => query.insert(&name, &resolved),
                _ => {}
            }
        }
    }

    Ok((path_params, query.build()))
}

/// Extracts the `application/json` schema from a (possibly `$ref`'d) request
/// body object, if the operation declares one at all.
fn request_schema(document: &Value, request_body: Option<&Value>) -> Result<Option<Value>, String> {
    let Some(request_body) = request_body else {
        return Ok(None);
    };
    let resolved = resolve_ref(document, request_body)?;
    Ok(json_content_schema(&resolved))
}

/// Extracts the `application/json` schema from the first 2xx response (or
/// `default` when no 2xx entry exists), if it declares a body at all.
fn response_schema(document: &Value, responses: Option<&Value>) -> Result<Option<Value>, String> {
    let Some(responses) = responses.and_then(Value::as_object) else {
        return Ok(None);
    };

    const PREFERRED: &[&str] = &["200", "201", "202", "204"];
    let entry = PREFERRED
        .iter()
        .find_map(|code| responses.get(*code))
        .or_else(|| {
            responses
                .iter()
                .find(|(code, _)| code.starts_with('2'))
                .map(|(_, v)| v)
        })
        .or_else(|| responses.get("default"));

    let Some(entry) = entry else {
        return Ok(None);
    };
    let resolved = resolve_ref(document, entry)?;
    Ok(json_content_schema(&resolved))
}

/// Reads `.content["application/json"].schema` off a (already-resolved)
/// Request/Response Body Object.
fn json_content_schema(body: &Value) -> Option<Value> {
    body.get("content")
        .and_then(|content| content.get("application/json"))
        .and_then(|media_type| media_type.get("schema"))
        .cloned()
}

/// Recursively scans `schema` (and, through `$ref`, anything it reaches in
/// `document`'s `components.schemas`) for `oneOf`/`allOf`/`anyOf`/`not`.
///
/// `visited` guards against re-descending into the same named component
/// twice (both to bound the work and to avoid infinite recursion on a
/// self-referential schema).
fn schema_uses_unsupported_composition(
    document: &Value,
    schema: &Value,
    visited: &mut HashSet<String>,
) -> bool {
    match schema {
        Value::Object(map) => {
            if ["oneOf", "allOf", "anyOf", "not"]
                .iter()
                .any(|keyword| map.contains_key(*keyword))
            {
                return true;
            }
            if let Some(Value::String(reference)) = map.get("$ref") {
                return match reference.strip_prefix("#/components/schemas/") {
                    Some(name) => {
                        if !visited.insert(name.to_string()) {
                            return false;
                        }
                        document
                            .pointer(&format!("/components/schemas/{name}"))
                            .is_some_and(|target| {
                                schema_uses_unsupported_composition(document, target, visited)
                            })
                    }
                    None => false,
                };
            }
            map.values()
                .any(|value| schema_uses_unsupported_composition(document, value, visited))
        }
        Value::Array(items) => items
            .iter()
            .any(|value| schema_uses_unsupported_composition(document, value, visited)),
        _ => false,
    }
}

/// Returns a document-unique `operationId`: the explicit one if present,
/// otherwise `{method}_{path_slug}`, suffixed on collision. Mirrors
/// `crates/lucyd-core/src/openapi/paths.rs::unique_operation_id`'s
/// first-wins, `_2`/`_3`/... fallback pattern.
fn unique_operation_id(
    explicit: Option<&str>,
    method: &str,
    path: &str,
    used: &mut HashSet<String>,
) -> String {
    let base = explicit
        .map(String::from)
        .unwrap_or_else(|| format!("{}_{}", method.to_lowercase(), path_slug(path)));

    if used.insert(base.clone()) {
        return base;
    }
    let mut suffix = 2u32;
    loop {
        let candidate = format!("{base}_{suffix}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        suffix += 1;
    }
}

/// Turns a URL path template into an underscore-joined slug: strips `{}`
/// braces and a leading `*` (axum catch-all syntax), and normalizes other
/// separators to `_`.
fn path_slug(path: &str) -> String {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let trimmed = segment
                .trim_start_matches('{')
                .trim_end_matches('}')
                .trim_start_matches('*');
            trimmed.replace(['-', '.'], "_")
        })
        .collect::<Vec<_>>()
        .join("_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const USERS_PATH: &str = "/api/users/{id}";

    fn document_with_paths(paths: Value) -> Value {
        json!({ "openapi": "3.1.0", "paths": paths })
    }

    #[test]
    fn explicit_operation_id_is_used_verbatim() {
        let document = document_with_paths(json!({
            "/health": { "get": { "operationId": "health_check" } }
        }));
        let ops = extract_operations(&document);
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operation_id, "health_check");
        assert_eq!(ops[0].method, "GET");
    }

    #[test]
    fn missing_operation_id_is_derived_from_method_and_path() {
        let document = document_with_paths(json!({
            USERS_PATH: { "get": {} }
        }));
        let ops = extract_operations(&document);
        assert_eq!(ops[0].operation_id, "get_api_users_id");
    }

    #[test]
    fn colliding_derived_ids_are_suffixed() {
        let document = document_with_paths(json!({
            "/a": { "get": { "operationId": "dup" } },
            "/b": { "get": { "operationId": "dup" } }
        }));
        let ops = extract_operations(&document);
        let ids: Vec<&str> = ops.iter().map(|op| op.operation_id.as_str()).collect();
        assert!(ids.contains(&"dup"));
        assert!(ids.contains(&"dup_2"));
    }

    #[test]
    fn callbacks_are_skipped_with_a_reason() {
        let document = document_with_paths(json!({
            "/hook": { "post": { "operationId": "hook", "callbacks": {} } }
        }));
        let ops = extract_operations(&document);
        assert!(ops[0].skip_reason.as_ref().unwrap().contains("callbacks"));
    }

    #[test]
    fn one_of_response_schema_is_skipped_with_a_reason() {
        let document = document_with_paths(json!({
            "/x": {
                "get": {
                    "operationId": "x",
                    "responses": {
                        "200": {
                            "content": {
                                "application/json": {
                                    "schema": { "oneOf": [{ "type": "string" }, { "type": "integer" }] }
                                }
                            }
                        }
                    }
                }
            }
        }));
        let ops = extract_operations(&document);
        assert!(ops[0].skip_reason.as_ref().unwrap().contains("oneOf"));
    }

    #[test]
    fn path_and_query_parameters_are_collected_by_location() {
        let document = document_with_paths(json!({
            USERS_PATH: {
                "parameters": [{ "name": "id", "in": "path" }],
                "get": {
                    "operationId": "get_user",
                    "parameters": [{
                        "name": "verbose",
                        "in": "query",
                        "required": true,
                        "description": "Include the full record.",
                        "schema": { "type": "boolean" }
                    }]
                }
            }
        }));
        let ops = extract_operations(&document);

        assert_eq!(ops[0].path_params, vec!["id".to_string()]);
        assert_eq!(
            ops[0].query_schema,
            Some(json!({
                "type": "object",
                "properties": {
                    "verbose": {
                        "type": "boolean",
                        "description": "Include the full record."
                    }
                },
                "required": ["verbose"]
            })),
            "query parameters must fold into the object schema `query = T` binds to"
        );
    }

    #[test]
    fn an_operation_level_query_parameter_overrides_the_path_level_one() {
        let document = document_with_paths(json!({
            "/users": {
                "parameters": [{
                    "name": "page", "in": "query", "required": true,
                    "schema": { "type": "string" }
                }],
                "get": {
                    "operationId": "list_users",
                    "parameters": [{
                        "name": "page", "in": "query",
                        "schema": { "type": "integer" }
                    }]
                }
            }
        }));
        let schema = extract_operations(&document)[0]
            .query_schema
            .clone()
            .expect("a query parameter must produce a schema");

        assert_eq!(schema["properties"]["page"]["type"], "integer");
        assert_eq!(
            schema["required"],
            json!([]),
            "the operation-level declaration drops the path-level `required`"
        );
    }

    #[test]
    fn a_query_parameter_without_a_schema_defaults_to_a_string() {
        let document = document_with_paths(json!({
            "/users": {
                "get": {
                    "operationId": "list_users",
                    "parameters": [{ "name": "q", "in": "query" }]
                }
            }
        }));
        let schema = extract_operations(&document)[0]
            .query_schema
            .clone()
            .expect("a query parameter must produce a schema");

        assert_eq!(schema["properties"]["q"]["type"], "string");
    }

    #[test]
    fn an_operation_without_query_parameters_has_no_query_schema() {
        let document = document_with_paths(json!({
            USERS_PATH: { "get": { "parameters": [{ "name": "id", "in": "path" }] } }
        }));

        assert!(
            extract_operations(&document)[0].query_schema.is_none(),
            "no `in: query` parameter means no `query =` argument to emit"
        );
    }

    #[test]
    fn request_and_response_schemas_are_extracted() {
        let document = document_with_paths(json!({
            "/users": {
                "post": {
                    "operationId": "create_user",
                    "requestBody": {
                        "content": { "application/json": { "schema": { "type": "object" } } }
                    },
                    "responses": {
                        "201": {
                            "content": { "application/json": { "schema": { "type": "string" } } }
                        }
                    }
                }
            }
        }));
        let ops = extract_operations(&document);
        assert!(ops[0].request_schema.is_some());
        assert_eq!(ops[0].response_schema, Some(json!({ "type": "string" })));
    }

    #[test]
    fn external_ref_in_request_body_is_skipped() {
        let document = document_with_paths(json!({
            "/users": {
                "post": {
                    "operationId": "create_user",
                    "requestBody": { "$ref": "other-file.yaml#/components/requestBodies/Foo" }
                }
            }
        }));
        let ops = extract_operations(&document);
        assert!(ops[0].skip_reason.is_some());
    }
}
