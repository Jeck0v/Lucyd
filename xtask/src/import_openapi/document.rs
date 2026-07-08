//! Loads and validates the OpenAPI document given to `cargo xtask import-openapi`.
//!
//! Accepts both JSON and YAML input without asking the caller to specify
//! which: JSON is tried first (the common case, and unambiguous — valid JSON
//! is never mistaken for something else), then YAML. `serde_yaml_ng`
//! deserializes straight into [`serde_json::Value`] (which implements
//! `Deserialize` generically), so no separate YAML value type needs to flow
//! through the rest of the pipeline — every downstream module only ever
//! looks at a `serde_json::Value`.

use serde_json::Value;
use std::{fs, path::Path};

/// Reads `path` from disk and parses it into a validated OpenAPI document.
pub fn load_document(path: &Path) -> Result<Value, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("failed to read '{}': {e}", path.display()))?;
    let document = parse_document(&content, path)?;
    validate_openapi_document(&document)?;
    Ok(document)
}

/// Parses `content` as JSON, falling back to YAML on failure.
fn parse_document(content: &str, path: &Path) -> Result<Value, String> {
    if let Ok(value) = serde_json::from_str::<Value>(content) {
        return Ok(value);
    }
    // Not valid JSON: try YAML. Its error message is generally more useful
    // for a document that actually is YAML (line/column oriented), so it's
    // the one surfaced when both attempts fail.
    serde_yaml_ng::from_str::<Value>(content).map_err(|yaml_err| {
        format!(
            "failed to parse '{}' as JSON or YAML: {yaml_err}",
            path.display()
        )
    })
}

/// Validates that `document` has an `openapi` field whose value starts with
/// `"3."` — this importer only understands OpenAPI 3.x documents.
fn validate_openapi_document(document: &Value) -> Result<(), String> {
    let Some(version) = document.get("openapi") else {
        return Err("not a valid OpenAPI document: missing 'openapi' field".to_string());
    };
    let Some(version) = version.as_str() else {
        return Err("not a valid OpenAPI document: 'openapi' field must be a string".to_string());
    };
    if !version.starts_with("3.") {
        return Err(format!(
            "unsupported OpenAPI version '{version}': only 3.x documents are supported"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_JSON_DOC: &str =
        r#"{"openapi": "3.1.0", "info": {"title": "T", "version": "1"}, "paths": {}}"#;
    const VALID_YAML_DOC: &str =
        "openapi: \"3.0.3\"\ninfo:\n  title: T\n  version: \"1\"\npaths: {}\n";
    const MISSING_OPENAPI_FIELD_DOC: &str = r#"{"info": {"title": "T"}}"#;
    const NEITHER_JSON_NOR_YAML: &str = "{not: json, [nor yaml";

    #[test]
    fn parses_json_document() {
        let path = Path::new("openapi.json");
        let value = parse_document(VALID_JSON_DOC, path).expect("valid JSON must parse");
        assert_eq!(value["openapi"], "3.1.0");
    }

    #[test]
    fn parses_yaml_document() {
        let path = Path::new("openapi.yaml");
        let value = parse_document(VALID_YAML_DOC, path).expect("valid YAML must parse");
        assert_eq!(value["openapi"], "3.0.3");
    }

    #[test]
    fn rejects_content_that_is_neither_json_nor_yaml() {
        let path = Path::new("openapi.yaml");
        let err = parse_document(NEITHER_JSON_NOR_YAML, path)
            .expect_err("malformed content must be rejected");
        assert!(err.contains("failed to parse 'openapi.yaml' as JSON or YAML"));
    }

    #[test]
    fn rejects_document_missing_openapi_field() {
        let value: Value = serde_json::from_str(MISSING_OPENAPI_FIELD_DOC).unwrap();
        let err =
            validate_openapi_document(&value).expect_err("missing 'openapi' field must error");
        assert_eq!(err, "not a valid OpenAPI document: missing 'openapi' field");
    }

    #[test]
    fn rejects_openapi_2_x_documents() {
        let value = serde_json::json!({ "openapi": "2.0" });
        let err = validate_openapi_document(&value).expect_err("2.x must be rejected");
        assert!(err.contains("unsupported OpenAPI version '2.0'"));
    }

    #[test]
    fn accepts_openapi_3_x_documents() {
        let value = serde_json::json!({ "openapi": "3.1.0" });
        validate_openapi_document(&value).expect("3.x must be accepted");
    }
}
