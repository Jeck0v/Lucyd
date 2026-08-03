# lucyd-cli

The command-line companion to [`lucyd`](https://crates.io/crates/lucyd).

`lucyd` is a Rust library that turns annotated Axum handlers into interactive API documentation served at `/docs`, including an OpenAPI 3.1 document at `/docs/openapi.json`. This crate is the tool that **validates** that document against a contract you already have.

```bash
cargo install lucyd-cli
```

> The crate is `lucyd-cli`, the binary it installs is `lucyd`.
>
> `cargo install lucyd` installs nothing: `lucyd` is a library and has no executable. The names differ because the library got to crates.io first, and renaming a published crate is not something crates.io allows.

## What it does

```bash
lucyd diff
```

Compares two OpenAPI documents and reports what the API lost, gained, or changed between them:

```
OpenAPI diff report

Missing:
  DELETE /api/users/{id}

Changed:
  GET /api/users/{id}
    response: field "createdAt" removed

Summary:
1 missing, 0 added, 1 changed
```

Exit code `0` when the two agree, `1` when they differ, `2` when the run could not happen at all. That last distinction is the point: a pipeline needs to tell a contract regression apart from a wrong path.

## How it relates to lucyd

They are two halves of the same migration workflow, and you install them differently because they do different jobs.

| | Crate | Where it runs |
|---|---|---|
| Serve the docs, export the contract | `lucyd`, a dependency of your app | in your application |
| Validate the contract | `lucyd-cli`, a binary | in your terminal or CI |

The typical use is moving an existing API onto Lucyd:

1. `cargo xtask import-openapi openapi.yaml` turns your existing spec into `#[lucyd_http]` handler stubs.
2. Your application runs and serves the equivalent document at `/docs/openapi.json`.
3. `lucyd diff` proves the two describe the same API, so nothing was dropped along the way.

`lucyd-cli` is not required to use `lucyd`. It is a migration and CI tool. An application that only serves `/docs` never compiles a line of it: the comparison lives behind the `openapi-diff` feature of `lucyd-core`, which is off by default.

## Usage

```bash
lucyd diff [--from <PATH>] [--to <PATH>] [--format human|json]
           [--fail-on <KIND>[,<KIND>...]] [--strict]

lucyd --update    # reinstall at the latest published version
```

Both paths are optional. A bare `lucyd diff` looks for `openapi.json` / `openapi.yaml` / `openapi.yml` as the baseline and `lucyd-openapi.json` as the candidate, searching upwards from the working directory to the project root. Both sides accept JSON and YAML.

By default the comparison ignores what Lucyd's exporter structurally cannot express (exact status codes, path parameter types, `tags`), because comparing those literally would report a difference on nearly every operation and bury the regressions that matter. `--strict` turns the literal comparison back on.

## Documentation

Full reference, including the comparison table, the JSON report format, and ready-to-use CI jobs: [§13, Validating a migration](https://github.com/Jeck0v/Lucyd/blob/main/docs/13-openapi-diff.md).

## License

MIT
