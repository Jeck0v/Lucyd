[← Back to index](README.md)

# 4. JSON Schema generation

Lucy integrates with [`schemars`](https://docs.rs/schemars) to generate JSON Schemas at startup for any type passed to `request =` or `response =`.

### **Requirements**

1. Add `schemars` to your `Cargo.toml` (see [Installation](01-installation.md)).
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

Previous: [3. Attribute macros](03-macros.md) · Next: [5. The spec format](05-spec-format.md)
