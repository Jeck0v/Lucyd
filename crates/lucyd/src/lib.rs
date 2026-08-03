//! # Lucyd — Unified API documentation for Rust/Axum backends
//!
//! Lucyd automatically generates an interactive documentation interface
//! for HTTP REST, WebSocket, and MQTT endpoints, served at `/docs`.
//!
//! ## Usage
//!
//! 1. Annotate your Axum handlers with Lucyd macros:
//!
//! ```rust,ignore
//! use lucyd::{lucyd_http, docs_router};
//! use axum::Router;
//!
//! #[lucyd_http(method = "GET", path = "/health", description = "Health check endpoint")]
//! async fn health() -> &'static str {
//!     "ok"
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let app = Router::new()
//!         .route("/health", axum::routing::get(health))
//!         .merge(docs_router());
//!
//!     // Lucyd docs are now available at http://localhost:3000/docs
//!     let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
//!     axum::serve(listener, app).await.unwrap();
//! }
//! ```
//!
//! 2. Build the UI bundle once: `cargo xtask build-ui`
//!
//! ## Protocols
//!
//! | Macro        | Protocol   | Use for                         |
//! |--------------|------------|---------------------------------|
//! | `lucyd_http` | HTTP REST  | Standard CRUD routes            |
//! | `lucyd_ws`   | WebSocket  | Real-time bidirectional streams |
//! | `lucyd_mqtt` | MQTT       | IoT device messaging topics     |
//!
//! Before 0.2 these were spelled `lucy_http`, `lucy_ws` and `lucy_mqtt`. Those
//! names still work and still expand to the same code; they emit a deprecation
//! warning pointing at the new spelling.

// Re-export the runtime API from lucyd-core
pub use lucyd_core::openapi::generate_openapi_document; // The OpenAPI 3.1 document `/docs/openapi.json` serves
pub use lucyd_core::registry::EndpointRegistry; // Global endpoint registry for collected metadata
pub use lucyd_core::registry::global_registry; // Accessor for the registry the macros populate
pub use lucyd_core::router::docs_router; // Axum router serving the `/docs` UI and JSON spec

// Re-export the proc-macros from lucyd-macro
pub use lucyd_macro::lucyd_http; // Attribute macro for HTTP REST handlers
pub use lucyd_macro::lucyd_mqtt; // Attribute macro for MQTT topic handlers
pub use lucyd_macro::lucyd_ws; // Attribute macro for WebSocket handlers

// The pre-0.2 spellings, so `use lucyd::lucy_http` keeps resolving. The
// deprecation lives on the definitions in lucyd-macro and travels with them,
// so it fires at the call site, not here.
pub use lucyd_macro::lucy_http;
pub use lucyd_macro::lucy_mqtt;
pub use lucyd_macro::lucy_ws;

/// Hidden re-exports required by macro-generated code.
///
/// Proc-macros emit `::lucyd::_private::inventory::submit!` and
/// `::lucyd::_private::lucyd_types::...` so that consumer crates only need
/// `lucyd` as a dependency — they do not need to depend on `inventory` or
/// `lucyd-types` directly.
#[doc(hidden)]
pub mod _private {
    pub use inventory;
    pub use lucyd_types;
    pub use schemars;
    pub use serde_json;
}
