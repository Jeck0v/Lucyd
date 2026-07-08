[← Back to index](README.md)

# 12. Importing an existing OpenAPI document (`cargo xtask import-openapi`)

This is the reverse of [§6, The OpenAPI export](06-openapi-export.md): instead of turning a running Lucyd application's registered endpoints *into* an OpenAPI document, `cargo xtask import-openapi` turns an existing OpenAPI 3.x document *into* the Rust scaffolding (structs + `#[lucy_http]` handler stubs) that would register those same endpoints. It exists to bootstrap a Lucyd project from a spec you already have,a legacy backend's contract, a spec handed to you by another team, or one exported by a different framework, rather than hand-transcribing every operation.

```bash
cargo xtask import-openapi <file> [--out <path>] [--remove-orphaned]
```

| Argument            | Required | Description |
|---------------------|----------|-------------|
| `<file>`             | yes      | Path to an OpenAPI 3.x document, JSON or YAML (detected automatically, no flag needed) |
| `--out <path>`       | no       | Output file. Defaults to `src/generated_endpoints.rs` |
| `--remove-orphaned`  | no       | Physically delete handlers/structs no longer present in `<file>` instead of only reporting them (see below) |

## What gets generated

- One `struct` per JSON Schema object encountered (named after its `components.schemas` entry, `{Op}Request`/`{Op}Response` when inline, or `{Parent}{Field}` for a nested inline object), deriving `Debug, Clone, Serialize, Deserialize, JsonSchema`. A `$ref`'d component referenced by several operations is only ever generated once and reused.
- One fieldless `enum` per `string` schema carrying an `enum` list, with `#[serde(rename = "...")]` on any variant whose PascalCased name differs from the original value.
- One `#[lucy_http(...)]`-annotated `async fn` stub per operation, with a `todo!("Implement handler")` body, named from `operationId` (or `{method}_{path_slug}` when absent, e.g. `get_api_users_id`) converted to `snake_case`.

## Before

```yaml
# openapi.yaml
openapi: "3.1.0"
paths:
  /api/users:
    post:
      operationId: create_user
      description: Create a new user
      tags: [users]
      requestBody:
        content:
          application/json:
            schema:
              type: object
              properties: { name: { type: string }, email: { type: string } }
              required: [name, email]
      responses:
        "201":
          content:
            application/json:
              schema:
                type: object
                properties: { id: { type: integer }, name: { type: string } }
                required: [id, name]
```

## After (`src/generated_endpoints.rs`)

```rust
use lucyd::lucy_http;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateUserRequest {
    pub email: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateUserResponse {
    pub id: i64,
    pub name: String,
}

/// operationId: create_user
#[lucy_http(
    method = "POST",
    path = "/api/users",
    description = "Create a new user",
    tags = "users",
    request = CreateUserRequest,
    response = CreateUserResponse
)]
pub async fn create_user() -> axum::Json<CreateUserResponse> {
    todo!("Implement handler")
}
```

The stub is deliberately parameter-less: wiring up the request-body extractor is left to whoever implements the handler, since the stub's only job is to compile and carry accurate metadata.

## Incremental merge: re-running is safe

Re-running the importer against an updated spec merges into the existing `--out` file rather than overwriting it. The reconciliation is keyed on the `/// operationId: {id}` doc comment above each stub, not the function's name specifically so a handler can be renamed by hand (`create_user` → `handle_create_user`) without a later re-import reporting a spurious remove-then-add for what is still the same operation. A plain `//` comment can't serve this role: it isn't a token in Rust's grammar, so `syn` drops it on parse and it wouldn't survive a parse → merge → re-emit round trip, whereas a `///` doc comment desugars to a real `#[doc = "..."]` attribute that does.

### On each run:

- **Added**: an operation in `<file>` with no matching marker in `--out` gets a brand-new stub appended.
- **Updated**: an operation whose marker is found gets its `#[lucy_http(...)]` attribute, signature, and associated struct/enum definitions refreshed from the current spec. Its handler body is only overwritten when it is still *exactly* the generated `todo!(...)` call; any other body (i.e. one you've started implementing) is preserved byte-for-byte.
- **Removed**: a marker in `--out` with no matching operation in the current `<file>` is always reported. By default the handler and its code are left in place (non-destructive); pass `--remove-orphaned` to physically delete it, along with any struct/enum it (or another now-removed operation) needed that the current spec no longer generates.
- **Unchanged**: nothing to report.
- **Skipped**: an operation the importer can't safely represent (see [§11, Known limitations](11-limitations.md)); always reported with a reason, never silently dropped.

```
OpenAPI import completed

Added: 1
  + list_orders
Updated: 1
  ~ create_user
Removed: 1
  - delete_legacy_user
Skipped: 1
  ! export_report (uses `oneOf`/`allOf`/`anyOf`/`not`, which don't map to a single Rust type)

Finished with warnings.
```

The `Finished with warnings.` line only appears when at least one operation was skipped.

**Consumer dependencies.** Generated code assumes the target project already depends on `lucyd`, `schemars`, `serde` (with the `derive` feature), and `axum`, processes the same peer dependencies listed in [§1, Installation](01-installation.md). A schema with no fixed `properties` (or no recognized `type` at all) maps to `serde_json::Value` / `HashMap<String, serde_json::Value>`, so `serde_json` should be added too if your spec has any free-form objects.

See [§11, Known limitations](11-limitations.md) for the importer's own scoping limits ($ref resolution, `oneOf`/`allOf`/`anyOf`, parameters, security schemes, hand-added `use` statements).

---

Previous: [11. Known limitations](11-limitations.md) · [Back to index](README.md)
