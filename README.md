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
- **Auto-generated**: annotate handlers with `#[lucy_http]`, `#[lucy_ws]`, `#[lucy_mqtt]`; everything else is automatic
- **Zero runtime overhead**: registration happens at link time via the `inventory` crate; no reflection, no startup cost

## Quick start

```toml
# Cargo.toml
[dependencies]
lucyd   = "0.1.8"
schemars = "0.8"
serde    = { version = "1", features = ["derive"] }
axum     = "0.8"
tokio    = { version = "1", features = ["full"] }
```

```rust
use axum::{routing::post, Router};
use lucyd::{docs_router, lucy_http, lucy_ws, lucy_mqtt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, JsonSchema)]
pub struct Ping { pub message: String }

#[derive(Serialize, JsonSchema)]
pub struct Pong { pub echo: String }

#[lucy_http(
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

#[lucy_ws(path = "/ws/events", tags = "realtime", description = "Live event stream")]
async fn events(ws: axum::extract::ws::WebSocketUpgrade) -> impl axum::response::IntoResponse {
    ws.on_upgrade(|_| async {})
}

#[lucy_mqtt(topic = "sensors/temperature", tags = "iot", description = "Temperature readings")]
async fn on_temp(_payload: bytes::Bytes) {}

#[tokio::main]
async fn main() {
    // Build the UI bundle once before running: cargo xtask build-ui
    let app = Router::new()
        .route("/api/ping", post(ping))
        .merge(docs_router()); // serves /docs and /docs/spec.json

    axum::serve(
        tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap(),
        app,
    )
    .await
    .unwrap();
}
```

## Macros

| Macro        | Protocol   | Use for                         |
|--------------|------------|---------------------------------|
| `lucy_http`  | HTTP REST  | Standard CRUD routes            |
| `lucy_ws`    | WebSocket  | Real-time bidirectional streams |
| `lucy_mqtt`  | MQTT       | IoT device messaging topics     |

## Crate structure

| Crate        | Role |
|--------------|------|
| `lucyd`      | Public facade: the only crate you import |
| `lucy-macro` | Proc-macros: parse `#[lucy_*]` attributes, emit `inventory::submit!` |
| `lucy-core`  | Runtime: global registry, spec generation, Axum router, asset serving |
| `lucy-types` | Shared types: `Protocol`, `EndpointMeta`, `EndpointMetaStatic` |
| `xtask`      | Build tooling: `cargo xtask build-ui` |

## Documentation

Full usage guide, all macro arguments, spec format, architecture details:

```
docs.md
```

Rust API documentation:

```bash
cargo doc --open
```
