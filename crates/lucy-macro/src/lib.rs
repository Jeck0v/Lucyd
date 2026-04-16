//! Proc-macro crate for the Lucy documentation framework.
//!
//! Provides three attribute macros to annotate Axum handlers:
//! - [`lucy_http`] — HTTP REST endpoints
//! - [`lucy_ws`]   — WebSocket endpoints
//! - [`lucy_mqtt`] — MQTT topics
//!
//! # Example
//! ```rust,ignore
//! #[lucy_macro::lucy_http(method = "GET", path = "/health", description = "Health check")]
//! async fn health_handler() -> &'static str { "ok" }
//! ```

use proc_macro::TokenStream;

mod http;
mod mqtt;
mod ws;

/// Annotates an Axum HTTP handler for Lucy documentation generation.
///
/// # Arguments
/// - `method`      — HTTP verb (GET, POST, PUT, DELETE, PATCH)
/// - `path`        — URL path (e.g. `/api/users`)
/// - `description` — Optional human-readable description
#[proc_macro_attribute]
pub fn lucy_http(attr: TokenStream, item: TokenStream) -> TokenStream {
    http::expand(attr, item)
}

/// Annotates an Axum WebSocket handler for Lucy documentation generation.
///
/// # Arguments
/// - `path`        — WebSocket upgrade path (e.g. `/ws/events`)
/// - `description` — Optional human-readable description
#[proc_macro_attribute]
pub fn lucy_ws(attr: TokenStream, item: TokenStream) -> TokenStream {
    ws::expand(attr, item)
}

/// Annotates an MQTT topic handler for Lucy documentation generation.
///
/// # Arguments
/// - `topic`       — MQTT topic string (e.g. `sensors/temperature`)
/// - `description` — Optional human-readable description
#[proc_macro_attribute]
pub fn lucy_mqtt(attr: TokenStream, item: TokenStream) -> TokenStream {
    mqtt::expand(attr, item)
}
