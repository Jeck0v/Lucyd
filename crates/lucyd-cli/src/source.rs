//! Turns a path into the OpenAPI document it holds.
//!
//! Accepts JSON and YAML without asking which: JSON is tried first (the
//! common case, and unambiguous, since valid JSON is never mistaken for
//! something else), then YAML. `serde_yaml_ng` deserializes straight into
//! [`serde_json::Value`], so the diff only ever sees one representation.

use serde_json::Value;
use std::{fs, path::Path};

/// URL prefixes recognised only so they can be refused with a useful message.
const URL_SCHEMES: &[&str] = &["http://", "https://"];

/// The OpenAPI major version this tool can compare.
const SUPPORTED_VERSION: &str = "3.";

/// Reads and parses the OpenAPI document at `path`.
pub fn load_document(path: &Path) -> Result<Value, String> {
    reject_url(path)?;
    let content = fs::read_to_string(path)
        .map_err(|error| format!("failed to read '{}': {error}", path.display()))?;
    let document = parse(&content, path)?;
    validate(&document, path)?;
    Ok(document)
}

/// Refuses a URL, naming the command that turns it into a supported input.
///
/// Worth its own step: the proposed design for this command used a URL, so
/// people will try one. Left to the filesystem it would fail as a missing
/// file, which says nothing about why it cannot work.
fn reject_url(path: &Path) -> Result<(), String> {
    let value = path.to_string_lossy();
    if !URL_SCHEMES.iter().any(|scheme| value.starts_with(scheme)) {
        return Ok(());
    }
    Err(format!(
        "'{value}' is a URL, and remote documents are not supported yet.\n  \
         Save it first, then compare the file:\n    \
         curl -o lucyd-openapi.json {value}"
    ))
}

/// Parses `content` as JSON, falling back to YAML.
fn parse(content: &str, path: &Path) -> Result<Value, String> {
    if let Ok(document) = serde_json::from_str::<Value>(content) {
        return Ok(document);
    }
    // Not JSON, so it is YAML or nothing. The YAML error is the one surfaced
    // because it is line and column oriented, which is more useful for a
    // document that really was meant to be YAML.
    serde_yaml_ng::from_str::<Value>(content).map_err(|error| {
        format!(
            "failed to parse '{}' as JSON or YAML: {error}",
            path.display()
        )
    })
}

/// Checks that the file really is an OpenAPI 3.x document.
///
/// Diffing an unrelated JSON file would otherwise succeed and report every
/// endpoint as missing, which reads as a catastrophic regression rather than
/// as the wrong path being passed.
fn validate(document: &Value, path: &Path) -> Result<(), String> {
    let file = path.display();
    match document.get("openapi").and_then(Value::as_str) {
        Some(version) if version.starts_with(SUPPORTED_VERSION) => Ok(()),
        Some(version) => Err(format!(
            "'{file}' declares OpenAPI {version}; only 3.x documents can be compared"
        )),
        None => Err(format!(
            "'{file}' is not an OpenAPI document: no 'openapi' field"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Writes `content` to `name` in a fresh directory and loads it.
    ///
    /// The directory is dropped on return, which is fine: the document is
    /// already fully in memory by then.
    fn load(name: &str, content: &str) -> Result<Value, String> {
        let directory = TempDir::new().expect("a temporary directory must be available");
        let path = directory.path().join(name);
        fs::write(&path, content).expect("the document must be writable");
        load_document(&path)
    }

    #[test]
    fn a_json_document_is_loaded() {
        let document = load("openapi.json", r#"{"openapi":"3.1.0","paths":{}}"#)
            .expect("valid JSON must load");

        assert_eq!(document["openapi"], "3.1.0");
    }

    #[test]
    fn a_yaml_document_is_loaded_without_being_announced() {
        let document =
            load("openapi.yaml", "openapi: \"3.0.3\"\npaths: {}\n").expect("valid YAML must load");

        assert_eq!(
            document["openapi"], "3.0.3",
            "the format must be detected, not declared by the caller"
        );
    }

    #[test]
    fn the_extension_does_not_decide_the_format() {
        let document = load("openapi.yaml", r#"{"openapi":"3.1.0","paths":{}}"#)
            .expect("content wins over the file name");

        assert_eq!(document["openapi"], "3.1.0");
    }

    #[test]
    fn a_missing_file_is_reported_as_unreadable() {
        let error = load_document(Path::new("does-not-exist.json")).expect_err("no such file");

        assert!(error.starts_with("failed to read"), "{error}");
    }

    #[test]
    fn a_url_is_refused_with_the_command_that_replaces_it() {
        let error = load_document(Path::new("http://localhost:3000/docs/openapi.json"))
            .expect_err("remote documents are out of scope for now");

        assert!(error.contains("not supported yet"), "{error}");
        assert!(
            error.contains("curl -o lucyd-openapi.json http://localhost:3000/docs/openapi.json"),
            "the message must be copy-pasteable, got: {error}"
        );
    }

    #[test]
    fn content_that_is_neither_json_nor_yaml_is_rejected() {
        let error = load("openapi.json", "{not: json, [nor yaml").expect_err("malformed input");

        assert!(error.contains("as JSON or YAML"), "{error}");
    }

    #[test]
    fn an_unrelated_json_file_is_not_treated_as_an_empty_api() {
        let error = load("package.json", r#"{"name":"my-app"}"#)
            .expect_err("a JSON file without an openapi field must be refused");

        assert!(error.contains("no 'openapi' field"), "{error}");
    }

    #[test]
    fn a_swagger_2_document_is_refused_rather_than_half_understood() {
        let error =
            load("swagger.json", r#"{"openapi":"2.0","paths":{}}"#).expect_err("2.x is not 3.x");

        assert!(error.contains("only 3.x documents"), "{error}");
    }
}
