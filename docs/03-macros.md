[← Back to index](README.md)

# 3. Attribute macros

Import the macros you need at the top of each handler file:

```rust
use lucyd::{lucyd_http, lucyd_ws, lucyd_mqtt};
```

All three macros are **zero-cost at runtime**: they register metadata at link time via the `inventory` crate without adding any overhead to request processing.

## Renamed in 0.2

These macros used to be spelled `lucy_http`, `lucy_ws` and `lucy_mqtt`, from before the crates were unified under the `lucyd` name.

**The old names still work.** They are the same macros, expand to the same code, and register endpoints identically. They emit a deprecation warning naming their replacement:

```
warning: use of deprecated macro `lucy_http`: renamed to `lucyd_http`, to match the crate name
```

Migrating is a find-and-replace of `lucy_` with `lucyd_` in your attributes and imports. If your build runs with `-D warnings`, the warning is an error and the rename is required rather than optional.

## Contents

- [`#[lucyd_http]`](#lucyd_http)
- [Query parameters](#query-parameters)
- [`#[lucyd_ws]`](#lucyd_ws)
- [`#[lucyd_mqtt]`](#lucyd_mqtt)

---

## `#[lucyd_http]`

Marks an Axum HTTP handler for documentation and interactive testing.

**Arguments**

| Argument      | Required | Type      | Description |
|---------------|----------|-----------|-------------|
| `method`      | yes      | string    | HTTP verb in uppercase: `"GET"`, `"POST"`, `"PUT"`, `"DELETE"`, `"PATCH"` |
| `path`        | yes      | string    | Full URL path, must start with `/` (e.g. `"/api/users"`) |
| `description` | no       | string    | Human-readable explanation shown in the UI |
| `tags`        | no       | string    | Comma-separated group labels (e.g. `"users, admin"`) used to visually group endpoints |
| `query`       | no       | type path | Rust type deriving `JsonSchema`; declares the query string (see [Query parameters](#query-parameters)) |
| `request`     | no       | type path | Rust type deriving `JsonSchema`; generates the request body schema and pre-fills the UI textarea |
| `response`    | no       | type path | Rust type deriving `JsonSchema`; generates the response schema shown after execution |

`path` must be a path template only. A query string written into it — `path = "/api/scores?limit=10"` — is a compile error, because it would corrupt the OpenAPI path template and make the endpoint stop matching itself in `lucyd diff`, which splits the path on `/` to identify an operation.

### **Examples**

```rust
use lucyd::lucyd_http;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Minimal: required fields only
#[lucyd_http(method = "GET", path = "/health")]
async fn health() -> &'static str {
    "ok"
}

// With description and tag
#[lucyd_http(
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

#[lucyd_http(
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

Lucyd validates arguments at compile time:

```rust
// Error: missing required `method` argument
#[lucyd_http(path = "/health")]
async fn bad() {}

// Error: duplicate `path` argument
#[lucyd_http(method = "GET", path = "/a", path = "/b")]
async fn bad() {}

// Error: unknown argument `verb`
#[lucyd_http(verb = "GET", path = "/health")]
async fn bad() {}

// Error: `path` must not contain a query string; declare query parameters with `query = T`
#[lucyd_http(method = "GET", path = "/api/scores?limit=10")]
async fn bad() {}
```

---

## Query parameters

`query = T` declares the query string as a type, the same way `request` and `response` declare bodies. `T` derives `JsonSchema`, and schemars supplies the parameter names, their types, and their doc comments; whether a field is `Option<T>` is what makes the parameter optional or required.

```rust
use axum::extract::Query;
use lucyd::lucyd_http;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct ScoreFilters {
    /// Board the scores belong to.
    pub board: String,
    /// Maximum number of rows returned.
    pub limit: Option<u32>,
}

#[lucyd_http(
    method   = "GET",
    path     = "/api/scores",
    query    = ScoreFilters,
    response = Scores,
)]
async fn scores(Query(filters): Query<ScoreFilters>) -> axum::Json<Scores> { /* ... */ }
```

The declaration is not wired into the handler for you: `Query<ScoreFilters>` in the signature is what actually parses the query string. The macro documents it, and nothing more — exactly as `request = T` documents a body that `Json<T>` extracts.

What the declaration buys:

- **`/docs` renders a typed input per parameter**, with the doc comment as help text and a `*` on the required ones, and sends them as a query string. The cURL preview shows the URL that was actually requested.
- **`/docs/openapi.json` emits one `in: query` Parameter Object per field**, carrying `required`, `schema` and `description`. `Option<T>`'s `["integer", "null"]` is exported as a plain `integer`: absence is already stated by `required: false`.
- **`/docs/spec.json` carries the schema** under `query_schema`, and the Models tab lists the type alongside request and response models.

### On WebSocket

`#[lucyd_ws]` takes the same argument, and it matters more there. The browser `WebSocket` constructor takes only `(url, protocols)` and cannot set an `Authorization` header, so the query string is the only way to pass anything at connect time:

```rust
#[derive(Deserialize, JsonSchema)]
pub struct ScreenAuth {
    /// Signed JWT authorising this screen.
    pub access_token: String,
}

#[lucyd_ws(path = "/ws/screen/{screen_id}", query = ScreenAuth)]
async fn ws_screen(
    ws: WebSocketUpgrade,
    Query(auth): Query<ScreenAuth>,
) -> impl IntoResponse { /* ... */ }
```

Because `access_token` is `String` and not `Option<String>`, axum's `Query` extractor rejects an upgrade that omits it with `400`, before `on_upgrade` runs. Declaring it is what lets `/docs` connect at all.

If a bearer token is configured globally, `/docs` appends it as `?token=`. An endpoint that declares its own parameter named `token` takes precedence: the entered value is sent and nothing is injected, so the two never collide.

### Not on MQTT

`#[lucyd_mqtt]` rejects `query` as an unknown argument. MQTT topics have no query string; the analogous feature is wildcard subscriptions, which `topic` already supports through `+` and `#`.

---

## `#[lucyd_ws]`

Marks an Axum WebSocket upgrade handler for documentation and interactive testing.

**Arguments**

| Argument      | Required | Type   | Description |
|---------------|----------|--------|-------------|
| `path`        | yes      | string | WebSocket upgrade path (e.g. `"/ws/events"`) |
| `description` | no       | string | Human-readable explanation shown in the UI |
| `tags`        | no       | string | Comma-separated group labels |
| `query`       | no       | type path | Rust type deriving `JsonSchema`; declares the upgrade URL's query string (see [Query parameters](#query-parameters)) |

### **Example**

```rust
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use lucyd::lucyd_ws;

#[lucyd_ws(
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

## `#[lucyd_mqtt]`

Marks an MQTT topic handler for documentation generation.

**Arguments**

| Argument      | Required | Type   | Description |
|---------------|----------|--------|-------------|
| `topic`       | yes      | string | MQTT topic string, supports wildcards (e.g. `"sensors/+/temperature"`) |
| `description` | no       | string | Human-readable explanation shown in the UI |
| `tags`        | no       | string | Comma-separated group labels |

###**Example**

```rust
use lucyd::lucyd_mqtt;

#[lucyd_mqtt(
    topic       = "sensors/temperature",
    tags        = "iot",
    description = "Current temperature from IoT sensors",
)]
async fn on_temperature(payload: bytes::Bytes) { /* ... */ }

#[lucyd_mqtt(
    topic       = "devices/+/status",
    tags        = "iot",
    description = "Device status, `+` matches any single device ID",
)]
async fn on_device_status(payload: bytes::Bytes) { /* ... */ }
```

---

Previous: [2. Setup](02-setup.md) · Next: [4. JSON Schema generation](04-json-schema.md)
