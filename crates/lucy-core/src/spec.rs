//! JSON spec generation.
//!
//! Converts the in-memory [`EndpointRegistry`] into the canonical
//! JSON document consumed by the embedded documentation UI.

use crate::registry::EndpointRegistry;
use serde_json::json;

/// Semantic version of the Lucy spec format.
///
/// Bumped whenever the on-the-wire JSON structure changes in a way
/// that is not backwards-compatible with previous UI bundles.
const SPEC_VERSION: &str = "0.1.0";

/// JSON key carrying the spec format version.
const KEY_VERSION: &str = "version";
/// JSON key carrying the array of registered endpoints.
const KEY_ENDPOINTS: &str = "endpoints";

/// Generates the JSON spec document for a given [`EndpointRegistry`].
///
/// The returned value has the following shape:
///
/// ```json
/// {
///   "version": "0.1.0",
///   "endpoints": [ /* serialized EndpointMeta */ ]
/// }
/// ```
///
/// Endpoints are serialised in the order they were registered, which
/// gives the UI a deterministic rendering sequence.
pub fn generate_spec(registry: &EndpointRegistry) -> serde_json::Value {
    json!({
        KEY_VERSION: SPEC_VERSION,
        KEY_ENDPOINTS: registry.all(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lucy_types::endpoint::{EndpointMeta, Protocol};

    const SAMPLE_NAME: &str = "health";
    const SAMPLE_PATH: &str = "/health";

    #[test]
    fn empty_registry_produces_empty_endpoints_array() {
        let registry = EndpointRegistry::new();
        let spec = generate_spec(&registry);

        assert_eq!(
            spec[KEY_VERSION], SPEC_VERSION,
            "version key must match the current spec version"
        );

        let endpoints = spec[KEY_ENDPOINTS]
            .as_array()
            .expect("endpoints key must serialise as a JSON array");
        assert!(
            endpoints.is_empty(),
            "an empty registry must produce an empty endpoints array"
        );
    }

    #[test]
    fn single_endpoint_appears_in_spec() {
        let mut registry = EndpointRegistry::new();
        registry.register(EndpointMeta::new(SAMPLE_NAME, SAMPLE_PATH, Protocol::Http));

        let spec = generate_spec(&registry);
        let endpoints = spec[KEY_ENDPOINTS]
            .as_array()
            .expect("endpoints key must serialise as a JSON array");

        assert_eq!(
            endpoints.len(),
            1,
            "a registry with one entry must produce one serialised endpoint"
        );
        assert_eq!(endpoints[0]["name"], SAMPLE_NAME);
        assert_eq!(endpoints[0]["path"], SAMPLE_PATH);
    }

    #[test]
    fn spec_version_is_pinned() {
        // Guardrail: if someone changes SPEC_VERSION they must also
        // update this test, which forces a conscious decision.
        assert_eq!(SPEC_VERSION, "0.1.0");
    }
}
