[← Back to index](README.md)

# 11. Known limitations (v0.1)

| Limitation | Status |
|---|---|
| **No auth guard** on `/docs`: do not expose `docs_router()` on a public-facing interface without adding authentication middleware. | Planned |
| **Single global registry**: one `EndpointRegistry` per process; two Lucy-using libraries in the same binary share the same doc surface. | By design |
| **No WebSocket schema**: `request` / `response` schema arguments are only supported on `#[lucy_http]`. WebSocket message schemas are not yet generated. | Planned |
| **MQTT broker URL is user-defined in the UI** as `ws://localhost:9001` by default — update it manually if the broker runs elsewhere. | Planned |
| **OpenAPI export is HTTP-only**: `#[lucy_ws]`/`#[lucy_mqtt]` endpoints are entirely absent from `/docs/openapi.json`, not represented via vendor extensions either (see [§6, The OpenAPI export](06-openapi-export.md)). Use `/docs/spec.json` for complete, protocol-agnostic metadata in the meantime. | By design (v0.1); vendor-extension preservation is a possible future addition |
| **No security schemes in OpenAPI export**: `EndpointMeta` carries no auth metadata today, so `components.securitySchemes` / operation `security` are always omitted (correctly, there is nothing to export, not a dropped field). | Planned |
| **Axum catch-all path segments (`{*name}`) get no OpenAPI parameter**: OpenAPI's path templating has no wildcard/remainder-of-path equivalent, so such segments are silently omitted from `parameters` rather than emitting an invalid entry. | By design |
| **`cargo xtask import-openapi` only resolves same-document `$ref`s**: a `$ref` pointing outside `#/...` (a separate file, a URL) causes that one operation to be skipped with a reason; the rest of the import still runs. | By design (v0.1) |
| **`oneOf`/`allOf`/`anyOf`/`not`, and `callbacks`/`links`, are skipped by the importer**: none of these compose into a single Rust type (or, for `callbacks`/`links`, aren't handlers at all); the affected operation is skipped with a warning rather than guessed at. | By design |
| **Imported path/query parameters are a doc comment, not a bound struct**: `#[lucy_http]` has no argument for them (only `request`/`response` bind to the JSON body), so generating an unwired `{Op}Params` struct would just be dead code; parameter names are listed in a `/// Path parameters: ...` / `/// Query parameters: ...` doc line on the stub instead. | By design |
| **The importer ignores security schemes entirely**: same stance as the OpenAPI export: `#[lucy_http]` models no auth today, so there is nothing to import, not a dropped field. | Planned |
| **A hand-added top-level `use` in the generated file is not preserved across a re-import**: the file's `use` block is always re-emitted fresh from a fixed template; only fn bodies (and, implicitly, any other item the importer doesn't manage: `impl` blocks, unmarked helper fns, ...) survive a re-run untouched. Prefer fully-qualified paths inside a handwritten handler body, or re-add the import after each re-run. | By design (v0.1) |

---

Previous: [10. Architecture overview](10-architecture.md) · Next: [12. Importing an OpenAPI document](12-import-openapi.md)
