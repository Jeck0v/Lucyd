//! End-to-end checks of the `lucyd` binary.
//!
//! These run the real executable in a real directory. That is the only way to
//! cover what unit tests structurally cannot: the exit code a CI job branches
//! on, and the discovery of documents from a working directory.

use serde_json::Value;
use std::{fs, path::Path, process::Command};
use tempfile::TempDir;

/// The two documents describe the same API.
const EXIT_CLEAN: i32 = 0;

/// A difference in a category `--fail-on` selected.
const EXIT_DIFFERENCES: i32 = 1;

/// The run could not happen at all.
const EXIT_ERROR: i32 = 2;

/// A specification written by hand, with two endpoints and a `204`.
const BASELINE: &str = r#"{
  "openapi": "3.0.3",
  "paths": {
    "/api/users": {
      "get": { "summary": "List users", "responses": { "200": { "description": "The users" } } }
    },
    "/api/users/{id}": {
      "delete": { "responses": { "204": { "description": "Deleted" } } }
    }
  }
}"#;

/// What Lucyd exports once both endpoints have been migrated.
const FAITHFUL_EXPORT: &str = r#"{
  "openapi": "3.1.0",
  "paths": {
    "/api/users": { "get": { "responses": { "200": { "description": "Successful response" } } } },
    "/api/users/{id}": { "delete": { "responses": { "200": { "description": "Successful response" } } } }
  }
}"#;

/// The same export with one endpoint never migrated.
const REGRESSED_EXPORT: &str = r#"{
  "openapi": "3.1.0",
  "paths": {
    "/api/users": { "get": { "responses": { "200": { "description": "Successful response" } } } }
  }
}"#;

/// What one invocation of the binary produced.
struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

/// Runs `lucyd` inside `directory` and captures everything it produced.
fn lucyd(directory: &Path, arguments: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_lucyd"))
        .args(arguments)
        .current_dir(directory)
        .output()
        .expect("the binary under test must be runnable");

    Run {
        code: output
            .status
            .code()
            .expect("the process must exit normally"),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// Creates a project directory holding the given `(file name, content)` pairs.
///
/// The `Cargo.toml` marks the project root, so discovery stops there instead
/// of walking out into whatever the temporary directory happens to live under.
fn project(files: &[(&str, &str)]) -> TempDir {
    let root = TempDir::new().expect("a temporary directory must be available");
    fs::write(root.path().join("Cargo.toml"), "").expect("the marker must be writable");
    for (name, content) in files {
        fs::write(root.path().join(name), content).expect("the document must be writable");
    }
    root
}

/// A project with a baseline and an export, both under their conventional
/// names.
fn migrated_project(export: &str) -> TempDir {
    project(&[("openapi.json", BASELINE), ("lucyd-openapi.json", export)])
}

#[test]
fn a_bare_diff_finds_both_documents_and_reports_a_faithful_migration() {
    let root = migrated_project(FAITHFUL_EXPORT);

    let run = lucyd(root.path(), &["diff"]);

    assert_eq!(run.code, EXIT_CLEAN, "stderr was: {}", run.stderr);
    assert!(
        run.stdout.contains("No differences found."),
        "stdout was: {}",
        run.stdout
    );
}

#[test]
fn a_bare_diff_works_from_a_subdirectory_of_the_project() {
    let root = migrated_project(FAITHFUL_EXPORT);
    let nested = root.path().join("crates/api/src");
    fs::create_dir_all(&nested).expect("the subdirectory must be creatable");

    let run = lucyd(&nested, &["diff"]);

    assert_eq!(run.code, EXIT_CLEAN, "stderr was: {}", run.stderr);
}

#[test]
fn a_lost_endpoint_fails_the_run_and_is_named() {
    let root = migrated_project(REGRESSED_EXPORT);

    let run = lucyd(root.path(), &["diff"]);

    assert_eq!(run.code, EXIT_DIFFERENCES);
    assert!(
        run.stdout.contains("DELETE /api/users/{id}"),
        "the report must name the endpoint that was lost, got: {}",
        run.stdout
    );
}

#[test]
fn fail_on_decides_the_exit_code_without_hiding_the_report() {
    let root = migrated_project(REGRESSED_EXPORT);

    let run = lucyd(root.path(), &["diff", "--fail-on", "added"]);

    assert_eq!(
        run.code, EXIT_CLEAN,
        "a missing endpoint must not fail a run that only watches additions"
    );
    assert!(
        run.stdout.contains("DELETE /api/users/{id}"),
        "the difference must still be reported, got: {}",
        run.stdout
    );
}

#[test]
fn the_json_format_is_parseable_and_carries_the_summary() {
    let root = migrated_project(REGRESSED_EXPORT);

    let run = lucyd(root.path(), &["diff", "--format", "json"]);

    let report: Value = serde_json::from_str(&run.stdout).expect("stdout must be valid JSON alone");
    assert_eq!(report["summary"]["missing"], 1);
    assert_eq!(report["missing"][0]["method"], "DELETE");
    assert_eq!(run.code, EXIT_DIFFERENCES);
}

#[test]
fn explicit_paths_override_discovery() {
    let root = project(&[("spec.json", BASELINE), ("export.json", FAITHFUL_EXPORT)]);

    let run = lucyd(
        root.path(),
        &["diff", "--from", "spec.json", "--to", "export.json"],
    );

    assert_eq!(
        run.code, EXIT_CLEAN,
        "unconventional names must still work, stderr was: {}",
        run.stderr
    );
}

#[test]
fn strict_mode_reports_what_the_default_profile_forgives() {
    let root = migrated_project(FAITHFUL_EXPORT);

    let run = lucyd(root.path(), &["diff", "--strict"]);

    assert_eq!(run.code, EXIT_DIFFERENCES);
    assert!(
        run.stdout.contains("204"),
        "the status code Lucyd cannot express must surface under --strict, got: {}",
        run.stdout
    );
}

#[test]
fn a_document_that_cannot_be_found_is_an_error_not_a_difference() {
    let root = project(&[("openapi.json", BASELINE)]);

    let run = lucyd(root.path(), &["diff"]);

    assert_eq!(
        run.code, EXIT_ERROR,
        "a setup problem must not look like a regression"
    );
    assert!(run.stderr.contains("Lucyd export"), "{}", run.stderr);
    assert!(run.stderr.contains("curl -o"), "{}", run.stderr);
}

#[test]
fn a_url_is_refused_with_the_command_that_replaces_it() {
    let root = migrated_project(FAITHFUL_EXPORT);

    let run = lucyd(
        root.path(),
        &["diff", "--to", "http://localhost:3000/docs/openapi.json"],
    );

    assert_eq!(run.code, EXIT_ERROR);
    assert!(run.stderr.contains("not supported yet"), "{}", run.stderr);
}

#[test]
fn the_binary_reports_its_own_version() {
    let root = project(&[]);

    let run = lucyd(root.path(), &["--version"]);

    assert_eq!(run.code, EXIT_CLEAN);
    assert!(
        run.stdout.contains(env!("CARGO_PKG_VERSION")),
        "stdout was: {}",
        run.stdout
    );
}

#[test]
fn a_bare_invocation_shows_the_help_screen() {
    let root = project(&[]);

    let run = lucyd(root.path(), &[]);

    assert_eq!(run.code, EXIT_ERROR);
    assert!(
        run.stderr.contains("diff") && run.stderr.contains("--update"),
        "the help must list what the binary can do, got: {}",
        run.stderr
    );
}
