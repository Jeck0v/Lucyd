[← Back to index](README.md)

# 6. The OpenAPI export (`/docs/openapi.json`)

Alongside the internal `/docs/spec.json`, Lucy exposes an **OpenAPI 3.1** document at:

```
GET /docs/openapi.json
```

This is a **derived, additive** view of the exact same endpoint registry that backs `/docs/spec.json`. The internal spec remains the source of truth and is left completely unchanged; the OpenAPI document is generated purely for interoperability with standard OpenAPI tooling — Swagger UI, [`openapi-generator`](https://openapi-generator.tech/), [`orval`](https://orval.dev/), Postman import, and so on.

## Scope: HTTP endpoints only

Only `#[lucy_http]` endpoints appear in `paths`. `#[lucy_ws]` and `#[lucy_mqtt]` endpoints are entirely absent from the exported document — not simplified, not listed under a placeholder entry, absent.

### **Why** 
OpenAPI 3.1 has no native object for a WebSocket upgrade or an MQTT topic. Two designs were considered:

1. Preserve them anyway, attached to a Path Item as vendor extensions (`x-lucyd-ws` / `x-lucyd-mqtt`) — valid OpenAPI (the spec explicitly allows `x-*` extension fields anywhere), so no metadata is silently lost.
2. Omit them from this export entirely and keep `/docs/spec.json` as the single, protocol-agnostic source of truth; a future, separate `/docs/asyncapi.json` export would cover WebSocket/MQTT properly using the [AsyncAPI](https://www.asyncapi.com/) standard instead.

**This version implements (2).** Option (1) is not implemented — it's a possible future addition, not a bug or an oversight. If you need WebSocket or MQTT metadata today, read it from `/docs/spec.json` instead: it is unaffected by this scoping and lists every registered endpoint regardless of protocol (see [§5, The spec format](05-spec-format.md)).

## Response shape (abbreviated)

```json
{
  "openapi": "3.1.0",
  "info": { "title": "Lucyd API", "version": "0.1.9" },
  "paths": {
    "/api/users": {
      "post": {
        "operationId": "create_user",
        "description": "Create a new user account",
        "tags": ["users"],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": { "$ref": "#/components/schemas/CreateUserRequest" }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Successful response",
            "content": {
              "application/json": {
                "schema": { "$ref": "#/components/schemas/User" }
              }
            }
          }
        }
      }
    }
  },
  "components": {
    "schemas": {
      "CreateUserRequest": { "type": "object", "properties": { "name": { "type": "string" } } },
      "User": { "type": "object", "properties": { "id": { "type": "integer" } } }
    }
  }
}
```

- **Schema hoisting & naming.** Every request/response JSON Schema is hoisted into `components.schemas` and referenced with `$ref`. Each schema is named after its `title` (falling back to `{endpoint}_request` / `{endpoint}_response` when no title is present), and identical schemas are de-duplicated document-wide; conflicting schemas that share a name are suffixed (`Name_2`, `Name_3`, ...).
- **`info` defaults.** `info.title` and `info.version` currently default to fixed values (`"Lucyd API"` and the crate's own version). They are not yet user-configurable.
- **No security schemes.** Lucy carries no auth metadata in its endpoint registry today, so `components.securitySchemes` and operation-level `security` are always omitted (no placeholder data is invented).

## Verify with real tooling

```bash
curl -s http://localhost:8080/docs/openapi.json -o /tmp/lucyd-openapi.json
npx @openapitools/openapi-generator-cli generate -i /tmp/lucyd-openapi.json -g typescript-fetch -o /tmp/lucyd-client
npx orval --input /tmp/lucyd-openapi.json --output /tmp/lucyd-orval-out
```

---

Previous: [5. The spec format](05-spec-format.md) · Next: [7. Building the UI](07-building-ui.md)
