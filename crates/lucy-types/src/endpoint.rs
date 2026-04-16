//! Endpoint metadata types.
//!
//! This module defines the core data structures used to describe an
//! endpoint registered with the Lucy documentation framework, regardless
//! of the underlying transport protocol.

use serde::{Deserialize, Serialize};

/// Transport protocol used by an endpoint.
///
/// The variants are serialized using their Rust identifier as a JSON
/// string (e.g. [`Protocol::Http`] becomes `"Http"`) so that the
/// on-the-wire representation stays stable and human-readable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Protocol {
    /// Classic synchronous HTTP/1.1 or HTTP/2 request-response endpoint.
    Http,
    /// Bidirectional WebSocket stream, typically used for real-time updates.
    WebSocket,
    /// MQTT topic-based publish/subscribe channel for IoT-style workloads.
    Mqtt,
}

/// Fully-qualified description of an endpoint exposed by the application.
///
/// An [`EndpointMeta`] carries everything required to generate documentation
/// for a single endpoint: its human-readable name, network location, protocol
/// and optional request/response JSON schemas.
///
/// Schemas are stored as raw [`serde_json::Value`] instances to avoid a hard
/// dependency on a specific schema-generation crate in the public API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointMeta {
    /// Display name of the endpoint, used as a title in generated docs.
    pub name: String,
    /// URL path (for HTTP/WebSocket) or topic string (for MQTT).
    pub path: String,
    /// Transport protocol used by this endpoint.
    pub protocol: Protocol,
    /// Optional long-form, human-readable description.
    // Skip serializing None so the JSON output omits the key entirely.
    // Without this, serde emits `"description": null` which TypeScript's
    // optional field syntax (`description?: string`) does not handle
    // correctly — null passes an `!== undefined` check and renders as "null".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// HTTP verb (`GET`, `POST`, ...). `None` for WebSocket and MQTT endpoints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// JSON Schema describing the expected request payload, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_schema: Option<serde_json::Value>,
    /// JSON Schema describing the response payload, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<serde_json::Value>,
}

/// Static-lifetime version of [`EndpointMeta`] used for compile-time
/// registration via the `inventory` crate.
///
/// Proc-macro generated code emits `inventory::submit!` blocks containing
/// this type (all fields are `&'static str`, which is const-constructible).
/// At runtime, [`EndpointMetaStatic::into_endpoint_meta`] converts each
/// entry into a heap-allocated [`EndpointMeta`].
pub struct EndpointMetaStatic {
    /// Display name of the endpoint.
    pub name: &'static str,
    /// URL path or MQTT topic string.
    pub path: &'static str,
    /// Transport protocol.
    pub protocol: Protocol,
    /// Optional human-readable description.
    pub description: Option<&'static str>,
    /// HTTP verb, if applicable.
    pub method: Option<&'static str>,
}

impl EndpointMetaStatic {
    /// Converts the static reference into an owned [`EndpointMeta`].
    pub fn into_endpoint_meta(&self) -> EndpointMeta {
        EndpointMeta {
            name: self.name.to_owned(),
            path: self.path.to_owned(),
            protocol: self.protocol.clone(),
            description: self.description.map(|s| s.to_owned()),
            method: self.method.map(|s| s.to_owned()),
            request_schema: None,
            response_schema: None,
        }
    }
}

// Declare EndpointMetaStatic as an inventory-collectable type.
// Must appear exactly once across the entire binary.
// Proc-macro generated code calls `::inventory::submit! { EndpointMetaStatic { ... } }`
// and lucy-core drains `::inventory::iter::<EndpointMetaStatic>()` on first registry access.
inventory::collect!(EndpointMetaStatic);

impl EndpointMeta {
    /// Create a new [`EndpointMeta`] with only the mandatory fields populated.
    ///
    /// Optional fields (`description`, `method`, `request_schema`,
    /// `response_schema`) are initialised to `None` and can be filled in
    /// afterwards by mutating the returned value.
    ///
    /// # Examples
    ///
    /// ```
    /// use lucy_types::endpoint::{EndpointMeta, Protocol};
    ///
    /// let meta = EndpointMeta::new("health", "/health", Protocol::Http);
    /// assert_eq!(meta.name, "health");
    /// assert_eq!(meta.path, "/health");
    /// assert_eq!(meta.protocol, Protocol::Http);
    /// ```
    pub fn new(name: impl Into<String>, path: impl Into<String>, protocol: Protocol) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            protocol,
            description: None,
            method: None,
            request_schema: None,
            response_schema: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Named constants avoid duplicating magic strings across tests and keep
    // the expected serde representation documented in a single place.
    const EXPECTED_HTTP_JSON: &str = "\"Http\"";
    const EXPECTED_WEBSOCKET_JSON: &str = "\"WebSocket\"";
    const EXPECTED_MQTT_JSON: &str = "\"Mqtt\"";

    const HEALTH_NAME: &str = "health";
    const HEALTH_PATH: &str = "/health";

    #[test]
    fn protocol_http_serializes_to_http_string() {
        let json = serde_json::to_string(&Protocol::Http)
            .expect("serializing Protocol::Http should never fail");
        assert_eq!(json, EXPECTED_HTTP_JSON);
    }

    #[test]
    fn protocol_websocket_serializes_to_websocket_string() {
        let json = serde_json::to_string(&Protocol::WebSocket)
            .expect("serializing Protocol::WebSocket should never fail");
        assert_eq!(json, EXPECTED_WEBSOCKET_JSON);
    }

    #[test]
    fn protocol_mqtt_serializes_to_mqtt_string() {
        let json = serde_json::to_string(&Protocol::Mqtt)
            .expect("serializing Protocol::Mqtt should never fail");
        assert_eq!(json, EXPECTED_MQTT_JSON);
    }

    #[test]
    fn endpoint_meta_round_trips_through_serde_json() {
        let original = EndpointMeta::new(HEALTH_NAME, HEALTH_PATH, Protocol::Http);

        // Serialize then deserialize: both directions must succeed and the
        // resulting value must be structurally identical to the input.
        let serialized =
            serde_json::to_string(&original).expect("serialization of EndpointMeta must succeed");
        let deserialized: EndpointMeta = serde_json::from_str(&serialized)
            .expect("deserialization of EndpointMeta must succeed");

        assert_eq!(deserialized.name, HEALTH_NAME);
        assert_eq!(deserialized.path, HEALTH_PATH);
        assert_eq!(deserialized.protocol, Protocol::Http);
        assert!(deserialized.description.is_none());
        assert!(deserialized.method.is_none());
        assert!(deserialized.request_schema.is_none());
        assert!(deserialized.response_schema.is_none());
    }
}
