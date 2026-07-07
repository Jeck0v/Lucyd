//! Shared fixtures for the `openapi` module's test suites.

use lucy_types::endpoint::{EndpointMeta, Protocol};
use serde_json::{Value, json};

pub(super) const USERS_PATH: &str = "/api/users";
pub(super) const HEALTH_NAME: &str = "health";
pub(super) const HEALTH_PATH: &str = "/health";

/// Builds an HTTP [`EndpointMeta`] with a method set.
pub(super) fn http_endpoint(name: &str, method: &str, path: &str) -> EndpointMeta {
    let mut meta = EndpointMeta::new(name, path, Protocol::Http);
    meta.method = Some(method.to_string());
    meta
}

/// A minimal schemars-style schema with a `title` and no nested defs.
pub(super) fn simple_schema(title: &str) -> Value {
    json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": title,
        "type": "object",
        "properties": { "x": { "type": "string" } }
    })
}

/// Same shape as [`simple_schema`] but with different content.
pub(super) fn simple_schema_variant(title: &str) -> Value {
    json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": title,
        "type": "object",
        "properties": { "y": { "type": "integer" } }
    })
}
