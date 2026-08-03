[← Back to index](README.md)

# 7. Building the UI

The interactive `/docs` UI is a React single-page app compiled into `crates/lucyd-core/ui/dist/` and embedded into the binary at compile time by `rust-embed`.

**Using `lucyd` from crates.io? There is nothing to do.** The published crate ships the bundle already built; `docs_router()` serves it out of the box. This page is for working on Lucyd itself.

## Building it

```bash
cargo xtask build-ui
```

It runs `npm install` and `npm run build` in `ui/`, and then checks that `crates/lucyd-core/ui/dist/index.html` exists. That last step matters: the output path is set by `build.outDir` in `ui/vite.config.ts` while the embedded path is set by `#[folder]` in `lucyd-core/src/assets.rs`, and nothing else keeps the two in step. Without the check, a bundler that wrote elsewhere would still exit `0` and you would get a binary that answers `404 UI not built` at runtime.

A copy of the built bundle is committed, so a fresh checkout compiles without Node installed. Re-run `build-ui` and commit the result whenever you change anything under `ui/src/`.

## CI / Docker

For a job that only lints or tests, an empty directory is enough to satisfy `rust-embed` at compile time:

```yaml
# GitHub Actions example
- name: Create ui/dist stub for rust-embed
  run: mkdir -p crates/lucyd-core/ui/dist
```

Note the path: it is `crates/lucyd-core/ui/dist`, not the `ui/` source directory at the repository root. Such a build responds `404 UI not built` to doc requests, which is fine where nothing serves them.

**Do not use that stub in a release job.** `.github/workflows/publish.yml` runs the real `cargo xtask build-ui` instead, so the bundle a user downloads is always built from the source at the published tag rather than from whatever copy happened to be committed.

---

Previous: [6. The OpenAPI export](06-openapi-export.md) · Next: [8. UI features](08-ui-features.md)
