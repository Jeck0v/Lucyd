[← Back to index](README.md)

# 7. Building the UI

The interactive `/docs` UI is a React single-page app bundled into the binary at compile time. Build it once with `cargo xtask build-ui` before compiling `lucy-core` for a production profile.

## CI / Docker

When building without running `cargo xtask build-ui` first (e.g. in CI or a lint-only job):

```yaml
# GitHub Actions example
- name: Create ui/dist stub for rust-embed
  run: mkdir -p ui/dist
```

An empty `ui/dist/` satisfies `rust-embed` at compile time. The binary will respond with `404 UI not built` for doc requests, which is acceptable for CI where only tests matter.

---

Previous: [6. The OpenAPI export](06-openapi-export.md) · Next: [8. UI features](08-ui-features.md)
