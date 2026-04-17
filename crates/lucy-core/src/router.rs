//! Axum router wiring for the `/docs` surface.
//!
//! The router exposes two things:
//!
//! * `GET /docs/spec.json` — the JSON specification generated from the
//!   global [`EndpointRegistry`]; consumed by the embedded UI.
//! * `GET /docs/{*path}`   — the embedded Single-Page-Application UI,
//!   served from assets compiled into the binary.

use crate::{assets, registry, spec};
use axum::{routing::get, Router};

/// Creates the Axum router that serves the Lucy docs interface.
///
/// Routes:
/// - `GET /docs/spec.json` — returns the generated endpoint spec
/// - `GET /docs/{*path}`   — serves embedded UI static assets with
///   SPA-style fallback to `index.html`
///
/// The function is generic over the host application's state type `S` so it
/// can be merged directly into any [`Router<S>`] without a state mismatch.
/// None of the Lucy handlers access `S` — they use the global registry only.
///
/// # Example
/// ```rust,ignore
/// let app: Router<AppState> = Router::new()
///     .merge(my_routes())
///     .merge(docs_router::<AppState>());
/// ```
pub fn docs_router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        // Explicit routes for the root — Axum's `{*path}` wildcard requires at
        // least one path segment, so `/docs` and `/docs/` would 404 without these.
        .route("/docs", get(assets::serve_index))
        .route("/docs/", get(assets::serve_index))
        .route("/docs/spec.json", get(spec_handler))
        .route("/docs/{*path}", get(assets::serve_asset))
}

/// Returns the currently registered endpoint spec as JSON.
///
/// Locks the global registry just long enough to snapshot its contents
/// into a [`serde_json::Value`], then releases the lock before the
/// response is serialised.
///
/// # Lock poisoning
/// If a thread panics while holding the registry lock the mutex becomes
/// poisoned. Rather than propagating a panic (which would crash the Axum
/// worker thread), we recover the inner guard via [`PoisonError::into_inner`]
/// and serve whatever data was accumulated before the poison event.
async fn spec_handler() -> axum::response::Json<serde_json::Value> {
    let registry = registry::global_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    axum::response::Json(spec::generate_spec(&registry))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docs_router_constructs_without_panicking() {
        // Smoke test: simply building the router exercises the route
        // path validation inside Axum (e.g. wildcard syntax). If the
        // syntax ever drifts from the version we target, the Router
        // constructor will panic and this test will fail loudly.
        let _router: Router = docs_router();
    }
}
