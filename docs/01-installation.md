[← Back to index](README.md)

# 1. Installation

Add `lucy` and its required peer dependencies to your `Cargo.toml`:

```toml
[dependencies]
lucy     = { path = "../Lucy/crates/lucy" } # local path during contribution
# lucy = "0.1.9" # from crates.io
schemars = "0.8"                              # needed only if you use request/response schemas
serde    = { version = "1", features = ["derive"] }
axum     = "0.8"
tokio    = { version = "1", features = ["full"] }
```

---

Next: [2. Setup](02-setup.md)
