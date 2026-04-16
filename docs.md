# Lucy - Documentation

Lucy is a Rust library that auto-generates an interactive documentation interface for HTTP REST, WebSocket, and MQTT endpoints, served directly at `localhost/docs`.  
It is designed for [Axum](https://github.com/tokio-rs/axum) backends and requires zero external tools.

---

## Table of contents

1. [Installation](#1-installation)
2. [Setup](#2-setup)
3. [Attribute macros (tags)](#3-attribute-macros-tags)
   - [`#[lucy_http]`](#lucy_http)
   - [`#[lucy_ws]`](#lucy_ws)
   - [`#[lucy_mqtt]`](#lucy_mqtt)
4. [The spec format](#4-the-spec-format)
5. [Building the UI](#5-building-the-ui)
6. [Full example](#6-full-example)
7. [Architecture overview](#7-architecture-overview)
8. [Known limitations (v0.1)](#8-known-limitations-v01)

---

## 1. Installation

Add `lucy` to your `Cargo.toml`:

```toml
[dependencies]
lucy = { path = "../Lucy/crates/lucy" }   # local path during development

# Required peer dependencies
axum  = "0.8"
tokio = { version = "1", features = ["full"] }
```

> Once published on crates.io the path will be replaced by a version: `lucy = "0.1"`.

---

## 2. Setup

Merge `docs_router()` into your Axum application. That is the only wiring required.

```rust
use axum::Router;
use lucy::docs_router;

#[tokio::main]
async fn main() {
    let app = Router::new()
        // your existing routes
        .route("/health", axum::routing::get(health_handler))
        // Lucy: serves /docs and /docs/spec.json
        .merge(docs_router());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind");

    axum::serve(listener, app)
        .await
        .expect("server error");
}
```

Then open **`http://localhost:3000/docs`** in your browser.

> `docs_router()` registers two routes:
> - `GET /docs/spec.json` — the machine-readable endpoint catalogue
> - `GET /docs/*path` — the embedded React UI (SPA)

---

## 3. Attribute macros (tags)

Import the macros you need at the top of each file:

```rust
use lucy::{lucy_http, lucy_ws, lucy_mqtt};
```

All three macros follow the same `key = "value"` syntax and are **zero-cost at runtime** — they annotate handlers without adding overhead to request processing.

---

### `#[lucy_http]`

Marks an Axum HTTP handler for documentation generation.

**Signature**

```
#[lucy_http(method = "…", path = "…")]
#[lucy_http(method = "…", path = "…", description = "…")]
```

**Arguments**

| Argument      | Required | Type   | Description |
|---------------|----------|--------|-------------|
| `method`      | yes      | string | HTTP verb in uppercase: `"GET"`, `"POST"`, `"PUT"`, `"DELETE"`, `"PATCH"` |
| `path`        | yes      | string | Full URL path, must start with `/` (e.g. `"/api/users"`) |
| `description` | no       | string | Human-readable explanation shown in the UI |

**Examples**

```rust
use lucy::lucy_http;

// Minimal — required fields only
#[lucy_http(method = "GET", path = "/health")]
async fn health() -> &'static str {
    "ok"
}

// With description
#[lucy_http(method = "POST", path = "/api/users", description = "Create a new user account")]
async fn create_user(
    axum::Json(body): axum::Json<CreateUserRequest>,
) -> axum::Json<User> {
    // ...
}

// Trailing comma is accepted
#[lucy_http(
    method = "DELETE",
    path   = "/api/users/:id",
    description = "Delete a user by ID",
)]
async fn delete_user(
    axum::extract::Path(id): axum::extract::Path<u64>,
) -> axum::http::StatusCode {
    // ...
}
```

**Compile errors**

Lucy validates the attribute at compile time. These will produce a clear error:

```rust
// Error: missing required `method` argument
#[lucy_http(path = "/health")]
async fn bad() {}

// Error: duplicate `path` argument
#[lucy_http(method = "GET", path = "/a", path = "/b")]
async fn bad() {}

// Error: unknown argument `verb`
#[lucy_http(verb = "GET", path = "/health")]
async fn bad() {}
```

---

### `#[lucy_ws]`

Marks an Axum WebSocket upgrade handler for documentation generation.

**Signature**

```
#[lucy_ws(path = "…")]
#[lucy_ws(path = "…", description = "…")]
```

**Arguments**

| Argument      | Required | Type   | Description |
|---------------|----------|--------|-------------|
| `path`        | yes      | string | WebSocket upgrade path (e.g. `"/ws/events"`) |
| `description` | no       | string | Human-readable explanation shown in the UI |

**Example**

```rust
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use lucy::lucy_ws;

#[lucy_ws(path = "/ws/physics", description = "Real-time physics event stream")]
async fn physics_stream(ws: WebSocketUpgrade) -> impl axum::response::IntoResponse {
    ws.on_upgrade(handle_physics_socket)
}

async fn handle_physics_socket(mut socket: WebSocket) {
    while let Some(Ok(msg)) = socket.recv().await {
        // handle message
    }
}

#[lucy_ws(path = "/ws/multiplayer", description = "Multiplayer session synchronisation")]
async fn multiplayer_sync(ws: WebSocketUpgrade) -> impl axum::response::IntoResponse {
    ws.on_upgrade(handle_multiplayer_socket)
}
```

---

### `#[lucy_mqtt]`

Marks an MQTT topic handler for documentation generation.

**Signature**

```
#[lucy_mqtt(topic = "…")]
#[lucy_mqtt(topic = "…", description = "…")]
```

**Arguments**

| Argument      | Required | Type   | Description |
|---------------|----------|--------|-------------|
| `topic`       | yes      | string | MQTT topic string, supports wildcards (e.g. `"sensors/+/temperature"`) |
| `description` | no       | string | Human-readable explanation shown in the UI |

**Example**

```rust
use lucy::lucy_mqtt;

#[lucy_mqtt(topic = "sensors/temperature", description = "Current temperature from IoT sensors")]
async fn on_temperature(payload: bytes::Bytes) {
    // handle MQTT message
}

#[lucy_mqtt(
    topic       = "devices/+/status",
    description = "Device status updates — + matches any single device ID",
)]
async fn on_device_status(payload: bytes::Bytes) {
    // handle MQTT message
}

#[lucy_mqtt(topic = "flipper/physics/collision")]
async fn on_collision(payload: bytes::Bytes) {
    // handle MQTT message
}
```

---

## 4. The spec format

Lucy exposes a machine-readable catalogue of all annotated endpoints at:

```
GET /docs/spec.json
```

**Response shape**

```json
{
  "version": "0.1.0",
  "endpoints": [
    {
      "name":        "health",
      "path":        "/health",
      "protocol":    "Http",
      "method":      "GET",
      "description": "Health check endpoint"
    },
    {
      "name":     "physics_stream",
      "path":     "/ws/physics",
      "protocol": "WebSocket",
      "description": "Real-time physics event stream"
    },
    {
      "name":     "on_temperature",
      "path":     "sensors/temperature",
      "protocol": "Mqtt",
      "description": "Current temperature from IoT sensors"
    }
  ]
}
```

**`protocol` values**

| Value         | Set by        |
|---------------|---------------|
| `"Http"`      | `#[lucy_http]` |
| `"WebSocket"` | `#[lucy_ws]`  |
| `"Mqtt"`      | `#[lucy_mqtt]` |

**Optional fields** — fields absent from the JSON means the value was not provided (no `null` is ever emitted):

| Field            | Present when |
|------------------|--------------|
| `method`         | `protocol == "Http"` |
| `description`    | argument was provided in the macro |
| `request_schema` | schema wiring is implemented (future) |
| `response_schema`| schema wiring is implemented (future) |

---

## 5. Building the UI

The React interface must be compiled once before the docs surface is available.  
Lucy ships a `cargo xtask` command for this:

```bash
# From the workspace root
cargo xtask build-ui
```

This runs `npm install` + `npm run build` inside `ui/` and writes the output to `ui/dist/`.  
The compiled files are then embedded into the `lucy-core` binary at compile time via `rust-embed` — **no separate static file server is needed**.

**Development workflow**

```bash
# 1. Build the UI once (or after UI changes)
cargo xtask build-ui

# 2. Build and run your Axum application
cargo run -p your-app

# 3. Open the docs
open http://localhost:3000/docs
```

**UI-only hot reload** (while iterating on the frontend)

```bash
# Terminal 1 — Rust backend
cargo run -p your-app

# Terminal 2 — Vite dev server (proxies /docs/spec.json → localhost:3000)
cd ui && npm run dev
# UI available at http://localhost:5173
```

---

## 6. Full example

```rust
// src/main.rs
use axum::{routing::get, Router};
use lucy::{docs_router, lucy_http, lucy_mqtt, lucy_ws};
use tokio::net::TcpListener;

// ── HTTP ─────────────────────────────────────────────────────────────────────

#[lucy_http(method = "GET", path = "/health", description = "Service health check")]
async fn health() -> &'static str {
    "ok"
}

#[lucy_http(method = "GET", path = "/api/objects", description = "List all 3D objects in the scene")]
async fn list_objects() -> axum::Json<Vec<String>> {
    axum::Json(vec!["cube".into(), "sphere".into()])
}

#[lucy_http(
    method      = "POST",
    path        = "/api/objects",
    description = "Add a new 3D object to the scene",
)]
async fn create_object() -> axum::http::StatusCode {
    axum::http::StatusCode::CREATED
}

// ── WebSocket ─────────────────────────────────────────────────────────────────

#[lucy_ws(path = "/ws/physics", description = "Real-time physics event stream")]
async fn physics_ws(
    ws: axum::extract::ws::WebSocketUpgrade,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(|_socket| async {})
}

#[lucy_ws(path = "/ws/multiplayer", description = "Multiplayer session sync")]
async fn multiplayer_ws(
    ws: axum::extract::ws::WebSocketUpgrade,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(|_socket| async {})
}

// ── MQTT ─────────────────────────────────────────────────────────────────────

#[lucy_mqtt(topic = "flipper/physics/collision", description = "Collision events from the physics engine")]
async fn on_collision(_payload: bytes::Bytes) {}

#[lucy_mqtt(
    topic       = "sensors/+/temperature",
    description = "Temperature readings — + matches any sensor ID",
)]
async fn on_temperature(_payload: bytes::Bytes) {}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let app = Router::new()
        // Application routes
        .route("/health",      get(health))
        .route("/api/objects", get(list_objects).post(create_object))
        // Lucy docs — serves /docs and /docs/spec.json
        .merge(docs_router());

    let listener = TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to port 3000");

    println!("Listening on http://localhost:3000");
    println!("Docs at    http://localhost:3000/docs");

    axum::serve(listener, app)
        .await
        .expect("server error");
}
```

---

## 7. Architecture overview

```
your-axum-app
│
├── #[lucy_http / ws / mqtt]   ← proc-macros in crates/lucy-macro
│         │                       parse & validate args at compile time
│         ▼
│   EndpointRegistry            ← global registry in crates/lucy-core
│   (populated at boot)            OnceLock<Mutex<Vec<EndpointMeta>>>
│         │
│         ▼
│   GET /docs/spec.json         ← generate_spec() serialises the registry
│                                  to JSON on each request
│
└── GET /docs/*                 ← React SPA embedded via rust-embed
                                   built by: cargo xtask build-ui
```

**Crate responsibilities**

| Crate | Role |
|---|---|
| `lucy` | Public facade — the only crate you import |
| `lucy-macro` | Proc-macros: parses and validates `#[lucy_*]` attributes |
| `lucy-core` | Runtime: registry, spec generation, Axum router, asset serving |
| `lucy-types` | Shared types: `Protocol`, `EndpointMeta` |
| `xtask` | Build tooling: `cargo xtask build-ui` |

---

## 8. Known limitations (v0.1)

| Limitation | Status |
|---|---|
| **Macros are annotations only** — endpoints do not yet auto-populate `spec.json`. The registration bridge (emitting `ctor`-style side-effects) is the next milestone. | In progress |
| **No auth guard** on `/docs` — do not expose `docs_router()` on a public-facing interface without adding authentication middleware. | Planned |
| **No request/response schema** — `request_schema` and `response_schema` fields in the spec are reserved for a future `schemars` integration. | Planned |
| **Single global registry** — one `EndpointRegistry` per process; two Lucy-using libraries in the same binary share the same doc surface. | By design |
