//! Integration test: verifies that `#[lucy_http]` registers the endpoint
//! in the global Lucy registry via the inventory bridge.
//!
//! This test lives in the `lucy` crate (not `lucy-macro`) so that `::lucy`
//! resolves correctly in the code emitted by the proc-macro.

use lucyd::lucy_http;
use lucy_core::registry::global_registry;

#[allow(dead_code)]
#[lucy_http(
    method = "GET",
    path = "/integration-test",
    description = "integration test endpoint"
)]
async fn dummy_handler() -> &'static str {
    "ok"
}

#[test]
fn lucy_http_registers_endpoint_in_global_registry() {
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
