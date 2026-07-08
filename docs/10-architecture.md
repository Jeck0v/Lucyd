[← Back to index](README.md)

# 10. Architecture overview

```
your-axum-app
│
├── #[lucy_http / ws / mqtt]        ← proc-macros (crates/lucy-macro)
│         │                            parse & validate args at compile time
│         ▼
│   inventory::submit!               ← linker-magic static registration
│   EndpointMetaStatic { ... }          fn pointers for schema generation
│         │
│         ▼ (first request to /docs/spec.json)
│   global_registry()               ← OnceLock<Mutex<EndpointRegistry>>
│   drains inventory::iter()           calls schema fn pointers once
│         │
│         ▼
│   GET /docs/spec.json             ← generate_spec() serialises registry to JSON
│
└── GET /docs/*                     ← React SPA served from rust-embed
                                       built by: cargo xtask build-ui
```

## Crate responsibilities

| Crate        | Role |
|--------------|------|
| `lucy`       | Public facade: the only crate consumers import |
| `lucy-macro` | Proc-macros: parse and validate `#[lucy_*]` attributes, emit `inventory::submit!` |
| `lucy-core`  | Runtime: global registry, spec generation, Axum router, asset serving |
| `lucy-types` | Shared types: `Protocol`, `EndpointMeta`, `EndpointMetaStatic` |
| `xtask`      | Build tooling: `cargo xtask build-ui` |

## Dependency flow

Consumers only need `lucy`:

```
your-crate  →  lucy  →  lucy-macro
                      →  lucy-core  →  lucy-types
                                    →  inventory
                                    →  rust-embed
                      →  lucy-types
                      →  inventory  (re-exported as lucy::_private::inventory)
                      →  schemars   (re-exported as lucy::_private::schemars)
                      →  serde_json (re-exported as lucy::_private::serde_json)
```

Macro-generated code references `::lucy::_private::*` so consumer crates only need `lucy` in `Cargo.toml`.

---

Previous: [9. Full example](09-full-example.md) · Next: [11. Known limitations](11-limitations.md)
