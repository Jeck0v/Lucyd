//! Integration test: verifies that `#[lucyd_http]` registers the endpoint
//! in the global Lucyd registry via the inventory bridge.
//!
//! This test lives in the `lucyd` crate (not `lucyd-macro`) so that `::lucyd`
//! resolves correctly in the code emitted by the proc-macro.

use lucyd::{lucyd_http, lucyd_ws};
use lucyd_core::registry::global_registry;
use lucyd_types::endpoint::EndpointMeta;
use schemars::JsonSchema;
use serde::Deserialize;

#[allow(dead_code)]
#[lucyd_http(
    method = "GET",
    path = "/integration-test",
    description = "integration test endpoint"
)]
async fn dummy_handler() -> &'static str {
    "ok"
}

/// The pre-0.2 spelling, which is deprecated but must still register: the
/// promise is that old code keeps working, not that it merely compiles.
///
/// Its own module because the deprecation fires when the attribute is
/// resolved, which an `allow` on the annotated item does not cover; an inner
/// attribute here scopes the exemption to this one handler.
mod deprecated_spelling {
    #![allow(deprecated)]

    use lucyd::lucy_http;

    #[allow(dead_code)]
    #[lucy_http(
        method = "POST",
        path = "/deprecated-spelling",
        description = "registered through the old macro name"
    )]
    async fn legacy_handler() -> &'static str {
        "ok"
    }
}

/// The query string of the WebSocket below.
///
/// Required, and deliberately *not* named `token`: an endpoint whose auth
/// parameter happens to be called `token` connects from `/docs` by coincidence
/// of the console's hardcoded injection. This one only connects if the declared
/// parameter actually reaches the UI.
#[derive(Deserialize, JsonSchema)]
#[allow(dead_code)]
struct ScreenAuth {
    /// Signed JWT authorising this screen.
    access_token: String,
}

#[allow(dead_code)]
#[lucyd_ws(
    path = "/ws/screen",
    description = "Per-screen stream, authenticated by query parameter",
    query = ScreenAuth
)]
async fn ws_screen() {}

#[allow(dead_code)]
#[lucyd_http(
    method = "GET",
    path = "/api/scores",
    description = "query-parameter integration endpoint",
    query = ScreenAuth
)]
async fn scores() -> &'static str {
    "[]"
}

/// Looks an endpoint up by path in the global registry.
fn endpoint_at(path: &str) -> EndpointMeta {
    global_registry()
        .lock()
        .expect("registry lock must not be poisoned")
        .all()
        .iter()
        .find(|endpoint| endpoint.path == path)
        .cloned()
        .unwrap_or_else(|| panic!("endpoint '{path}' must be registered"))
}

#[test]
fn lucyd_http_registers_endpoint_in_global_registry() {
    let registry = global_registry()
        .lock()
        .expect("registry lock must not be poisoned");

    let endpoints = registry.all();

    let found = endpoints
        .iter()
        .find(|e| e.path == "/integration-test")
        .expect("endpoint '/integration-test' must be present after macro annotation");

    assert_eq!(found.method.as_deref(), Some("GET"));
    assert_eq!(
        found.description.as_deref(),
        Some("integration test endpoint")
    );
}

#[test]
fn the_deprecated_macro_name_still_registers_its_endpoint() {
    let registry = global_registry()
        .lock()
        .expect("registry lock must not be poisoned");

    let found = registry
        .all()
        .iter()
        .find(|e| e.path == "/deprecated-spelling")
        .cloned()
        .expect("a deprecated macro must still do its job, not just compile");

    assert_eq!(found.method.as_deref(), Some("POST"));
}

#[test]
fn a_query_argument_reaches_the_registry_as_a_json_schema() {
    let schema = endpoint_at("/api/scores")
        .query_schema
        .expect("`query = T` must produce a schema");

    assert_eq!(schema["properties"]["access_token"]["type"], "string");
    assert_eq!(
        schema["required"],
        serde_json::json!(["access_token"]),
        "a non-Option field must come through as a required parameter"
    );
    assert_eq!(
        schema["properties"]["access_token"]["description"], "Signed JWT authorising this screen.",
        "the field's doc comment must survive as the parameter description"
    );
}

#[test]
fn a_websocket_can_declare_a_required_query_parameter_of_any_name() {
    // The regression the console's hardcoded `?token=` injection used to hide:
    // a WebSocket whose required parameter is named anything else was
    // unreachable from `/docs`, because nothing declared it.
    let schema = endpoint_at("/ws/screen")
        .query_schema
        .expect("`query = T` must work on `#[lucyd_ws]`, not just `#[lucyd_http]`");

    assert_eq!(schema["properties"]["access_token"]["type"], "string");
    assert_eq!(schema["required"], serde_json::json!(["access_token"]));
}

#[test]
fn an_endpoint_without_a_query_argument_declares_no_query_schema() {
    assert!(
        endpoint_at("/integration-test").query_schema.is_none(),
        "omitting `query` must leave the field absent, not empty"
    );
}
