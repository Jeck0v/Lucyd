//! Builds the OpenAPI `paths` object: one Path Item + Operation per
//! registered [`Protocol::Http`] endpoint.

use super::components::ComponentSchemas;
use crate::registry::EndpointRegistry;
use lucy_types::endpoint::{EndpointMeta, Protocol};
use serde_json::{Map, Value, json};
use std::collections::HashSet;

/// Builds the `paths` object by walking the registry in registration order,
/// keeping only [`Protocol::Http`] endpoints.
pub(super) fn collect_paths(
    registry: &EndpointRegistry,
    components: &mut ComponentSchemas,
) -> Map<String, Value> {
    let mut paths: Map<String, Value> = Map::new();
    let mut used_operation_ids: HashSet<String> = HashSet::new();

    for endpoint in registry.all() {
        // WebSocket / MQTT endpoints are not representable in OpenAPI 3.1.
        if endpoint.protocol != Protocol::Http {
            continue;
        }
        // An HTTP endpoint always carries a method in practice; if one is
        // somehow missing there is no valid path-item verb to place it under,
        // so skip it rather than panicking on an unexpected shape.
        let Some(method) = endpoint.method.as_deref() else {
            continue;
        };

        let operation = build_operation(endpoint, components, &mut used_operation_ids);
        insert_operation(&mut paths, &endpoint.path, method, operation);
    }

    paths
}

/// Inserts `operation` under `paths[path][method]`, creating the Path Item
/// object on first use.
///
/// Two endpoints sharing an exact (path, method) pair: the later one
/// overwrites the earlier — an accepted, documented edge case.
fn insert_operation(paths: &mut Map<String, Value>, path: &str, method: &str, operation: Value) {
    paths
        .entry(path.to_string())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .expect("path_item is always constructed as Value::Object above")
        .insert(method.to_lowercase(), operation);
}

/// Builds a single Operation Object for one HTTP endpoint.
fn build_operation(
    endpoint: &EndpointMeta,
    components: &mut ComponentSchemas,
    used_operation_ids: &mut HashSet<String>,
) -> Value {
    let mut operation = Map::new();

    let operation_id = unique_operation_id(&endpoint.name, used_operation_ids);
    operation.insert("operationId".to_string(), Value::String(operation_id));
    insert_operation_metadata(&mut operation, endpoint);

    if let Some(request_body) = build_request_body(endpoint, components) {
        operation.insert("requestBody".to_string(), request_body);
    }
    operation.insert(
        "responses".to_string(),
        build_responses(endpoint, components),
    );

    Value::Object(operation)
}

/// Inserts `description`, `tags`, and path `parameters` into `operation`,
/// omitting each key the endpoint doesn't provide.
fn insert_operation_metadata(operation: &mut Map<String, Value>, endpoint: &EndpointMeta) {
    if let Some(description) = &endpoint.description {
        operation.insert(
            "description".to_string(),
            Value::String(description.clone()),
        );
    }

    if !endpoint.tags.is_empty() {
        operation.insert("tags".to_string(), json!(endpoint.tags));
    }

    let parameters = path_parameters(&endpoint.path);
    if !parameters.is_empty() {
        operation.insert("parameters".to_string(), Value::Array(parameters));
    }
}

/// Builds the Request Body Object for an endpoint with a request schema, or
/// `None` when the endpoint declares no request body.
fn build_request_body(endpoint: &EndpointMeta, components: &mut ComponentSchemas) -> Option<Value> {
    let request_schema = endpoint.request_schema.as_ref()?;
    let name = components.hoist_root_schema(request_schema, &format!("{}_request", endpoint.name));
    Some(json!({
        "required": true,
        "content": {
            "application/json": {
                "schema": { "$ref": schema_ref(&name) }
            }
        }
    }))
}

/// Builds the `responses` object, which always carries a `200` entry
/// (OpenAPI requires `responses` to be non-empty).
fn build_responses(endpoint: &EndpointMeta, components: &mut ComponentSchemas) -> Value {
    let mut ok = Map::new();
    ok.insert(
        "description".to_string(),
        Value::String("Successful response".to_string()),
    );

    if let Some(response_schema) = &endpoint.response_schema {
        let name =
            components.hoist_root_schema(response_schema, &format!("{}_response", endpoint.name));
        ok.insert(
            "content".to_string(),
            json!({
                "application/json": {
                    "schema": { "$ref": schema_ref(&name) }
                }
            }),
        );
    }

    let mut responses = Map::new();
    responses.insert("200".to_string(), Value::Object(ok));
    Value::Object(responses)
}

/// Formats a `components.schemas` reference for a given schema name.
fn schema_ref(name: &str) -> String {
    format!("#/components/schemas/{name}")
}

/// Returns a document-unique `operationId` derived from `base`.
///
/// The registry is append-only, so iterating in registration order makes the
/// suffixing deterministic: the first `base` wins, later collisions become
/// `base_2`, `base_3`, and so on.
fn unique_operation_id(base: &str, used: &mut HashSet<String>) -> String {
    if used.insert(base.to_string()) {
        return base.to_string();
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

/// Extracts axum-style `{name}` path parameters from a URL path.
///
/// Emits one Parameter Object per clean `{name}` segment; returns an empty
/// vector when the path has no parameters (the caller then omits
/// `parameters`). Malformed segments (residual braces, e.g. a typo'd
/// `{na{me}`) and axum's `{*name}` catch-all syntax are skipped rather than
/// emitting an invalid or misleading Parameter Object — OpenAPI's path
/// templating has no equivalent to a wildcard remainder-of-path match, so a
/// catch-all segment is a known, undocumented gap (see `docs/11-limitations.md`)
/// rather than something we can represent faithfully.
fn path_parameters(path: &str) -> Vec<Value> {
    path.split('/')
        .filter_map(|segment| {
            let name = segment.strip_prefix('{')?.strip_suffix('}')?;
            if name.is_empty() || name.contains(['{', '}']) || name.starts_with('*') {
                return None;
            }
            Some(json!({
                "name": name,
                "in": "path",
                "required": true,
                "schema": { "type": "string" }
            }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::openapi::generate_openapi_document;

    #[test]
    fn non_http_endpoints_are_excluded() {
        let mut registry = EndpointRegistry::new();
        registry.register(http_endpoint(HEALTH_NAME, "GET", HEALTH_PATH));
        registry.register(EndpointMeta::new(
            "events",
            "/ws/events",
            Protocol::WebSocket,
        ));
        registry.register(EndpointMeta::new(
            "temperature",
            "sensors/temperature",
            Protocol::Mqtt,
        ));

        let doc = generate_openapi_document(&registry);
        let paths = doc["paths"].as_object().expect("paths must be an object");

        assert_eq!(paths.len(), 1, "only the HTTP endpoint may appear in paths");
        assert!(paths.contains_key(HEALTH_PATH));
    }

    #[test]
    fn endpoint_without_schemas_has_no_request_body() {
        let mut registry = EndpointRegistry::new();
        registry.register(http_endpoint(HEALTH_NAME, "GET", HEALTH_PATH));

        let doc = generate_openapi_document(&registry);
        let operation = &doc["paths"][HEALTH_PATH]["get"];

        assert!(
            operation.get("requestBody").is_none(),
            "an endpoint without a request schema must not emit requestBody"
        );
        assert_eq!(
            operation["responses"]["200"]["description"], "Successful response",
            "the 200 response must always carry a description"
        );
        assert!(
            operation["responses"]["200"].get("content").is_none(),
            "a schemaless response must not emit a content key"
        );
    }

    #[test]
    fn duplicate_endpoint_names_get_unique_operation_ids() {
        let mut registry = EndpointRegistry::new();
        registry.register(http_endpoint("dup", "GET", "/a"));
        registry.register(http_endpoint("dup", "GET", "/b"));

        let doc = generate_openapi_document(&registry);

        assert_eq!(doc["paths"]["/a"]["get"]["operationId"], "dup");
        assert_eq!(
            doc["paths"]["/b"]["get"]["operationId"], "dup_2",
            "a colliding operationId must be suffixed"
        );
    }

    #[test]
    fn brace_segments_become_path_parameters() {
        let mut registry = EndpointRegistry::new();
        registry.register(http_endpoint("get_user", "GET", "/api/users/{id}"));

        let doc = generate_openapi_document(&registry);
        let parameters = doc["paths"]["/api/users/{id}"]["get"]["parameters"]
            .as_array()
            .expect("a braced path must emit a parameters array");

        assert_eq!(parameters.len(), 1);
        assert_eq!(parameters[0]["name"], "id");
        assert_eq!(parameters[0]["in"], "path");
        assert_eq!(parameters[0]["required"], true);
        assert_eq!(parameters[0]["schema"]["type"], "string");
    }

    #[test]
    fn path_without_braces_omits_parameters() {
        let mut registry = EndpointRegistry::new();
        registry.register(http_endpoint(HEALTH_NAME, "GET", HEALTH_PATH));

        let doc = generate_openapi_document(&registry);

        assert!(
            doc["paths"][HEALTH_PATH]["get"].get("parameters").is_none(),
            "a path with no braces must not emit a parameters key"
        );
    }

    #[test]
    fn catch_all_and_malformed_segments_are_skipped() {
        let mut registry = EndpointRegistry::new();
        registry.register(http_endpoint("serve_file", "GET", "/files/{*rest}"));
        registry.register(http_endpoint("weird", "GET", "/oops/{na{me}"));

        let doc = generate_openapi_document(&registry);

        assert!(
            doc["paths"]["/files/{*rest}"]["get"]
                .get("parameters")
                .is_none(),
            "axum's catch-all `{{*name}}` syntax has no OpenAPI equivalent and must not \
             produce a parameter named \"*rest\""
        );
        assert!(
            doc["paths"]["/oops/{na{me}"]["get"]
                .get("parameters")
                .is_none(),
            "a malformed segment with residual braces must not produce a bogus parameter"
        );
    }
}
