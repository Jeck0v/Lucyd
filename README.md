# Lucyd

**Lucyd** is a Rust library that auto-generates an **interactive documentation and testing interface** for Axum backends: HTTP REST, WebSocket, and MQTT served directly at `/docs`.  
No external tools required. Annotate your handlers, run your server, open your browser.

## Features

- **HTTP REST**: collapsible endpoint cards grouped by tag, editable request body pre-filled from JSON Schema, Execute button, live cURL preview, response display with status + latency
- **WebSocket**: Connect/Disconnect per endpoint, message textarea, real-time message log (in/out), RFC 6455 close code descriptions
- **MQTT**: shared broker WebSocket connection, Subscribe/Unsubscribe per topic, Publish, per-topic message log
- **JSON Schema**: derive `JsonSchema` on your types and pass them to `request =` / `response =`; Lucyd generates typed examples and schema viewers automatically
- **Authentication**: global Authorize modal (Bearer / API Key / Basic), persisted in `localStorage`, applied to all HTTP requests
- **Models tab**: lists all unique request/response schemas collected from registered endpoints
- **Auto-generated**: annotate handlers with `#[lucyd_http]`, `#[lucyd_ws]`, `#[lucyd_mqtt]`; everything else is automatic
- **Zero runtime overhead**: registration happens at link time via the `inventory` crate; no reflection, no startup cost

## Quick start

```toml
# Cargo.toml
[dependencies]
lucyd    = "0.2.1"
schemars = "0.8"
serde    = { version = "1", features = ["derive"] }
axum     = "0.8"
tokio    = { version = "1", features = ["full"] }
```

```rust
use axum::{routing::post, Router};
use lucyd::{docs_router, lucyd_http, lucyd_ws, lucyd_mqtt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, JsonSchema)]
pub struct Ping { pub message: String }

#[derive(Serialize, JsonSchema)]
pub struct Pong { pub echo: String }

#[lucyd_http(
    method      = "POST",
    path        = "/api/ping",
    tags        = "system",
    description = "Echo back the message",
    request     = Ping,
    response    = Pong,
)]
async fn ping(axum::Json(body): axum::Json<Ping>) -> axum::Json<Pong> {
    axum::Json(Pong { echo: body.message })
}

#[lucyd_ws(path = "/ws/events", tags = "realtime", description = "Live event stream")]
async fn events(ws: axum::extract::ws::WebSocketUpgrade) -> impl axum::response::IntoResponse {
    ws.on_upgrade(|_| async {})
}

#[lucyd_mqtt(topic = "sensors/temperature", tags = "iot", description = "Temperature readings")]
async fn on_temp(_payload: bytes::Bytes) {}

#[tokio::main]
async fn main() {
    // Build the UI bundle once before running: cargo xtask build-ui
    let app = Router::new()
        .route("/api/ping", post(ping))
        .merge(docs_router()); // serves /docs, /docs/spec.json and /docs/openapi.json

    axum::serve(
        tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap(),
        app,
    )
    .await
    .unwrap();
}
```

## Try it locally

A runnable example lives in `crates/lucyd/examples/demo.rs`: it wires up `#[lucyd_http]`, `#[lucyd_ws]`, and `#[lucyd_mqtt]` together with `docs_router()`, so you can exercise the whole pipeline without writing any code of your own.

```bash
cargo run --example demo -p lucyd
```

Then, in another terminal:

```bash
curl http://localhost:3000/docs/openapi.json                 # generated OpenAPI 3.1 document
curl http://localhost:3000/docs/spec.json                    # internal spec, all protocols
curl -X POST http://localhost:3000/api/ping \
  -H "Content-Type: application/json" \
  -d '{"message":"hello"}'                                   # real handler round-trip
```

Or open [http://localhost:3000/docs](http://localhost:3000/docs) in a browser for the interactive UI.

## Macros

| Macro        | Protocol   | Use for                         |
|--------------|------------|---------------------------------|
| `lucyd_http`  | HTTP REST  | Standard CRUD routes            |
| `lucyd_ws`    | WebSocket  | Real-time bidirectional streams |
| `lucyd_mqtt`  | MQTT       | IoT device messaging topics     |

## Crate structure

| Crate        | Role |
|--------------|------|
| `lucyd`      | Public facade: the only crate you import |
| `lucyd-macro` | Proc-macros: parse `#[lucyd_*]` attributes, emit `inventory::submit!` |
| `lucyd-core`  | Runtime: global registry, spec generation, Axum router, asset serving |
| `lucyd-types` | Shared types: `Protocol`, `EndpointMeta`, `EndpointMetaStatic` |
| `lucyd-cli`   | The `lucyd` binary: `lucyd diff`, for validating a migration |
| `xtask`      | Build tooling: `cargo xtask build-ui`, `cargo xtask import-openapi` |

Only `lucyd` and `lucyd-cli` are meant to be named directly. The other three are pulled in by the facade.

## Migrating an existing API onto Lucyd

Three steps, each with a tool:

```bash
# 1. Turn the spec you already have into Rust scaffolding.
cargo xtask import-openapi openapi.yaml

# 2. Implement the stubs, run the server, save what it now serves.
curl -o lucyd-openapi.json http://localhost:3000/docs/openapi.json

# 3. Prove nothing was dropped along the way.
cargo install lucyd-cli
lucyd diff
```

```
OpenAPI diff report

Missing:
  DELETE /api/users/{id}

Changed:
  GET /api/users/{id}
    response: field "createdAt" removed

Summary:
1 missing, 0 added, 1 changed
```

Exit code `0` when the two documents agree, `1` when they differ, `2` when the run failed outright, so a CI job can tell a real regression apart from a broken step. See [docs/13-openapi-diff.md](docs/13-openapi-diff.md).

> The binary is installed with `cargo install lucyd-cli`, not `cargo install lucyd`: the latter is the library and has no executable to install.

## Documentation

Full usage guide, all macro arguments, spec format, architecture details:

```
docs/README.md
```

Rust API documentation:

```bash
cargo doc --open
```
