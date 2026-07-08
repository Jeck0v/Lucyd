//! Conformance test: the generated OpenAPI document must validate against the
//! official OpenAPI 3.1 document meta-schema.
//!
//! The meta-schema fixture is the vendored "without schema validation" variant
//! of the OpenAPI 3.1 schema; it is self-contained (only internal `#/$defs`
//! refs plus the draft-2020-12 dialect, both resolved by `jsonschema` without
//! any network access).

use lucy_core::openapi::generate_openapi_document;
use lucy_core::registry::EndpointRegistry;
use lucy_types::endpoint::{EndpointMeta, Protocol};
use serde_json::{Value, json};

/// The vendored OpenAPI 3.1 document meta-schema.
const OAS_31_META_SCHEMA: &str = include_str!("fixtures/oas-3.1-base-schema.json");

/// Builds an HTTP endpoint with a method set.
fn http_endpoint(name: &str, method: &str, path: &str) -> EndpointMeta {
    let mut meta = EndpointMeta::new(name, path, Protocol::Http);
    meta.method = Some(method.to_string());
    meta
}

#[test]
fn generated_document_conforms_to_openapi_31_meta_schema() {
    let mut registry = EndpointRegistry::new();

    // A plain GET with no schemas.
    let mut health = http_endpoint("health", "GET", "/health");
    health.description = Some("Service health check".to_string());
    health.tags = vec!["system".to_string()];
    registry.register(health);

    // A POST with nested request + response schemas (each with a definition).
    let mut create = http_endpoint("create_user", "POST", "/api/users/{id}");
    create.request_schema = Some(json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "CreateUserRequest",
        "type": "object",
        "properties": { "address": { "$ref": "#/definitions/Address" } },
        "definitions": {
            "Address": {
                "$schema": "http://json-schema.org/draft-07/schema#",
                "type": "object",
                "properties": { "city": { "type": "string" } }
            }
        }
    }));
    create.response_schema = Some(json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "User",
        "type": "object",
        "properties": { "id": { "type": "integer" } }
    }));
    registry.register(create);

    // A WebSocket endpoint that must be excluded without breaking generation.
    registry.register(EndpointMeta::new(
        "events",
        "/ws/events",
        Protocol::WebSocket,
    ));

    let document = generate_openapi_document(&registry);

    let meta_schema: Value =
        serde_json::from_str(OAS_31_META_SCHEMA).expect("meta-schema fixture must be valid JSON");
    let validator =
        jsonschema::validator_for(&meta_schema).expect("meta-schema must compile to a validator");

    let errors: Vec<String> = validator
        .iter_errors(&document)
        .map(|error| error.to_string())
        .collect();

    assert!(
        errors.is_empty(),
        "generated OpenAPI document must conform to the 3.1 meta-schema, but got:\n{}",
        errors.join("\n")
    );
}
