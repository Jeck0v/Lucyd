[← Back to index](README.md)

# 3. Attribute macros

Import the macros you need at the top of each handler file:

```rust
use lucy::{lucy_http, lucy_ws, lucy_mqtt};
```

All three macros are **zero-cost at runtime**: they register metadata at link time via the `inventory` crate without adding any overhead to request processing.

## Contents

- [`#[lucy_http]`](#lucy_http)
- [`#[lucy_ws]`](#lucy_ws)
- [`#[lucy_mqtt]`](#lucy_mqtt)

---

## `#[lucy_http]`

Marks an Axum HTTP handler for documentation and interactive testing.

**Arguments**

| Argument      | Required | Type      | Description |
|---------------|----------|-----------|-------------|
| `method`      | yes      | string    | HTTP verb in uppercase: `"GET"`, `"POST"`, `"PUT"`, `"DELETE"`, `"PATCH"` |
| `path`        | yes      | string    | Full URL path, must start with `/` (e.g. `"/api/users"`) |
| `description` | no       | string    | Human-readable explanation shown in the UI |
| `tags`        | no       | string    | Comma-separated group labels (e.g. `"users, admin"`) used to visually group endpoints |
| `request`     | no       | type path | Rust type deriving `JsonSchema` — generates the request body schema and pre-fills the UI textarea |
| `response`    | no       | type path | Rust type deriving `JsonSchema` — generates the response schema shown after execution |

### **Examples**

```rust
use lucy::lucy_http;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Minimal — required fields only
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

### **Compile errors**

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

## `#[lucy_ws]`

Marks an Axum WebSocket upgrade handler for documentation and interactive testing.

**Arguments**

| Argument      | Required | Type   | Description |
|---------------|----------|--------|-------------|
| `path`        | yes      | string | WebSocket upgrade path (e.g. `"/ws/events"`) |
| `description` | no       | string | Human-readable explanation shown in the UI |
| `tags`        | no       | string | Comma-separated group labels |

### **Example**

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

## `#[lucy_mqtt]`

Marks an MQTT topic handler for documentation generation.

**Arguments**

| Argument      | Required | Type   | Description |
|---------------|----------|--------|-------------|
| `topic`       | yes      | string | MQTT topic string, supports wildcards (e.g. `"sensors/+/temperature"`) |
| `description` | no       | string | Human-readable explanation shown in the UI |
| `tags`        | no       | string | Comma-separated group labels |

###**Example**

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
    description = "Device status — `+` matches any single device ID",
)]
async fn on_device_status(payload: bytes::Bytes) { /* ... */ }
```

---

Previous: [2. Setup](02-setup.md) · Next: [4. JSON Schema generation](04-json-schema.md)
