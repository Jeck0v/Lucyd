[← Back to index](README.md)

# 13. Validating a migration (`lucyd diff`)

`lucyd diff` compares two OpenAPI documents and tells you what an API lost, gained, or changed between them. It is the third piece of the migration workflow:

| Step | Tool | Direction |
|---|---|---|
| 1. Import | [`cargo xtask import-openapi`](12-import-openapi.md) | spec → Rust scaffolding |
| 2. Export | [`GET /docs/openapi.json`](06-openapi-export.md) | running app → spec |
| 3. **Validate** | **`lucyd diff`** | **spec vs spec** |

Steps 1 and 2 move an API onto Lucyd. Step 3 is what proves nothing fell out on the way.

## Contents

- [Installing the binary](#installing-the-binary)
- [The five-minute version](#the-five-minute-version)
- [Command reference](#command-reference)
- [How the documents are found](#how-the-documents-are-found)
- [What is compared](#what-is-compared)
- [Reading a report](#reading-a-report)
- [Exit codes](#exit-codes)
- [Choosing what fails the build](#choosing-what-fails-the-build)
- [In CI](#in-ci)
- [Where the code lives](#where-the-code-lives)

---

## Installing the binary

```bash
cargo install lucyd-cli
```

> **`cargo install lucyd` installs nothing.**
> `lucyd` is a library, it has no executable to install and the command fails with *"there is nothing to install"*. The binary lives in a separate crate, `lucyd-cli`, because the name `lucyd` on crates.io was already taken by the library your application depends on.
>
> The crate is `lucyd-cli`. The command it gives you is `lucyd`.

Check it landed:

```bash
lucyd --version
```

To upgrade later:

```bash
lucyd --update
```

which is a thin wrapper around `cargo install lucyd-cli --force`, so it needs `cargo` on your `PATH`. Nothing stops you from running that command yourself instead.

You do **not** need `lucyd-cli` to use Lucyd. It is a migration and CI tool. An application that only serves `/docs` never compiles a line of it: the comparison lives behind the `openapi-diff` feature of `lucyd-core`, off by default.

## The five-minute version

From the root of the project you are migrating:

```bash
# 1. Your existing contract, the one you must not regress against.
#    Already there if you imported from it: openapi.json / openapi.yaml / openapi.yml

# 2. What your Lucyd application actually serves today.
cargo run &
curl -o lucyd-openapi.json http://localhost:3000/docs/openapi.json

# 3. Compare.
lucyd diff
```

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

Read: one endpoint you have not implemented yet, and one whose response dropped a field. Exit code `1`.

When the migration is complete:

```
OpenAPI diff report

No differences found.
```

Exit code `0`.

## Command reference

```bash
lucyd diff [--from <PATH>] [--to <PATH>] [--format human|json]
           [--fail-on <KIND>[,<KIND>...]] [--strict]
```

| Flag | Default | Description |
|---|---|---|
| `--from <PATH>` | discovered | Baseline document: the contract being protected. JSON or YAML |
| `--to <PATH>` | discovered | Candidate document: what is checked against the baseline. JSON or YAML |
| `--format` | `human` | `human` for a terminal, `json` for a pipeline that parses the output |
| `--fail-on` | `missing,changed` | Which categories of difference make the command exit `1` |
| `--strict` | off | Also compare the facets Lucyd's exporter cannot express (see [What is compared](#what-is-compared)) |

The direction matters. `--from` is the reference; `--to` is what gets judged against it. Swapping them turns every `missing` into an `added`.

Both formats accept JSON and YAML on either side, detected from the content rather than the extension. A YAML baseline against a JSON export is the normal case.

## How the documents are found

A migration gets checked over and over as it progresses, so the common invocation is a bare `lucyd diff` from anywhere inside the project. Each side is looked up by convention, independently, which is also why naming only one of the two works fine.

| Side | Flag | File names tried, in order |
|---|---|---|
| Baseline | `--from` | `openapi.json`, `openapi.yaml`, `openapi.yml` |
| Candidate | `--to` | `lucyd-openapi.json` |

The search starts in the working directory and walks **upwards**, stopping at the first directory containing a `Cargo.toml`. That is the same rule `cargo` and `git` use to find their own files, and it means:

- a bare `lucyd diff` works from a subdirectory, not just from the project root;
- the nearest document wins over one further up, so a per-crate spec beats a workspace-wide one;
- a document sitting *outside* the project is never picked up by accident.

Walking downwards instead would be slower and would happily pick a fixture out of `tests/` or a vendored spec out of `target/`.

The two sides have different conventional names on purpose: they can never resolve to the same file.

When a search comes up empty, the error says what it looked for, where, and both ways to fix it:

```
error: no Lucyd export found.
  Looked for lucyd-openapi.json in '/home/me/api' and its parent directories, up to the project root.
  Export it from your running application first:
    curl -o lucyd-openapi.json http://localhost:3000/docs/openapi.json
  Or name it explicitly with `--to <path>`.
```

## What is compared

This is the section worth reading before you reach for `--strict`.

Lucyd's exporter is deliberately narrower than OpenAPI. It emits one `200` response per operation, types every path parameter as `string`, and writes no `summary`, no `security`, no `servers`. Compared *literally* against a hand-written spec, that reports a difference on nearly every operation and buries the regressions that actually matter.

So the default profile compares what callers depend on and ignores what Lucyd structurally cannot express. `--strict` turns the literal comparison back on, which is what you want when diffing two documents that neither side generated (two revisions of a hand-written spec, for instance).

| Facet | Default | `--strict` |
|---|---|---|
| Endpoint present or absent (method + path) | yes | yes |
| Request body schema | yes | yes |
| Success response body schema | yes, matched across differing success codes | yes, matched code by code |
| Other response bodies (`404`, `422`, ...) | no | yes, for codes both sides declare |
| Status codes declared on one side only | no | yes |
| Path parameter renamed | yes | yes |
| Query / header / cookie parameter added or removed | yes | yes |
| Parameter became required, or stopped being required | yes | yes |
| Parameter declared type | no | yes |
| `tags`, `deprecated` | no | yes |
| `description`, `summary`, `title`, `example`, `default` | **never** | **never** |
| `security`, `servers` | **never** | **never** |

Two consequences of the default profile worth stating plainly:

**A spec's `201` matched against Lucyd's `200` is not a regression.** The bodies still get compared, across the differing codes, so a dropped response field is still caught. Only the code itself is ignored. Under `--strict` you get `response(201): no longer declared` and `response(200): newly declared` instead.

**A numeric `{id}` exported as a string is not a regression.** Lucyd has no way to express the type today, so reporting it would fire on every numeric identifier in a real spec without describing anything you can act on.

Prose is never a change, in either mode. Rewording a `description` is not an API change, and a diff that failed CI over one would be switched off within a week.

### Schema comparison

Body schemas are compared structurally, not textually. The walk descends through `properties`, array `items`, and `$ref`s (resolved within their own document), comparing `type`, `format`, `enum`, and the `required` list.

Two details worth knowing:

- **`$ref` cycles terminate.** A self-referential schema is descended once per `$ref` pair, then skipped.
- **An unresolvable `$ref` is reported, never assumed equal.** It shows up as a change of kind `unresolved` saying the contents were not compared, which is honest about what the tool did rather than quietly passing.

### What is not compared

`lucyd diff` compares HTTP operations. WebSocket and MQTT endpoints are absent from the OpenAPI export entirely (see [§6](06-openapi-export.md)), so there is nothing on either side to compare. `oneOf` / `allOf` / `anyOf` are compared as the values they are, not composed and normalised first. See [§11, Known limitations](11-limitations.md).

## Reading a report

### Human format

Sections only appear when they have content, so a report is as short as the situation allows.

```
OpenAPI diff report

Missing:
  DELETE /api/users/{id}

Added:
  POST /api/ping

Changed:
  GET /api/users/{id}
    response: field "createdAt" removed
    query parameter "page": is now required

Summary:
1 missing, 1 added, 1 changed
```

Under `Changed:`, each line is `location: detail`. The location names where the difference sits (`request`, `response`, `response(404)`, `query parameter "page"`), the detail states what differs.

### JSON format

`--format json` prints the same report as a stable envelope, indented so it stays readable in a CI log.

```json
{
  "lucydDiffVersion": 1,
  "summary": { "missing": 1, "added": 1, "changed": 1 },
  "missing": [ { "method": "DELETE", "path": "/api/users/{id}" } ],
  "added": [ { "method": "POST", "path": "/api/ping" } ],
  "changed": [
    {
      "method": "GET",
      "path": "/api/users/{id}",
      "changes": [
        {
          "kind": "responseSchema",
          "location": "response",
          "detail": "field \"createdAt\" removed"
        }
      ]
    }
  ]
}
```

`lucydDiffVersion` is bumped whenever the shape changes in a way an existing consumer could not parse, so a script can check it before trusting the rest.

Every change carries a `kind`, which is what makes the output filterable:

| `kind` | Meaning |
|---|---|
| `requestSchema` | The `application/json` request body |
| `responseSchema` | An `application/json` response body |
| `statusCode` | A status code declared on only one side (`--strict` only) |
| `parameter` | A path, query, header or cookie parameter |
| `metadata` | `tags` or `deprecated` (`--strict` only) |
| `unresolved` | A `$ref` that could not be followed, so what it pointed at was not compared |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | No difference in any category `--fail-on` selected |
| `1` | At least one such difference: the report is on stdout |
| `2` | The run could not happen: bad usage, missing file, unparsable document |

`1` and `2` are deliberately distinct. A pipeline needs to tell *"the contract regressed"* apart from *"the tool was pointed at the wrong path"*, and a single non-zero code would conflate a real finding with a broken step.

The report always goes to stdout, errors always to stderr. `lucyd diff --format json > report.json` gives you a clean file whatever happened.

## Choosing what fails the build

By default, `--fail-on missing,changed`. A lost or altered endpoint breaks existing callers; a newly added one usually does not.

```bash
lucyd diff --fail-on missing            # only a lost endpoint fails
lucyd diff --fail-on missing,added,changed   # any difference at all fails
```

**`--fail-on` replaces the default, it does not extend it.** `--fail-on added` fails on additions *only*, and stops failing on missing and changed endpoints. This is deliberate: a flag that silently kept categories you did not name would make the strictest setting impossible to express.

Every difference always appears in the report. `--fail-on` decides the exit code, never what gets printed.

## In CI

The candidate document has to come from somewhere. Two approaches, both fine.

### Run the application and export from it

Closest to reality: it validates what your app actually serves.

```yaml
name: API contract

on: [push, pull_request]

jobs:
  contract:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable

      - name: Install the lucyd CLI
        run: cargo install lucyd-cli

      - name: Start the application
        run: |
          cargo run --release &
          # Wait for /docs to answer rather than sleeping a fixed amount.
          timeout 60 sh -c 'until curl -sf http://localhost:3000/docs/spec.json > /dev/null; do sleep 1; done'

      - name: Export the current contract
        run: curl -sf -o lucyd-openapi.json http://localhost:3000/docs/openapi.json

      - name: Compare against the committed baseline
        run: lucyd diff --format json
```

### Generate the document from a test

No server, no port, no waiting. `generate_openapi_document` is a plain library function, re-exported from `lucyd` so this needs no extra dependency.

```rust
// tests/export_openapi.rs
use lucyd::{generate_openapi_document, global_registry};

/// Writes the document this build would serve, so CI can diff it.
///
/// Not really a test: it is the export step, run by the test harness because
/// that is the cheapest way to reach the registry the macros populated.
#[test]
fn export_the_openapi_document() {
    let registry = global_registry().lock().expect("registry lock");
    let document = generate_openapi_document(&registry);

    std::fs::write(
        "lucyd-openapi.json",
        serde_json::to_string_pretty(&document).expect("a document always serialises"),
    )
    .expect("the export must be writable");
}
```

```yaml
      - name: Export the current contract
        run: cargo test --test export_openapi

      - name: Compare against the committed baseline
        run: lucyd diff
```

`serde_json` needs to be a dev-dependency of your crate for the snippet above, since it writes the document out itself.

This only sees endpoints whose module is linked into the test binary. Endpoints registered by a module the test never references will be reported as `missing`, which is a real trap: `inventory` collects what the linker kept.

### Making the check meaningful

Commit the baseline `openapi.json`. That is the whole point: a file in the repository that a reviewer can see change, so widening the contract becomes a deliberate act rather than a side effect.

## Where the code lives

For contributors, the split between the two crates is the thing to understand:

```
lucyd-core                             lucyd-cli
  openapi/diff/                          cli.rs         flags -> library types
    mod.rs      diff_documents()         discovery.rs   finding files on disk
    index.rs    pairing endpoints        source.rs      JSON and YAML loading
    operation.rs  default vs strict      diff.rs        sequence + printing
    parameters.rs parameter comparison   update.rs      --update
    schema.rs   the schema walk          main.rs        exit codes
    resolver.rs $ref resolution
    report.rs   both renderings
```

**`lucyd-core` owns every decision.** What counts as a difference, what the default profile ignores, how a report reads in either format, and whether a report is a failure. All of it is behind the `openapi-diff` feature, off by default, so an application that only serves `/docs` never compiles it.

**`lucyd-cli` owns only what a library should not.** Finding documents on disk, accepting YAML as well as JSON, and turning a report into a process exit code.

The line between them is not decoration. It is what makes a report produced by a test identical to one produced by the binary, and it is why the CLI has no formatting code of its own: `render` calls `report.to_string()` and there is a test asserting exactly that.

Useful entry points if you are changing behaviour:

| You want to change | Start at |
|---|---|
| What counts as a difference on an operation | `openapi/diff/operation.rs` |
| What the default profile ignores | the `options.strict` branches, same file |
| How schemas are walked | `openapi/diff/schema.rs` |
| How a report is printed | `openapi/diff/report.rs`, both `Display` and `to_json` |
| Which files a bare `lucyd diff` finds | `lucyd-cli/src/discovery.rs`, the `Convention` constants |
| A new flag | `lucyd-cli/src/cli.rs`, then map it onto a library type |

Adding a facet to the comparison means adding it to `operation.rs` (or `parameters.rs`), deciding whether it belongs on the default profile, and giving it a `ChangeKind` if it is a new category. The `Change` type carries a `location` and a `detail` so a new comparator produces report lines that read like the existing ones without touching the renderer.

## Limitations

See [§11, Known limitations](11-limitations.md) for the diff's own scoping gaps: no HTTP sources, shallow `oneOf` / `allOf` / `anyOf` comparison, `security` and `servers` out of scope.

---

Previous: [12. Importing an OpenAPI document](12-import-openapi.md) · [Back to index](README.md)
