//! OpenAPI 3.1 document generation.
//!
//! Converts the in-memory [`EndpointRegistry`] into an OpenAPI 3.1 JSON
//! document consumable by standard tooling (Swagger UI, `openapi-generator`,
//! `orval`, Postman, ...).
//!
//! This is a **derived, additive** view of the same registry that backs
//! [`crate::spec::generate_spec`]; the internal `/docs/spec.json` format is
//! the source of truth and is left untouched.
//!
//! # Scope
//!
//! Only [`Protocol::Http`] endpoints are represented. `WebSocket` and `Mqtt`
//! endpoints are intentionally excluded — OpenAPI 3.1 has no native way to
//! describe them; a separate future AsyncAPI export will cover those.

use crate::registry::EndpointRegistry;
use components::ComponentSchemas;
use serde_json::{Map, Value, json};

mod components;
mod paths;
mod refs;
#[cfg(test)]
mod test_support;

/// OpenAPI specification version emitted in the `openapi` field.
const OPENAPI_VERSION: &str = "3.1.0";

/// Default `info.title` used until per-application configuration exists.
const DEFAULT_API_TITLE: &str = "Lucyd API";

/// Generates the OpenAPI 3.1 document for a given [`EndpointRegistry`].
///
/// Iterates the registry in registration order, keeping only
/// [`Protocol::Http`] endpoints, and emits a Path Item + Operation for each.
/// Request/response JSON Schemas are hoisted into `components.schemas` and
/// referenced with `$ref`.
///
/// The returned value has roughly the following shape:
///
/// ```json
/// {
///   "openapi": "3.1.0",
///   "info": { "title": "Lucyd API", "version": "0.1.9" },
///   "paths": { "/api/users": { "post": { /* operation */ } } },
///   "components": { "schemas": { "CreateUserRequest": { /* ... */ } } }
/// }
/// ```
pub fn generate_openapi_document(registry: &EndpointRegistry) -> Value {
    build_document(registry, default_info())
}

/// Builds the default `info` object.
///
/// `title` and `version` currently default to fixed values; the split between
/// [`generate_openapi_document`] and [`build_document`] exists so a future
/// config-carrying variant can supply a custom `info` object without changing
/// the public signature.
fn default_info() -> Value {
    json!({
        "title": DEFAULT_API_TITLE,
        "version": env!("CARGO_PKG_VERSION"),
    })
}

/// Assembles the full OpenAPI document from a registry and an `info` object.
fn build_document(registry: &EndpointRegistry, info: Value) -> Value {
    let mut components = ComponentSchemas::default();
    let paths = paths::collect_paths(registry, &mut components);

    let mut document = Map::new();
    document.insert(
        "openapi".to_string(),
        Value::String(OPENAPI_VERSION.to_string()),
    );
    document.insert("info".to_string(), info);
    document.insert("paths".to_string(), Value::Object(paths));

    // Only emit `components` when at least one schema was hoisted.
    if !components.is_empty() {
        let mut wrapper = Map::new();
        wrapper.insert("schemas".to_string(), Value::Object(components.into_map()));
        document.insert("components".to_string(), Value::Object(wrapper));
    }

    Value::Object(document)
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn empty_registry_produces_minimal_document() {
        let registry = EndpointRegistry::new();
        let doc = generate_openapi_document(&registry);

        assert_eq!(doc["openapi"], OPENAPI_VERSION);
        assert_eq!(doc["info"]["title"], DEFAULT_API_TITLE);
        assert!(
            doc["info"]["version"].is_string(),
            "info.version must be populated"
        );
        assert!(
            doc["paths"]
                .as_object()
                .expect("paths must be an object")
                .is_empty(),
            "an empty registry must produce an empty paths object"
        );
        assert!(
            doc.get("components").is_none(),
            "no schemas hoisted means no components key at all"
        );
    }

    #[test]
    fn request_and_response_schemas_are_hoisted() {
        let mut registry = EndpointRegistry::new();
        let mut endpoint = http_endpoint("create_user", "POST", USERS_PATH);
        endpoint.request_schema = Some(json!({
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
        endpoint.response_schema = Some(json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "UserResponse",
            "type": "object",
            "properties": { "role": { "$ref": "#/definitions/Role" } },
            "definitions": {
                "Role": {
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "type": "string"
                }
            }
        }));
        registry.register(endpoint);

        let doc = generate_openapi_document(&registry);
        let operation = &doc["paths"][USERS_PATH]["post"];

        assert_eq!(
            operation["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/CreateUserRequest"
        );
        assert_eq!(
            operation["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/UserResponse"
        );

        let schemas = &doc["components"]["schemas"];
        assert_eq!(
            schemas["CreateUserRequest"]["properties"]["address"]["$ref"],
            "#/components/schemas/Address",
            "the nested ref must be rewritten to the components namespace"
        );
        assert!(
            schemas.get("Address").is_some(),
            "the nested definition must be hoisted under its own name"
        );
        assert!(
            schemas.get("Role").is_some(),
            "the response's nested definition must be hoisted too"
        );

        // No draft-07 leftovers on the hoisted schema objects.
        assert!(schemas["CreateUserRequest"].get("$schema").is_none());
        assert!(schemas["CreateUserRequest"].get("definitions").is_none());
        assert!(schemas["Address"].get("$schema").is_none());

        let serialized = serde_json::to_string(&doc).expect("document must serialise");
        assert!(
            !serialized.contains("#/definitions/"),
            "no legacy #/definitions/ ref may survive in the document"
        );
    }
}
