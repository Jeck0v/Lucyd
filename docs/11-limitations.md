[← Back to index](README.md)

# 11. Known limitations (v0.2)

| Limitation | Status |
|---|---|
| **No auth guard** on `/docs`: do not expose `docs_router()` on a public-facing interface without adding authentication middleware. | Planned |
| **Single global registry**: one `EndpointRegistry` per process; two Lucyd-using libraries in the same binary share the same doc surface. | By design |
| **No WebSocket schema**: `request` / `response` schema arguments are only supported on `#[lucyd_http]`. WebSocket message schemas are not yet generated. | Planned |
| **MQTT broker URL is user-defined in the UI** as `ws://localhost:9001` by default: update it manually if the broker runs elsewhere. | Planned |
| **OpenAPI export is HTTP-only**: `#[lucyd_ws]`/`#[lucyd_mqtt]` endpoints are entirely absent from `/docs/openapi.json`, not represented via vendor extensions either (see [§6, The OpenAPI export](06-openapi-export.md)). Use `/docs/spec.json` for complete, protocol-agnostic metadata in the meantime. | By design (v0.2); vendor-extension preservation is a possible future addition |
| **No security schemes in OpenAPI export**: `EndpointMeta` carries no auth metadata today, so `components.securitySchemes` / operation `security` are always omitted (correctly, there is nothing to export, not a dropped field). | Planned |
| **Axum catch-all path segments (`{*name}`) get no OpenAPI parameter**: OpenAPI's path templating has no wildcard/remainder-of-path equivalent, so such segments are silently omitted from `parameters` rather than emitting an invalid entry. | By design |
| **`cargo xtask import-openapi` only resolves same-document `$ref`s**: a `$ref` pointing outside `#/...` (a separate file, a URL) causes that one operation to be skipped with a reason; the rest of the import still runs. | By design (v0.2) |
| **`oneOf`/`allOf`/`anyOf`/`not`, and `callbacks`/`links`, are skipped by the importer**: none of these compose into a single Rust type (or, for `callbacks`/`links`, aren't handlers at all); the affected operation is skipped with a warning rather than guessed at. | By design |
| **Imported path parameters are a doc comment, not a bound struct**: the path template already names them, and `#[lucyd_http]` has no argument that binds them, so they are listed in a `/// Path parameters: ...` doc line on the stub. Query parameters are no longer in this row — they import as a generated `{Op}Params` struct bound by `query = T`. | By design |
| **A declared query parameter is documented, not extracted**: `query = T` describes the query string for the UI and the OpenAPI export; parsing it is still `Query<T>`'s job in the handler signature, exactly as `request = T` describes a body that `Json<T>` extracts. Nothing checks that the two agree. | By design |
| **The importer ignores security schemes entirely**: same stance as the OpenAPI export: `#[lucyd_http]` models no auth today, so there is nothing to import, not a dropped field. | Planned |
| **`lucyd diff` reads documents from disk only**: there is no `--from https://...`. Fetching a spec is `curl`'s job, and building an HTTP client into the tool would drag in TLS, proxies, redirects and auth for something one pipe already does. Export first, then diff. | By design |
| **`oneOf`/`allOf`/`anyOf` are compared as written, not composed first**: two schemas that describe the same contract through different composition (an inlined `allOf` against its flattened equivalent) are reported as a difference. Normalising them means implementing schema composition, which is a much larger piece of work than the rest of the diff put together. | By design (v0.2) |
| **`security` and `servers` are never compared**, in either mode: Lucyd's exporter emits neither (see the security-schemes row above), so every comparison would report the baseline's entries as removed on every operation. The day the registry carries auth metadata, this becomes comparable. | By design |
| **A hand-added top-level `use` in the generated file is not preserved across a re-import**: the file's `use` block is always re-emitted fresh from a fixed template; only fn bodies (and, implicitly, any other item the importer doesn't manage: `impl` blocks, unmarked helper fns, ...) survive a re-run untouched. Prefer fully-qualified paths inside a handwritten handler body, or re-add the import after each re-run. | By design (v0.2) |

---

Previous: [10. Architecture overview](10-architecture.md) · Next: [12. Importing an OpenAPI document](12-import-openapi.md) · [13. Validating a migration](13-openapi-diff.md)
