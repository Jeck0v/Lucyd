# Lucy Documentation

Lucy is a Rust library that auto-generates an **interactive documentation and testing interface** for HTTP REST, WebSocket, and MQTT endpoints, served directly at `localhost:8080/docs`.  
It is designed for [Axum](https://github.com/tokio-rs/axum) backends and requires zero external tools.

---

## Table of contents

1. [Installation](#1-installation)
2. [Setup](#2-setup)
3. [Attribute macros](#3-attribute-macros)
   - [`#[lucy_http]`](#lucy_http)
   - [`#[lucy_ws]`](#lucy_ws)
   - [`#[lucy_mqtt]`](#lucy_mqtt)
4. [JSON Schema generation](#4-json-schema-generation)
5. [The spec format](#5-the-spec-format)
6. [Building the UI](#6-building-the-ui)
7. [UI features](#7-ui-features)
8. [Full example](#8-full-example)
9. [Architecture overview](#9-architecture-overview)
10. [Known limitations (v0.1)](#10-known-limitations-v01)

---

## 1. Installation

Add `lucy` and its required peer dependencies to your `Cargo.toml`:

```toml
[dependencies]
lucy     = { path = "../Lucy/crates/lucy" } # local path during development
# lucy = "0.1" # Soon...
schemars = "0.8"                              # needed only if you use request/response schemas
serde    = { version = "1", features = ["derive"] }
axum     = "0.8"
tokio    = { version = "1", features = ["full"] }
```

> Once published on crates.io the path will be replaced by a version: `lucy = "0.1"`.

---

## 2. Setup

Merge `docs_router()` into your Axum application, that is the only wiring required.

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

    axum::serve(listener, app).await.expect("server error");
}
```

Then open **`http://localhost:3000/docs`** in your browser.

> `docs_router()` registers:
> - `GET /docs` and `GET /docs/`: the embedded React UI (SPA)
> - `GET /docs/spec.json`: the machine-readable endpoint catalogue
> - `GET /docs/*path`: static assets (JS, CSS) embedded in the binary

---

## 3. Attribute macros

Import the macros you need at the top of each handler file:

```rust
use lucy::{lucy_http, lucy_ws, lucy_mqtt};
```

All three macros are **zero-cost at runtime**: they register metadata at link time via the `inventory` crate without adding any overhead to request processing.

---

### `#[lucy_http]`

Marks an Axum HTTP handler for documentation and interactive testing.

**Arguments**

| Argument      | Required | Type      | Description |
|---------------|----------|-----------|-------------|
| `method`      | yes      | string    | HTTP verb in uppercase: `"GET"`, `"POST"`, `"PUT"`, `"DELETE"`, `"PATCH"` |
| `path`        | yes      | string    | Full URL path, must start with `/` (e.g. `"/api/users"`) |
| `description` | no       | string    | Human-readable explanation shown in the UI |
| `tags`        | no       | string    | Comma-separated group labels (e.g. `"users, admin"`) used to visually group endpoints |
| `request`     | no       | type path | Rust type deriving `JsonSchema` generates the request body schema and pre-fills the UI textarea |
| `response`    | no       | type path | Rust type deriving `JsonSchema` generates the response schema shown after execution |

**Examples**

```rust
use lucy::lucy_http;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Minimal  required fields only
#[lucy_http(method = "GET", path = "/health")]
async fn health() -> &'static str {
    "ok"
}

// With description and tag
#[lucy_http(
    method      = "GET",
    path        = "/api/users",
    tags        = "users",
    description = "List all registered users",
)]
async fn list_users() -> axum::Json<Vec<User>> { /* ... */ }

// With request and response schemas
#[derive(Deserialize, JsonSchema)]
pub struct CreateUserRequest { pub name: String, pub email: String }

#[derive(Serialize, JsonSchema)]
pub struct User { pub id: u64, pub name: String, pub email: String }

#[lucy_http(
    method      = "POST",
    path        = "/api/users",
    tags        = "users",
    description = "Create a new user account",
    request     = CreateUserRequest,
    response    = User,
)]
async fn create_user(
    axum::Json(body): axum::Json<CreateUserRequest>,
) -> axum::Json<User> { /* ... */ }
```

**Compile errors**

Lucy validates arguments at compile time:

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

Marks an Axum WebSocket upgrade handler for documentation and interactive testing.

**Arguments**

| Argument      | Required | Type   | Description |
|---------------|----------|--------|-------------|
| `path`        | yes      | string | WebSocket upgrade path (e.g. `"/ws/events"`) |
| `description` | no       | string | Human-readable explanation shown in the UI |
| `tags`        | no       | string | Comma-separated group labels |

**Example**

```rust
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use lucy::lucy_ws;

#[lucy_ws(
    path        = "/ws/physics",
    tags        = "realtime",
    description = "Real-time physics event stream",
)]
async fn physics_stream(ws: WebSocketUpgrade) -> impl axum::response::IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    while let Some(Ok(msg)) = socket.recv().await {
        // handle message
    }
}
```

---

### `#[lucy_mqtt]`

Marks an MQTT topic handler for documentation generation.

**Arguments**

| Argument      | Required | Type   | Description |
|---------------|----------|--------|-------------|
| `topic`       | yes      | string | MQTT topic string, supports wildcards (e.g. `"sensors/+/temperature"`) |
| `description` | no       | string | Human-readable explanation shown in the UI |
| `tags`        | no       | string | Comma-separated group labels |

**Example**

```rust
use lucy::lucy_mqtt;

#[lucy_mqtt(
    topic       = "sensors/temperature",
    tags        = "iot",
    description = "Current temperature from IoT sensors",
)]
async fn on_temperature(payload: bytes::Bytes) { /* ... */ }

#[lucy_mqtt(
    topic       = "devices/+/status",
    tags        = "iot",
    description = "Device status  + matches any single device ID",
)]
async fn on_device_status(payload: bytes::Bytes) { /* ... */ }
```

---

## 4. JSON Schema generation

Lucy integrates with [`schemars`](https://docs.rs/schemars) to generate JSON Schemas at startup for any type passed to `request =` or `response =`.

**Requirements**

1. Add `schemars` to your `Cargo.toml` (see [Installation](#1-installation)).
2. Derive `JsonSchema` on your request/response types.

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, JsonSchema)]
pub struct ScreenEnvelope {
    pub from:       String,
    pub to:         ScreenTarget,
    pub event_type: String,
    pub payload:    serde_json::Value,
}

#[lucy_http(
    method   = "POST",
    path     = "/api/screens/send",
    tags     = "screens",
    request  = ScreenEnvelope,
    response = SendResponse,
)]
pub async fn send_to_screen(/* ... */) { /* ... */ }
```

The schema is generated **once at server startup** (not per request) and included in `/docs/spec.json`. The UI uses it to:
- Pre-fill the request body textarea with a typed example value
- Show the raw JSON Schema in the "Schema" tab (response, after execution)
- Populate the **Models** tab with all unique schemas

---

## 5. The spec format

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
      "name":        "create_user",
      "path":        "/api/users",
      "protocol":    "Http",
      "method":      "POST",
      "description": "Create a new user account",
      "tags":        ["users"],
      "request_schema":  { "$schema": "...", "title": "CreateUserRequest", ... },
      "response_schema": { "$schema": "...", "title": "User", ... }
    },
    {
      "name":        "physics_stream",
      "path":        "/ws/physics",
      "protocol":    "WebSocket",
      "description": "Real-time physics event stream",
      "tags":        ["realtime"]
    },
    {
      "name":        "on_temperature",
      "path":        "sensors/temperature",
      "protocol":    "Mqtt",
      "description": "Current temperature from IoT sensors",
      "tags":        ["iot"]
    }
  ]
}
```

**`protocol` values**

| Value         | Set by         |
|---------------|----------------|
| `"Http"`      | `#[lucy_http]` |
| `"WebSocket"` | `#[lucy_ws]`   |
| `"Mqtt"`      | `#[lucy_mqtt]` |

**Optional fields** absent from JSON when not provided (no `null` emitted):

| Field             | Present when |
|-------------------|--------------|
| `method`          | `protocol == "Http"` |
| `description`     | `description = "…"` was provided |
| `tags`            | at least one tag was provided |
| `request_schema`  | `request = MyType` was provided |
| `response_schema` | `response = MyType` was provided |

---


**CI / Docker**

When building without running `cargo xtask build-ui` first (e.g. in CI or a lint-only job):

```yaml
# GitHub Actions example
- name: Create ui/dist stub for rust-embed
  run: mkdir -p ui/dist
```

An empty `ui/dist/` satisfies `rust-embed` at compile time. The binary will respond with `404 UI not built` for doc requests, which is acceptable for CI where only tests matter.

---

## . UI features

The Lucy UI at `/docs` is an interactive API explorer similar to Swagger UI.

### HTTP endpoints

- **Collapsible cards** per endpoint, grouped by tag
- **Path parameters**  auto-detected from `{param}` placeholders, with individual inputs
- **Request body**  editable textarea pre-filled with a typed example derived from the request schema
- **Execute**  sends the request from the browser and displays the response with status code and latency
- **cURL preview**  always-visible, updates live as inputs or body change
- **Response schema**  shown after execution (Example Value / Schema tabs)

### WebSocket endpoints

- **Connect / Disconnect** per endpoint with status indicator
- **Message textarea** pre-filled with a placeholder, `Ctrl+Enter` to send
- **Message log**  incoming (`←`) and outgoing (`→`) messages with timestamps
- **Error display**  RFC 6455 close codes mapped to human-readable descriptions (e.g. `1008 → Policy violation  check auth`)

### MQTT endpoints

- **Shared broker connection**  one `ws://` URL input at the panel top, all topic cards share it
- **Subscribe / Unsubscribe** toggle per topic
- **Publish**  payload input + Publish button per topic
- **Per-topic message log**

### Authentication

Click **Authorize** in the top-right corner to configure global authentication applied to all HTTP requests:

| Type         | Header emitted |
|--------------|----------------|
| Bearer Token | `Authorization: Bearer <token>` |
| API Key      | `<custom-header>: <key>` |
| Basic Auth   | `Authorization: Basic <base64>` |

Auth is persisted in `localStorage` across page reloads. For WebSocket endpoints, a bearer token is forwarded as `?token=<value>` in the URL.

### Models tab

Lists all unique JSON Schemas collected from `request_schema` and `response_schema` across all endpoints, with "Example Value" and "Schema" tabs per model.

---

## 8. Full example

```rust
// src/main.rs
use axum::{routing::get, Router};
use lucy::{docs_router, lucy_http, lucy_mqtt, lucy_ws};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
pub struct CreateObjectRequest {
    pub name:  String,
    pub shape: String,
}

#[derive(Serialize, JsonSchema)]
pub struct SceneObject {
    pub id:    u64,
    pub name:  String,
    pub shape: String,
}

// ── HTTP ──────────────────────────────────────────────────────────────────────

#[lucy_http(
    method      = "GET",
    path        = "/health",
    tags        = "system",
    description = "Service health check",
)]
async fn health() -> &'static str {
    "ok"
}

#[lucy_http(
    method      = "GET",
    path        = "/api/objects",
    tags        = "scene",
    description = "List all 3D objects in the scene",
    response    = SceneObject,
)]
async fn list_objects() -> axum::Json<Vec<SceneObject>> {
    axum::Json(vec![])
}

#[lucy_http(
    method      = "POST",
    path        = "/api/objects",
    tags        = "scene",
    description = "Add a new 3D object to the scene",
    request     = CreateObjectRequest,
    response    = SceneObject,
)]
async fn create_object(
    axum::Json(_body): axum::Json<CreateObjectRequest>,
) -> axum::http::StatusCode {
    axum::http::StatusCode::CREATED
}

// ── WebSocket ─────────────────────────────────────────────────────────────────

#[lucy_ws(
    path        = "/ws/physics",
    tags        = "realtime",
    description = "Real-time physics event stream",
)]
async fn physics_ws(
    ws: axum::extract::ws::WebSocketUpgrade,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(|_socket| async {})
}

// ── MQTT ──────────────────────────────────────────────────────────────────────

#[lucy_mqtt(
    topic       = "flipper/physics/collision",
    tags        = "iot",
    description = "Collision events from the physics engine",
)]
async fn on_collision(_payload: bytes::Bytes) {}

#[lucy_mqtt(
    topic       = "sensors/+/temperature",
    tags        = "iot",
    description = "Temperature readings  + matches any sensor ID",
)]
async fn on_temperature(_payload: bytes::Bytes) {}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/health", get(health))
        .route("/api/objects", get(list_objects).post(create_object))
        .merge(docs_router());

    let listener = TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to port 3000");

    println!("Listening on http://localhost:3000");
    println!("Docs at    http://localhost:3000/docs");

    axum::serve(listener, app).await.expect("server error");
}
```

---

## 9. Architecture overview

```
your-axum-app
│
├── #[lucy_http / ws / mqtt]        ← proc-macros (crates/lucy-macro)
│         │                            parse & validate args at compile time
│         ▼
│   inventory::submit!               ← linker-magic static registration
│   EndpointMetaStatic { ... }          fn pointers for schema generation
│         │
│         ▼ (first request to /docs/spec.json)
│   global_registry()               ← OnceLock<Mutex<EndpointRegistry>>
│   drains inventory::iter()           calls schema fn pointers once
│         │
│         ▼
│   GET /docs/spec.json             ← generate_spec() serialises registry to JSON
│
└── GET /docs/*                     ← React SPA served from rust-embed
                                       built by: cargo xtask build-ui
```

**Crate responsibilities**

| Crate        | Role |
|--------------|------|
| `lucy`       | Public facade  the only crate consumers import |
| `lucy-macro` | Proc-macros: parse and validate `#[lucy_*]` attributes, emit `inventory::submit!` |
| `lucy-core`  | Runtime: global registry, spec generation, Axum router, asset serving |
| `lucy-types` | Shared types: `Protocol`, `EndpointMeta`, `EndpointMetaStatic` |
| `xtask`      | Build tooling: `cargo xtask build-ui` |

**Dependency flow** (consumers only need `lucy`):

```
your-crate  →  lucy  →  lucy-macro
                      →  lucy-core  →  lucy-types
                                    →  inventory
                                    →  rust-embed
                      →  lucy-types
                      →  inventory  (re-exported as lucy::_private::inventory)
                      →  schemars   (re-exported as lucy::_private::schemars)
                      →  serde_json (re-exported as lucy::_private::serde_json)
```

Macro-generated code references `::lucy::_private::*` so consumer crates only need `lucy` in `Cargo.toml`.

---

## 10. Known limitations (v0.1)

| Limitation | Status |
|---|---|
| **No auth guard** on `/docs`  do not expose `docs_router()` on a public-facing interface without adding authentication middleware. | Planned |
| **Single global registry**  one `EndpointRegistry` per process; two Lucy-using libraries in the same binary share the same doc surface. | By design |
| **No WebSocket schema**  `request` / `response` schema arguments are only supported on `#[lucy_http]`. WebSocket message schemas are not yet generated. | Planned |
| **MQTT broker URL is user-defined in the UI** as `ws://localhost:9001` by default  update it manually if the broker runs elsewhere. | Planned |
