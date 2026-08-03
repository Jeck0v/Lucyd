//! Compares the two operations indexed under the same endpoint identity.
//!
//! This is where the default profile earns its keep. Lucyd's exporter is
//! deliberately narrower than OpenAPI: it always emits a single `200`
//! response, never a `summary`, never `security` or `servers`. Comparing those
//! facets against a hand-written spec would report a difference on nearly
//! every operation of a migrating API and drown the real regressions, so they
//! are only compared under [`DiffOptions::strict`].

use super::DiffOptions;
use super::index::OperationView;
use super::parameters::parameter_changes;
use super::report::{Change, ChangeKind, describe};
use super::schema::SchemaComparator;

/// Operation metadata compared verbatim under `--strict`.
const COMPARED_METADATA: &[&str] = &["tags", "deprecated"];

/// Collects every difference between two operations describing the same
/// endpoint. An empty result means the two documents agree on its contract.
pub(super) fn compare_operations<'a>(
    baseline: &OperationView<'a>,
    candidate: &OperationView<'a>,
    options: &DiffOptions,
) -> Vec<Change> {
    let mut comparator = SchemaComparator::new(baseline.document, candidate.document);
    comparator.compare_root(
        ChangeKind::RequestSchema,
        baseline.request_schema(),
        candidate.request_schema(),
        "request",
    );
    compare_responses(&mut comparator, baseline, candidate, options);

    let mut changes = comparator.into_changes();
    changes.extend(parameter_changes(baseline, candidate, options));
    if options.strict {
        changes.extend(status_code_changes(baseline, candidate));
        changes.extend(metadata_changes(baseline, candidate));
    }
    changes
}

/// Pairs up response bodies before handing them to the schema comparator.
///
/// The default profile compares each side's success response whatever code
/// carries it, because Lucyd always answers `200` where a spec may say `201`.
/// `--strict` compares code by code, over the codes both sides declare; codes
/// present on only one side are reported by [`status_code_changes`] instead,
/// which describes them far better than "body added" would.
fn compare_responses<'a>(
    comparator: &mut SchemaComparator<'a>,
    baseline: &OperationView<'a>,
    candidate: &OperationView<'a>,
    options: &DiffOptions,
) {
    if options.strict {
        compare_responses_per_code(comparator, baseline, candidate);
        return;
    }
    comparator.compare_root(
        ChangeKind::ResponseSchema,
        baseline.success_response_schema(),
        candidate.success_response_schema(),
        "response",
    );
}

/// Compares response bodies code by code, over the codes both sides declare.
fn compare_responses_per_code<'a>(
    comparator: &mut SchemaComparator<'a>,
    baseline: &OperationView<'a>,
    candidate: &OperationView<'a>,
) {
    let (before, after) = (baseline.status_codes(), candidate.status_codes());
    for code in before.intersection(&after) {
        comparator.compare_root(
            ChangeKind::ResponseSchema,
            baseline.response_schema(code),
            candidate.response_schema(code),
            &format!("response({code})"),
        );
    }
}

/// Reports status codes declared on only one side.
fn status_code_changes(baseline: &OperationView<'_>, candidate: &OperationView<'_>) -> Vec<Change> {
    let (before, after) = (baseline.status_codes(), candidate.status_codes());
    let removed = before
        .difference(&after)
        .map(|code| status_change(code, "no longer declared"));
    let added = after
        .difference(&before)
        .map(|code| status_change(code, "newly declared"));
    removed.chain(added).collect()
}

/// Builds one status-code change entry.
fn status_change(code: &str, detail: &str) -> Change {
    Change::new(ChangeKind::StatusCode, format!("response({code})"), detail)
}

/// Reports operation metadata that differs.
fn metadata_changes(baseline: &OperationView<'_>, candidate: &OperationView<'_>) -> Vec<Change> {
    COMPARED_METADATA
        .iter()
        .filter_map(|key| {
            let before = baseline.operation.get(*key);
            let after = candidate.operation.get(*key);
            (before != after).then(|| {
                let detail = format!("changed: {} -> {}", describe(before), describe(after));
                Change::new(ChangeKind::Metadata, *key, detail)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::index::index_operations;
    use super::*;
    use serde_json::{Value, json};

    /// Wraps an operation body into a one-path document.
    fn document(operation: Value) -> Value {
        json!({ "paths": { "/users": { "post": operation } } })
    }

    /// A Response Object carrying a JSON body of the given schema.
    fn response(schema: Value) -> Value {
        json!({ "content": { "application/json": { "schema": schema } } })
    }

    /// Compares two single-operation documents into `location: detail` lines.
    fn changes(baseline: Value, candidate: Value, strict: bool) -> Vec<String> {
        let baseline_index = index_operations(&baseline);
        let candidate_index = index_operations(&candidate);
        let before = baseline_index.values().next().expect("one operation");
        let after = candidate_index.values().next().expect("one operation");

        compare_operations(before, after, &DiffOptions { strict })
            .iter()
            .map(|change| format!("{}: {}", change.location, change.detail))
            .collect()
    }

    #[test]
    fn a_different_success_code_carrying_the_same_body_is_not_a_change() {
        let spec =
            document(json!({ "responses": { "201": response(json!({ "type": "object" })) } }));
        let export =
            document(json!({ "responses": { "200": response(json!({ "type": "object" })) } }));

        assert!(
            changes(spec, export, false).is_empty(),
            "Lucyd always exports 200; a spec's 201 must not read as a regression by default"
        );
    }

    #[test]
    fn a_success_body_change_is_found_across_differing_codes() {
        let spec = document(json!({
            "responses": { "201": response(json!({ "properties": { "id": {}, "createdAt": {} } })) }
        }));
        let export = document(json!({
            "responses": { "200": response(json!({ "properties": { "id": {} } })) }
        }));

        assert_eq!(
            changes(spec, export, false),
            ["response: field \"createdAt\" removed"]
        );
    }

    #[test]
    fn strict_mode_reports_the_status_code_difference() {
        let spec =
            document(json!({ "responses": { "201": response(json!({ "type": "object" })) } }));
        let export =
            document(json!({ "responses": { "200": response(json!({ "type": "object" })) } }));

        assert_eq!(
            changes(spec, export, true),
            [
                "response(201): no longer declared",
                "response(200): newly declared"
            ]
        );
    }

    #[test]
    fn strict_mode_compares_error_responses_the_two_sides_share() {
        let before = document(json!({
            "responses": {
                "200": response(json!({ "type": "object" })),
                "404": response(json!({ "properties": { "message": {} } }))
            }
        }));
        let after = document(json!({
            "responses": {
                "200": response(json!({ "type": "object" })),
                "404": response(json!({ "properties": { "error": {} } }))
            }
        }));

        assert_eq!(
            changes(before, after, true),
            [
                "response(404): field \"message\" removed",
                "response(404): field \"error\" added"
            ]
        );
    }

    #[test]
    fn a_request_body_change_is_reported() {
        let body = |schema: Value| {
            document(json!({
                "requestBody": { "content": { "application/json": { "schema": schema } } },
                "responses": { "200": response(json!({ "type": "object" })) }
            }))
        };

        assert_eq!(
            changes(
                body(json!({ "required": ["name"] })),
                body(json!({ "required": ["name", "email"] })),
                false
            ),
            ["request: field \"email\" is now required"]
        );
    }

    #[test]
    fn metadata_is_compared_in_strict_mode_only() {
        let before = document(json!({ "tags": ["users"], "responses": {} }));
        let after = document(json!({ "tags": ["accounts"], "deprecated": true, "responses": {} }));

        assert!(changes(before.clone(), after.clone(), false).is_empty());
        assert_eq!(
            changes(before, after, true),
            [
                "tags: changed: [\"users\"] -> [\"accounts\"]",
                "deprecated: changed: absent -> true"
            ]
        );
    }

    #[test]
    fn descriptions_never_count_as_a_change() {
        let before = document(json!({
            "summary": "Create", "description": "Creates a user", "responses": {}
        }));
        let after = document(json!({
            "summary": "Add", "description": "Adds a user", "responses": {}
        }));

        assert!(changes(before.clone(), after.clone(), false).is_empty());
        assert!(
            changes(before, after, true).is_empty(),
            "prose is never a contract change, even under --strict"
        );
    }
}
