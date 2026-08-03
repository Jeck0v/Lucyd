[← Back to index](README.md)

# 10. Architecture overview

```
your-axum-app
│
├── #[lucyd_http / ws / mqtt]        ← proc-macros (crates/lucyd-macro)
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

| Crate        | Published | Role |
|--------------|-----------|------|
| `lucyd`       | yes | Public facade: the only crate consumers import |
| `lucyd-macro` | yes | Proc-macros: parse and validate `#[lucyd_*]` attributes, emit `inventory::submit!` |
| `lucyd-core`  | yes | Runtime: global registry, spec generation, Axum router, asset serving, OpenAPI diff |
| `lucyd-types` | yes | Shared types: `Protocol`, `EndpointMeta`, `EndpointMetaStatic` |
| `lucyd-cli`   | yes | The `lucyd` binary: file discovery, YAML/JSON loading, exit codes |
| `xtask`      | no  | Build tooling: `cargo xtask build-ui`, `build-docs`, `import-openapi` |

Two of these are meant to be named directly: `lucyd` as a dependency, `lucyd-cli` as an installed binary. The rest are reached through them.

## Dependency flow

Consumers only need `lucyd`:

```
your-crate  →  lucyd  →  lucyd-macro
                       →  lucyd-core  →  lucyd-types
                                      →  inventory
                                      →  rust-embed
                       →  lucyd-types
                       →  inventory  (re-exported as lucyd::_private::inventory)
                       →  schemars   (re-exported as lucyd::_private::schemars)
                       →  serde_json (re-exported as lucyd::_private::serde_json)

lucyd-cli   →  lucyd-core  (feature "openapi-diff")
            →  clap, serde_json, serde_yaml_ng
```

Macro-generated code references `::lucyd::_private::*` so consumer crates only need `lucyd` in `Cargo.toml`. That indirection is also why the internal crate names are free to change: no user code ever spells them.

## Where a feature's logic lives

The recurring rule is that `lucyd-core` decides and the front-ends only sequence. The clearest case is the OpenAPI diff:

| Concern | Crate |
|---|---|
| What counts as a difference between two documents | `lucyd-core` |
| How a report reads, in both human and JSON form | `lucyd-core` |
| Whether a report should fail a build | `lucyd-core` |
| Which files to compare when none were named | `lucyd-cli` |
| Parsing YAML as well as JSON | `lucyd-cli` |
| Turning a report into a process exit code | `lucyd-cli` |

A report produced by a test is therefore byte-identical to one produced by the binary. The comparison sits behind the `openapi-diff` feature of `lucyd-core`, off by default, so an application that only serves `/docs` never compiles it.

The same split applies elsewhere: `lucyd-macro` validates attribute arguments and emits registration code but knows nothing about the spec format, and `lucyd-core` owns both the internal spec and the OpenAPI export without knowing how either is requested.

---

Previous: [9. Full example](09-full-example.md) · Next: [11. Known limitations](11-limitations.md)
