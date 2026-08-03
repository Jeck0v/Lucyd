[← Back to index](README.md)

# 1. Installation

## The library

Add `lucyd` and its required peer dependencies to your `Cargo.toml`:

```toml
[dependencies]
lucyd    = "0.2.0"
schemars = "0.8"                              # needed only if you use request/response schemas
serde    = { version = "1", features = ["derive"] }
axum     = "0.8"
tokio    = { version = "1", features = ["full"] }
```

Contributing to Lucyd itself, or testing an unreleased change? Point at your checkout instead:

```toml
lucyd = { path = "../Lucyd/crates/lucyd" }
```

`lucyd` is the only Lucyd crate you ever name. It re-exports everything you need and pulls in `lucyd-core`, `lucyd-macro` and `lucyd-types` on its own; the macros it exports expand to paths that go back through it, so those three are implementation details you never depend on directly.

## The CLI (optional)

Migrating an existing API onto Lucyd, or guarding its contract in CI? There is a separate binary for that:

```bash
cargo install lucyd-cli
```

> **`cargo install lucyd` installs nothing.** `lucyd` is a library, it has no executable, and the command fails with *"there is nothing to install"*. The tool ships as `lucyd-cli` because the name `lucyd` on crates.io belongs to the library.
>
> The crate is `lucyd-cli`. The command it gives you is `lucyd`.

```bash
lucyd --version
lucyd --update     # reinstall at the latest published version
```

You do not need it to use Lucyd. See [§13, Validating a migration](13-openapi-diff.md).

---

Next: [2. Setup](02-setup.md)
