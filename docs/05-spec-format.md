[← Back to index](README.md)

# 5. The spec format

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

**Optional fields** are absent from the JSON when not provided (no `null` emitted):

| Field             | Present when |
|-------------------|--------------|
| `method`          | `protocol == "Http"` |
| `description`     | `description = "…"` was provided |
| `tags`            | at least one tag was provided |
| `request_schema`  | `request = MyType` was provided |
| `response_schema` | `response = MyType` was provided |

---

Previous: [4. JSON Schema generation](04-json-schema.md) · Next: [6. The OpenAPI export](06-openapi-export.md)
