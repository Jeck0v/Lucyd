//! # Lucy — Unified API documentation for Rust/Axum backends
//!
//! Lucy automatically generates an interactive documentation interface
//! for HTTP REST, WebSocket, and MQTT endpoints, served at `/docs`.
//!
//! ## Usage
//!
//! 1. Annotate your Axum handlers with Lucy macros:
//!
//! ```rust,ignore
//! use lucy::{lucy_http, docs_router};
//! use axum::Router;
//!
//! #[lucy_http(method = "GET", path = "/health", description = "Health check endpoint")]
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
//!     // Lucy docs are now available at http://localhost:3000/docs
//!     let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
//!     axum::serve(listener, app).await.unwrap();
//! }
//! ```
//!
//! 2. Build the UI bundle once: `cargo xtask build-ui`
//!
//! ## Protocols
//!
//! | Macro         | Protocol   | Use for                         |
//! |---------------|------------|---------------------------------|
//! | `lucy_http`   | HTTP REST  | Standard CRUD routes            |
//! | `lucy_ws`     | WebSocket  | Real-time bidirectional streams |
//! | `lucy_mqtt`   | MQTT       | IoT device messaging topics     |

// Re-export the runtime API from lucy-core
pub use lucy_core::registry::EndpointRegistry; // Global endpoint registry for collected metadata
pub use lucy_core::router::docs_router;        // Axum router serving the `/docs` UI and JSON spec

// Re-export the proc-macros from lucy-macro
pub use lucy_macro::lucy_http; // Attribute macro for HTTP REST handlers
pub use lucy_macro::lucy_mqtt; // Attribute macro for MQTT topic handlers
pub use lucy_macro::lucy_ws;   // Attribute macro for WebSocket handlers

/// Hidden re-exports required by macro-generated code.
///
/// Proc-macros emit `::lucy::_private::inventory::submit!` and
/// `::lucy::_private::lucy_types::...` so that consumer crates only need
/// `lucy` as a dependency — they do not need to depend on `inventory` or
/// `lucy-types` directly.
#[doc(hidden)]
pub mod _private {
    pub use inventory;
    pub use lucy_types;
}
