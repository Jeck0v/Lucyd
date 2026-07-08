[← Back to index](README.md)

# 8. UI features

The Lucy UI at `/docs` is an interactive API explorer similar to Swagger UI.

## HTTP endpoints

- **Collapsible cards** per endpoint, grouped by tag
- **Path parameters**: auto-detected from `{param}` placeholders, with individual inputs
- **Request body**: editable textarea pre-filled with a typed example derived from the request schema
- **Execute**: sends the request from the browser and displays the response with status code and latency
- **cURL preview**: always-visible, updates live as inputs or body change
- **Response schema**: shown after execution (Example Value / Schema tabs)

## WebSocket endpoints

- **Connect / Disconnect** per endpoint with status indicator
- **Message textarea**: pre-filled with a placeholder, `Ctrl+Enter` to send
- **Message log**: incoming (`←`) and outgoing (`→`) messages with timestamps
- **Error display**: RFC 6455 close codes mapped to human-readable descriptions (e.g. `1008 → Policy violation — check auth`)

## MQTT endpoints

- **Shared broker connection**: one `ws://` URL input at the panel top, all topic cards share it
- **Subscribe / Unsubscribe** toggle per topic
- **Publish**: payload input + Publish button per topic
- **Per-topic message log**

## Authentication

Click **Authorize** in the top-right corner to configure global authentication applied to all HTTP requests:

| Type         | Header emitted |
|--------------|----------------|
| Bearer Token | `Authorization: Bearer <token>` |
| API Key      | `<custom-header>: <key>` |
| Basic Auth   | `Authorization: Basic <base64>` |

Auth is persisted in `localStorage` across page reloads. For WebSocket endpoints, a bearer token is forwarded as `?token=<value>` in the URL.

## Models tab

Lists all unique JSON Schemas collected from `request_schema` and `response_schema` across all endpoints, with "Example Value" and "Schema" tabs per model.

---

Previous: [7. Building the UI](07-building-ui.md) · Next: [9. Full example](09-full-example.md)
