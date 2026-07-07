use super::refs::{rewrite_definition_refs, rewrite_renamed_refs};
use serde_json::{Map, Value};
use std::collections::HashMap;

/// A pending or already-decided `(global_name, value, needs_insert)` entry
/// produced while resolving name collisions in [`ComponentSchemas`].
type NameDecision = (String, Value, bool);

/// Result of comparing a candidate name against the shared schema map.
enum Slot {
    /// No schema is stored under the candidate name.
    Empty,
    /// A byte-identical schema is already stored under the candidate name.
    Equal,
    /// A different schema is already stored under the candidate name.
    Different,
}

/// Shared accumulator for schemas hoisted into `components.schemas`.
///
/// schemars emits self-contained JSON Schema documents (draft-07 style) with
/// nested types under `definitions`. This helper flattens each root schema and
/// its definitions into a single, document-wide, de-duplicated map, rewriting
/// internal `$ref`s to point at `#/components/schemas/...`.
#[derive(Default)]
pub(super) struct ComponentSchemas {
    schemas: Map<String, Value>,
}

impl ComponentSchemas {
    /// Returns `true` if no schema has been hoisted yet.
    pub(super) fn is_empty(&self) -> bool {
        self.schemas.is_empty()
    }

    /// Consumes the accumulator, yielding the raw `schemas` map.
    pub(super) fn into_map(self) -> Map<String, Value> {
        self.schemas
    }

    /// Hoists a raw schemars schema (root + its definitions) into the shared
    /// map and returns the final, document-wide name of the root schema.
    ///
    /// `fallback_name` is used as the root name when the schema has no
    /// `title` (mirroring the `{endpoint}_request` / `{endpoint}_response`
    /// convention used elsewhere).
    pub(super) fn hoist_root_schema(&mut self, raw: &Value, fallback_name: &str) -> String {
        let mut root = raw.clone();
        let mut extracted = extract_definitions(&mut root);
        let root_name = root_schema_name(&root, fallback_name);
        rewrite_all_definition_refs(&mut root, &mut extracted);

        // Root is placed first so its name has priority over its definitions.
        let mut items: Vec<(String, Value)> = Vec::with_capacity(1 + extracted.len());
        items.push((root_name, root));
        items.extend(extracted);

        let (renames, decisions) = self.resolve_global_names(items);
        let root_global = decisions[0].0.clone();
        self.apply_renames_and_insert(decisions, &renames);

        root_global
    }

    /// Applies any local-to-global rename to internal `$ref`s in each
    /// decision, then inserts the ones that still need to be stored.
    fn apply_renames_and_insert(
        &mut self,
        decisions: Vec<NameDecision>,
        renames: &HashMap<String, String>,
    ) {
        let any_renamed = renames.iter().any(|(local, global)| local != global);
        for (global_name, mut value, needs_insert) in decisions {
            if any_renamed {
                rewrite_renamed_refs(&mut value, renames);
            }
            if needs_insert {
                self.schemas.insert(global_name, value);
            }
        }
    }

    /// Decides the global (document-wide) name for each local `(name, value)`
    /// pair, de-duplicating against both the shared map and names already
    /// decided earlier in this same call. Returns the local-to-global rename
    /// map alongside the ordered decisions (global name, value, whether it
    /// still needs inserting into the shared map).
    fn resolve_global_names(
        &self,
        items: Vec<(String, Value)>,
    ) -> (HashMap<String, String>, Vec<NameDecision>) {
        let mut renames: HashMap<String, String> = HashMap::new();
        let mut decisions: Vec<NameDecision> = Vec::with_capacity(items.len());

        for (local_name, value) in items {
            let (global_name, needs_insert) =
                self.resolve_global_name(&local_name, &value, &decisions);
            renames.insert(local_name, global_name.clone());
            decisions.push((global_name, value, needs_insert));
        }

        (renames, decisions)
    }

    /// Finds the global name to store `value` under, de-duplicating against
    /// both the shared map and names already decided in this hoist call.
    ///
    /// Identical content reuses the existing name; differing content is stored
    /// under the first free `name_2`, `name_3`, ... candidate.
    fn resolve_global_name(
        &self,
        local: &str,
        value: &Value,
        pending: &[NameDecision],
    ) -> (String, bool) {
        let mut suffix = 1u32;
        loop {
            let candidate = if suffix == 1 {
                local.to_string()
            } else {
                format!("{local}_{suffix}")
            };
            match self.slot_for(&candidate, value, pending) {
                Slot::Empty => return (candidate, true),
                Slot::Equal => return (candidate, false),
                Slot::Different => suffix += 1,
            }
        }
    }

    /// Classifies a candidate name against the shared map and pending inserts.
    fn slot_for(&self, candidate: &str, value: &Value, pending: &[NameDecision]) -> Slot {
        if let Some(existing) = self.schemas.get(candidate) {
            return if existing == value {
                Slot::Equal
            } else {
                Slot::Different
            };
        }
        for (name, pending_value, needs_insert) in pending {
            if *needs_insert && name == candidate {
                return if pending_value == value {
                    Slot::Equal
                } else {
                    Slot::Different
                };
            }
        }
        Slot::Empty
    }
}

/// Extracts and dialect-strips schemars' `definitions`/`$defs` block from a
/// root schema, returning the extracted definitions keyed by their local
/// (pre-hoist) name. `definitions` is schemars 0.8's key; `$defs` is checked
/// defensively in case of a future schemars upgrade.
fn extract_definitions(root: &mut Value) -> Map<String, Value> {
    let mut extracted = Map::new();
    if let Some(object) = root.as_object_mut() {
        for key in ["definitions", "$defs"] {
            if let Some(Value::Object(defs)) = object.remove(key) {
                extracted.extend(defs);
            }
        }
        object.remove("$schema");
    }
    for definition in extracted.values_mut() {
        if let Some(object) = definition.as_object_mut() {
            object.remove("$schema");
        }
    }
    extracted
}

/// Computes the local name a root schema will be hoisted under: its `title`,
/// or the caller-supplied fallback when no title is present.
fn root_schema_name(root: &Value, fallback_name: &str) -> String {
    root.get("title")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| fallback_name.to_string())
}

/// Rewrites `#/definitions/...` / `#/$defs/...` refs (still using local
/// names) to the `#/components/schemas/...` namespace, across the root
/// schema and each of its extracted definitions.
fn rewrite_all_definition_refs(root: &mut Value, extracted: &mut Map<String, Value>) {
    rewrite_definition_refs(root);
    for definition in extracted.values_mut() {
        rewrite_definition_refs(definition);
    }
}

#[cfg(test)]
mod tests {
    use super::super::generate_openapi_document;
    use super::super::test_support::*;
    use crate::registry::EndpointRegistry;

    #[test]
    fn identical_schemas_are_deduplicated() {
        let mut registry = EndpointRegistry::new();
        let mut first = http_endpoint("first", "POST", "/first");
        first.request_schema = Some(simple_schema("Payload"));
        let mut second = http_endpoint("second", "POST", "/second");
        second.request_schema = Some(simple_schema("Payload"));
        registry.register(first);
        registry.register(second);

        let doc = generate_openapi_document(&registry);
        let schemas = doc["components"]["schemas"]
            .as_object()
            .expect("components.schemas must be an object");

        assert_eq!(
            schemas.len(),
            1,
            "identical schemas must collapse to a single components entry"
        );
        let first_ref = &doc["paths"]["/first"]["post"]["requestBody"]["content"]["application/json"]
            ["schema"]["$ref"];
        let second_ref = &doc["paths"]["/second"]["post"]["requestBody"]["content"]["application/json"]
            ["schema"]["$ref"];
        assert_eq!(first_ref, "#/components/schemas/Payload");
        assert_eq!(
            first_ref, second_ref,
            "both operations must reference the same de-duplicated schema"
        );
    }

    #[test]
    fn same_title_different_content_is_suffixed() {
        let mut registry = EndpointRegistry::new();
        let mut first = http_endpoint("first", "POST", "/first");
        first.request_schema = Some(simple_schema("Payload"));
        let mut second = http_endpoint("second", "POST", "/second");
        second.request_schema = Some(simple_schema_variant("Payload"));
        registry.register(first);
        registry.register(second);

        let doc = generate_openapi_document(&registry);
        let schemas = doc["components"]["schemas"]
            .as_object()
            .expect("components.schemas must be an object");

        assert_eq!(schemas.len(), 2, "conflicting schemas must both be kept");
        assert!(schemas.contains_key("Payload"));
        assert!(schemas.contains_key("Payload_2"));

        assert_eq!(
            doc["paths"]["/first"]["post"]["requestBody"]["content"]["application/json"]["schema"]
                ["$ref"],
            "#/components/schemas/Payload"
        );
        assert_eq!(
            doc["paths"]["/second"]["post"]["requestBody"]["content"]["application/json"]["schema"]
                ["$ref"],
            "#/components/schemas/Payload_2"
        );
    }
}
