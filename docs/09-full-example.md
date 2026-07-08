[← Back to index](README.md)

# 9. Full example

A complete `main.rs` using all three macros: HTTP, WebSocket, and MQTT together.

```rust
// src/main.rs
use axum::{routing::get, Router};
use lucy::{docs_router, lucy_http, lucy_mqtt, lucy_ws};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

// Types
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

// HTTP

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

// WebSocket

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

// MQTT

#[lucy_mqtt(
    topic       = "flipper/physics/collision",
    tags        = "iot",
    description = "Collision events from the physics engine",
)]
async fn on_collision(_payload: bytes::Bytes) {}

#[lucy_mqtt(
    topic       = "sensors/+/temperature",
    tags        = "iot",
    description = "Temperature readings — `+` matches any sensor ID",
)]
async fn on_temperature(_payload: bytes::Bytes) {}

// Entry point

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

Previous: [8. UI features](08-ui-features.md) · Next: [10. Architecture overview](10-architecture.md)
