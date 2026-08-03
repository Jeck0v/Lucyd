//! Proc-macro crate for the Lucyd documentation framework.
//!
//! Provides three attribute macros to annotate Axum handlers:
//! - [`lucyd_http`] : HTTP REST endpoints
//! - [`lucyd_ws`]   : WebSocket endpoints
//! - [`lucyd_mqtt`] : MQTT topics
//!
//! Each one also answers to its pre-0.2 name: [`lucy_http`], [`lucy_ws`] and
//! [`lucy_mqtt`] expand to exactly the same code and only warn.
//!
//! # Example
//! ```rust,ignore
//! #[lucyd_macro::lucyd_http(method = "GET", path = "/health", description = "Health check")]
//! async fn health_handler() -> &'static str { "ok" }
//! ```

use proc_macro::TokenStream;

mod common;
mod http;
mod mqtt;
mod ws;

/// Annotates an Axum HTTP handler for Lucyd documentation generation.
///
/// # Arguments
/// - `method`      — HTTP verb (GET, POST, PUT, DELETE, PATCH)
/// - `path`        — URL path (e.g. `/api/users`)
/// - `description` — Optional human-readable description
#[proc_macro_attribute]
pub fn lucyd_http(attr: TokenStream, item: TokenStream) -> TokenStream {
    http::expand(attr, item)
}

/// Annotates an Axum WebSocket handler for Lucyd documentation generation.
///
/// # Arguments
/// - `path`        — WebSocket upgrade path (e.g. `/ws/events`)
/// - `description` — Optional human-readable description
#[proc_macro_attribute]
pub fn lucyd_ws(attr: TokenStream, item: TokenStream) -> TokenStream {
    ws::expand(attr, item)
}

/// Annotates an MQTT topic handler for Lucyd documentation generation.
///
/// # Arguments
/// - `topic`       — MQTT topic string (e.g. `sensors/temperature`)
/// - `description` — Optional human-readable description
#[proc_macro_attribute]
pub fn lucyd_mqtt(attr: TokenStream, item: TokenStream) -> TokenStream {
    mqtt::expand(attr, item)
}

// Deprecated aliases.
//
// Each one calls the same `expand` as the macro that replaced it, rather than
// forwarding to it: a proc-macro cannot invoke another proc-macro, and routing
// through the implementation keeps the two spellings provably identical
// instead of merely similar.
//
// The version in `since` is the one that introduced the new names, which is
// also the first version in which these warn.

/// Deprecated spelling of [`lucyd_http`].
#[deprecated(
    since = "0.2.0",
    note = "renamed to `lucyd_http`, to match the crate name"
)]
#[proc_macro_attribute]
pub fn lucy_http(attr: TokenStream, item: TokenStream) -> TokenStream {
    http::expand(attr, item)
}

/// Deprecated spelling of [`lucyd_ws`].
#[deprecated(
    since = "0.2.0",
    note = "renamed to `lucyd_ws`, to match the crate name"
)]
#[proc_macro_attribute]
pub fn lucy_ws(attr: TokenStream, item: TokenStream) -> TokenStream {
    ws::expand(attr, item)
}

/// Deprecated spelling of [`lucyd_mqtt`].
#[deprecated(
    since = "0.2.0",
    note = "renamed to `lucyd_mqtt`, to match the crate name"
)]
#[proc_macro_attribute]
pub fn lucy_mqtt(attr: TokenStream, item: TokenStream) -> TokenStream {
    mqtt::expand(attr, item)
}
