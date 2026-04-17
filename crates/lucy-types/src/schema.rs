//! JSON Schema wrapper types.
//!
//! This module provides a thin newtype around [`serde_json::Value`] used to
//! represent JSON Schemas throughout the Lucy framework. Keeping a dedicated
//! type here leaves room for future validation hooks without breaking the
//! public API of [`crate::endpoint::EndpointMeta`].

/// A thin wrapper around [`serde_json::Value`] representing a JSON Schema.
///
/// This newtype exists to allow future validation hooks and schema
/// manipulation to be added without changing the public API of [`EndpointMeta`].
///
/// [`EndpointMeta`]: crate::endpoint::EndpointMeta
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JsonSchemaWrapper(pub serde_json::Value);

impl From<serde_json::Value> for JsonSchemaWrapper {
    /// Wrap a raw [`serde_json::Value`] into a [`JsonSchemaWrapper`].
    fn from(value: serde_json::Value) -> Self {
        Self(value)
    }
}

impl From<JsonSchemaWrapper> for serde_json::Value {
    /// Unwrap a [`JsonSchemaWrapper`] back into its inner [`serde_json::Value`].
    fn from(wrapper: JsonSchemaWrapper) -> serde_json::Value {
        wrapper.0
    }
}
