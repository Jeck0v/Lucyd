//! Integration test: verifies that `#[lucyd_http]` registers the endpoint
//! in the global Lucyd registry via the inventory bridge.
//!
//! This test lives in the `lucyd` crate (not `lucyd-macro`) so that `::lucyd`
//! resolves correctly in the code emitted by the proc-macro.

use lucyd::lucyd_http;
use lucyd_core::registry::global_registry;

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
