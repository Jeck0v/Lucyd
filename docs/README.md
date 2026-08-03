# Lucyd Documentation

Lucyd is a Rust library that auto-generates an **interactive documentation and testing interface** for HTTP REST, WebSocket, and MQTT endpoints, served directly at `localhost:8080/docs`.
It is designed for [Axum](https://github.com/tokio-rs/axum) backends and requires zero external tools.

## Contents

| # | Page | What's inside |
|---|------|----------------|
| 1 | [Installation](01-installation.md) | Adding `lucyd` to `Cargo.toml`, and the optional `lucyd-cli` binary |
| 2 | [Setup](02-setup.md) | Wiring `docs_router()` into your Axum app |
| 3 | [Attribute macros](03-macros.md) | `#[lucyd_http]`, `#[lucyd_ws]`, `#[lucyd_mqtt]`: arguments, examples, compile errors |
| 4 | [JSON Schema generation](04-json-schema.md) | Deriving `JsonSchema` for request/response types |
| 5 | [The spec format](05-spec-format.md) | `/docs/spec.json`, the internal endpoint catalogue |
| 6 | [The OpenAPI export](06-openapi-export.md) | `/docs/openapi.json`, scope, schema hoisting, tooling interop |
| 7 | [Building the UI](07-building-ui.md) | `cargo xtask build-ui`, CI/Docker setup |
| 8 | [UI features](08-ui-features.md) | The `/docs` explorer: HTTP, WebSocket, MQTT panels, auth |
| 9 | [Full example](09-full-example.md) | A complete `main.rs` using all three macros |
| 10 | [Architecture overview](10-architecture.md) | Macro → registry → spec pipeline, crate responsibilities |
| 11 | [Known limitations (v0.2)](11-limitations.md) | Current scoping gaps and their status |
| 12 | [Importing an OpenAPI document](12-import-openapi.md) | `cargo xtask import-openapi`: reverse-generating Rust scaffolding from a spec |
| 13 | [Validating a migration](13-openapi-diff.md) | `lucyd diff`: proving the migrated API still matches the contract |

Migrating an existing API? Pages 12, 6 and 13 are the three steps, in that order.

## Quick start

```toml
[dependencies]
lucyd    = "0.2.0"
schemars = "0.8"
serde    = { version = "1", features = ["derive"] }
axum     = "0.8"
tokio    = { version = "1", features = ["full"] }
```

```rust
use axum::Router;
use lucyd::docs_router;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .merge(docs_router()); // serves /docs, /docs/spec.json, /docs/openapi.json

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

Then open **`http://localhost:3000/docs`**. See [Installation](01-installation.md) and [Setup](02-setup.md) for the full walkthrough.
