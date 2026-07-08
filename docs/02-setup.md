[← Back to index](README.md)

# 2. Setup

Merge `docs_router()` into your Axum application that is the only wiring required.

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

`docs_router()` registers:

- `GET /docs` and `GET /docs/`: the embedded React UI (SPA)
- `GET /docs/spec.json`: the machine-readable endpoint catalogue
- `GET /docs/openapi.json`: an OpenAPI 3.1 document for standard tooling (HTTP endpoints only)
- `GET /docs/*path`: static assets (JS, CSS) embedded in the binary

---

Previous: [1. Installation](01-installation.md) · Next: [3. Attribute macros](03-macros.md)
