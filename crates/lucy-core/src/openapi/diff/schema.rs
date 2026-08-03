//! Compares two JSON Schemas structurally.
//!
//! Only the keywords that change what a caller may send or must handle are
//! compared. Prose (`description`, `title`, `example`, `default`) is skipped
//! outright, in every mode: rewording a description is not an API change, and
//! a diff that fails CI over one would be turned off within a week.

use super::report::{Change, ChangeKind, describe};
use super::resolver::{SchemaResolver, reference_of};
use serde_json::{Map, Value};
use std::collections::{BTreeSet, HashSet};
use std::sync::LazyLock;

/// Schema keywords compared verbatim: any difference is reported as-is.
const COMPARED_KEYWORDS: &[&str] = &["type", "format", "enum"];

/// Borrowed when a schema doesn't declare an object-valued member at all,
/// so callers can iterate unconditionally instead of branching on `Option`.
static NO_MEMBERS: LazyLock<Map<String, Value>> = LazyLock::new(Map::new);

/// Walks two schemas in lockstep, collecting their differences.
///
/// One comparator is built per endpoint pair and used for both the request and
/// the response, so a recursive type shared by the two is only descended once.
pub(super) struct SchemaComparator<'a> {
    baseline: SchemaResolver<'a>,
    candidate: SchemaResolver<'a>,
    /// `$ref` pairs already descended into. Only a reference can make a JSON
    /// document recursive, so recording them is enough to guarantee the walk
    /// terminates on a self-referential schema.
    visited: HashSet<(&'a str, &'a str)>,
    /// The kind stamped on changes found by the walk currently in progress,
    /// set by [`Self::compare_root`].
    kind: ChangeKind,
    changes: Vec<Change>,
}

impl<'a> SchemaComparator<'a> {
    /// Builds a comparator for schemas belonging to these two documents.
    pub(super) fn new(baseline_document: &'a Value, candidate_document: &'a Value) -> Self {
        Self {
            baseline: SchemaResolver::new(baseline_document),
            candidate: SchemaResolver::new(candidate_document),
            visited: HashSet::new(),
            kind: ChangeKind::RequestSchema,
            changes: Vec::new(),
        }
    }

    /// Compares one body's schema, tagging every difference found below it
    /// with `kind` and locating it under `location`.
    ///
    /// A body declared on only one side is a difference in itself, so the
    /// `Option`s are part of the comparison rather than a precondition.
    pub(super) fn compare_root(
        &mut self,
        kind: ChangeKind,
        baseline: Option<&'a Value>,
        candidate: Option<&'a Value>,
        location: &str,
    ) {
        self.kind = kind;
        match (baseline, candidate) {
            (None, None) => {}
            (Some(_), None) => self.record(location, "body removed"),
            (None, Some(_)) => self.record(location, "body added"),
            (Some(baseline), Some(candidate)) => self.compare(baseline, candidate, location),
        }
    }

    /// Consumes the comparator, yielding everything it found.
    pub(super) fn into_changes(self) -> Vec<Change> {
        self.changes
    }

    /// Compares two schemas, descending into their properties and elements.
    fn compare(&mut self, baseline: &'a Value, candidate: &'a Value, location: &str) {
        if !self.enter(baseline, candidate) {
            return;
        }
        let (Some(baseline), Some(candidate)) = (
            self.baseline.resolve(baseline),
            self.candidate.resolve(candidate),
        ) else {
            let detail = "schema reference could not be resolved, contents not compared";
            return self.record_as(ChangeKind::Unresolved, location, detail);
        };

        for keyword in COMPARED_KEYWORDS {
            self.compare_keyword(baseline, candidate, location, keyword);
        }
        self.compare_properties(baseline, candidate, location);
        self.compare_required(baseline, candidate, location);
        self.compare_items(baseline, candidate, location);
    }

    /// Records a `$ref` pair before descending into it, returning `false` when
    /// this exact pair has already been compared.
    fn enter(&mut self, baseline: &'a Value, candidate: &'a Value) -> bool {
        match (reference_of(baseline), reference_of(candidate)) {
            (Some(baseline), Some(candidate)) => self.visited.insert((baseline, candidate)),
            _ => true,
        }
    }

    /// Reports a keyword whose value differs between the two schemas.
    fn compare_keyword(
        &mut self,
        baseline: &Value,
        candidate: &Value,
        location: &str,
        keyword: &str,
    ) {
        let (before, after) = (baseline.get(keyword), candidate.get(keyword));
        if before == after {
            return;
        }
        let detail = format!(
            "{keyword} changed: {} -> {}",
            describe(before),
            describe(after)
        );
        self.record(location, detail);
    }

    /// Reports fields gained or lost, and descends into the ones both declare.
    fn compare_properties(&mut self, baseline: &'a Value, candidate: &'a Value, location: &str) {
        let before = object_member(baseline, "properties");
        let after = object_member(candidate, "properties");

        for (field, schema) in before {
            match after.get(field) {
                Some(counterpart) => {
                    self.compare(schema, counterpart, &format!("{location}.{field}"))
                }
                None => self.record(location, format!("field \"{field}\" removed")),
            }
        }
        for field in after.keys().filter(|field| !before.contains_key(*field)) {
            self.record(location, format!("field \"{field}\" added"));
        }
    }

    /// Reports fields whose optionality flipped in either direction: a newly
    /// required field breaks existing callers, a newly optional one breaks
    /// callers that relied on it always being present.
    fn compare_required(&mut self, baseline: &Value, candidate: &Value, location: &str) {
        let before = required_fields(baseline);
        let after = required_fields(candidate);

        for field in before.difference(&after) {
            self.record(location, format!("field \"{field}\" is no longer required"));
        }
        for field in after.difference(&before) {
            self.record(location, format!("field \"{field}\" is now required"));
        }
    }

    /// Descends into an array schema's element type.
    fn compare_items(&mut self, baseline: &'a Value, candidate: &'a Value, location: &str) {
        match (baseline.get("items"), candidate.get("items")) {
            (Some(before), Some(after)) => self.compare(before, after, &format!("{location}[]")),
            (Some(_), None) => self.record(location, "array element type removed"),
            (None, Some(_)) => self.record(location, "array element type added"),
            (None, None) => {}
        }
    }

    /// Records a change under the kind of the walk in progress.
    fn record(&mut self, location: &str, detail: impl Into<String>) {
        self.record_as(self.kind, location, detail);
    }

    /// Records a change under an explicit kind.
    fn record_as(&mut self, kind: ChangeKind, location: &str, detail: impl Into<String>) {
        self.changes.push(Change::new(kind, location, detail));
    }
}

/// Borrows an object-valued member of a schema, or an empty map when the
/// schema doesn't declare it (or declares it with the wrong type).
fn object_member<'a>(schema: &'a Value, key: &str) -> &'a Map<String, Value> {
    schema
        .get(key)
        .and_then(Value::as_object)
        .unwrap_or(&NO_MEMBERS)
}

/// Collects a schema's `required` field names, ignoring non-string entries.
fn required_fields(schema: &Value) -> BTreeSet<&str> {
    schema
        .get("required")
        .and_then(Value::as_array)
        .map(|fields| fields.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Compares two standalone schemas, returning `location: detail` lines.
    fn compare(baseline: Value, candidate: Value) -> Vec<String> {
        compare_within(json!({}), json!({}), baseline, candidate)
    }

    /// Compares two schemas that resolve their `$ref`s against a document each.
    fn compare_within(
        baseline_document: Value,
        candidate_document: Value,
        baseline: Value,
        candidate: Value,
    ) -> Vec<String> {
        let mut comparator = SchemaComparator::new(&baseline_document, &candidate_document);
        comparator.compare_root(
            ChangeKind::ResponseSchema,
            Some(&baseline),
            Some(&candidate),
            "response",
        );
        comparator
            .into_changes()
            .iter()
            .map(|change| format!("{}: {}", change.location, change.detail))
            .collect()
    }

    #[test]
    fn identical_schemas_produce_nothing() {
        let schema = json!({
            "type": "object",
            "properties": { "id": { "type": "integer" } },
            "required": ["id"]
        });

        assert!(compare(schema.clone(), schema).is_empty());
    }

    #[test]
    fn prose_only_edits_are_not_api_changes() {
        let before = json!({ "type": "object", "description": "A user", "title": "User" });
        let after = json!({ "type": "object", "description": "A person", "example": { "id": 1 } });

        assert!(
            compare(before, after).is_empty(),
            "rewording documentation must never fail a diff"
        );
    }

    #[test]
    fn a_removed_field_is_reported() {
        let before = json!({ "properties": { "id": {}, "createdAt": {} } });
        let after = json!({ "properties": { "id": {} } });

        assert_eq!(
            compare(before, after),
            ["response: field \"createdAt\" removed"]
        );
    }

    #[test]
    fn an_added_field_is_reported() {
        let before = json!({ "properties": { "id": {} } });
        let after = json!({ "properties": { "id": {}, "createdAt": {} } });

        assert_eq!(
            compare(before, after),
            ["response: field \"createdAt\" added"]
        );
    }

    #[test]
    fn a_nested_field_change_is_located_by_its_dotted_path() {
        let before = json!({
            "properties": { "address": { "properties": { "city": { "type": "string" } } } }
        });
        let after = json!({
            "properties": { "address": { "properties": { "city": { "type": "integer" } } } }
        });

        assert_eq!(
            compare(before, after),
            ["response.address.city: type changed: \"string\" -> \"integer\""]
        );
    }

    #[test]
    fn an_array_element_change_is_located_with_brackets() {
        let before = json!({ "type": "array", "items": { "type": "string" } });
        let after = json!({ "type": "array", "items": { "type": "integer" } });

        assert_eq!(
            compare(before, after),
            ["response[]: type changed: \"string\" -> \"integer\""]
        );
    }

    #[test]
    fn optionality_flips_are_reported_in_both_directions() {
        let before = json!({ "required": ["id", "name"] });
        let after = json!({ "required": ["name", "email"] });

        assert_eq!(
            compare(before, after),
            [
                "response: field \"id\" is no longer required",
                "response: field \"email\" is now required"
            ]
        );
    }

    #[test]
    fn enum_and_format_changes_are_reported() {
        let before = json!({ "type": "string", "format": "date-time", "enum": ["a"] });
        let after = json!({ "type": "string", "format": "date", "enum": ["a", "b"] });

        assert_eq!(
            compare(before, after),
            [
                "response: format changed: \"date-time\" -> \"date\"",
                "response: enum changed: [\"a\"] -> [\"a\",\"b\"]"
            ]
        );
    }

    #[test]
    fn refs_are_compared_by_target_not_by_component_name() {
        let user = json!({ "type": "object", "properties": { "id": { "type": "integer" } } });
        let baseline_document = json!({ "components": { "schemas": { "User": user } } });
        let candidate_document = json!({
            "components": { "schemas": { "get_user_response": {
                "type": "object", "properties": { "id": { "type": "integer" } }
            } } }
        });

        let changes = compare_within(
            baseline_document,
            candidate_document,
            json!({ "$ref": "#/components/schemas/User" }),
            json!({ "$ref": "#/components/schemas/get_user_response" }),
        );

        assert!(
            changes.is_empty(),
            "components renamed by Lucyd's exporter must not read as a contract change: {changes:?}"
        );
    }

    #[test]
    fn a_recursive_schema_terminates() {
        let node = json!({
            "type": "object",
            "properties": { "child": { "$ref": "#/components/schemas/Node" } }
        });
        let document = json!({ "components": { "schemas": { "Node": node } } });
        let reference = json!({ "$ref": "#/components/schemas/Node" });

        let changes = compare_within(document.clone(), document, reference.clone(), reference);

        assert!(
            changes.is_empty(),
            "a self-referential schema must compare clean"
        );
    }

    #[test]
    fn an_unresolvable_ref_is_reported_rather_than_assumed_equal() {
        let changes = compare_within(
            json!({}),
            json!({}),
            json!({ "$ref": "#/components/schemas/Gone" }),
            json!({ "type": "object" }),
        );

        assert_eq!(changes.len(), 1);
        assert!(changes[0].contains("could not be resolved"));
    }

    #[test]
    fn a_body_present_on_one_side_only_is_a_difference() {
        let document = json!({});
        let schema = json!({ "type": "object" });

        let mut comparator = SchemaComparator::new(&document, &document);
        comparator.compare_root(ChangeKind::RequestSchema, Some(&schema), None, "request");
        comparator.compare_root(ChangeKind::ResponseSchema, None, Some(&schema), "response");
        let changes = comparator.into_changes();

        assert_eq!(changes[0].detail, "body removed");
        assert_eq!(changes[0].kind, ChangeKind::RequestSchema);
        assert_eq!(changes[1].detail, "body added");
        assert_eq!(changes[1].kind, ChangeKind::ResponseSchema);
    }
}
