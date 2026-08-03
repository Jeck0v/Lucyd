//! Rewrites JSON Schema `$ref` strings between the schemars dialect and the
//! OpenAPI `components.schemas` namespace.

use serde_json::Value;
use std::collections::HashMap;

/// Rewrites `$ref` strings pointing at `#/definitions/...` or `#/$defs/...`
/// so they target `#/components/schemas/...`, preserving the trailing name.
pub(super) fn rewrite_definition_refs(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, entry) in map.iter_mut() {
                if key != "$ref" {
                    rewrite_definition_refs(entry);
                    continue;
                }
                if let Value::String(reference) = entry
                    && let Some(rest) = reference
                        .strip_prefix("#/definitions/")
                        .or_else(|| reference.strip_prefix("#/$defs/"))
                {
                    *reference = format!("#/components/schemas/{rest}");
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(rewrite_definition_refs),
        _ => {}
    }
}

/// Rewrites `#/components/schemas/{local}` refs to their renamed global name
/// for every local that was suffixed during the naming decision.
pub(super) fn rewrite_renamed_refs(value: &mut Value, renames: &HashMap<String, String>) {
    match value {
        Value::Object(map) => {
            for (key, entry) in map.iter_mut() {
                if key != "$ref" {
                    rewrite_renamed_refs(entry, renames);
                    continue;
                }
                if let Value::String(reference) = entry
                    && let Some(global) = reference
                        .strip_prefix("#/components/schemas/")
                        .and_then(|name| renames.get(name).filter(|g| g.as_str() != name))
                {
                    *reference = format!("#/components/schemas/{global}");
                }
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| rewrite_renamed_refs(item, renames)),
        _ => {}
    }
}
