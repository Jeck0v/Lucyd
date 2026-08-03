//! Compares the parameters two operations declare.
//!
//! Path parameters and the rest are handled differently on purpose. A path
//! parameter's identity is its *position* in the URL template, which already
//! had to match for the two operations to be paired at all, so only its name
//! (a rename) and, under `--strict`, its declared type are compared. Query,
//! header and cookie parameters have no positional identity and are compared
//! as a set keyed on `(in, name)`.

use super::DiffOptions;
use super::index::{OperationView, path_parameter_names};
use super::report::{Change, ChangeKind};
use serde_json::Value;
use std::collections::BTreeMap;

/// The `in` value of a parameter carried by the URL path itself.
const IN_PATH: &str = "path";

/// Rendered for a parameter that declares no type.
const UNTYPED: &str = "untyped";

/// A parameter reduced to the facets that affect callers.
struct ParameterView {
    /// The `in` value: `path`, `query`, `header` or `cookie`.
    location: String,
    name: String,
    required: bool,
    /// `schema.type`, when the parameter declares one.
    declared_type: Option<String>,
}

impl ParameterView {
    /// Reads a (already `$ref`-resolved) Parameter Object, ignoring one that
    /// lacks the `in`/`name` pair that identifies it.
    fn read(parameter: &Value) -> Option<Self> {
        Some(Self {
            location: parameter.get("in")?.as_str()?.to_string(),
            name: parameter.get("name")?.as_str()?.to_string(),
            required: parameter
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            declared_type: parameter
                .pointer("/schema/type")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
    }

    /// The identity two parameters must share to be compared with each other.
    fn key(&self) -> (String, String) {
        (self.location.clone(), self.name.clone())
    }

    /// How this parameter is named in a report line.
    fn label(&self) -> String {
        format!("{} parameter \"{}\"", self.location, self.name)
    }

    /// Builds a change about this parameter, e.g. `removed`.
    fn change(&self, detail: impl Into<String>) -> Change {
        Change::new(ChangeKind::Parameter, self.label(), detail)
    }

    /// The declared type, or a placeholder when there is none.
    fn type_name(&self) -> &str {
        self.declared_type.as_deref().unwrap_or(UNTYPED)
    }
}

/// Compares every parameter of two paired operations.
pub(super) fn parameter_changes(
    baseline: &OperationView<'_>,
    candidate: &OperationView<'_>,
    options: &DiffOptions,
) -> Vec<Change> {
    let mut changes = renamed_path_parameters(baseline.path, candidate.path);
    changes.extend(keyed_parameter_changes(baseline, candidate, options));
    if options.strict {
        changes.extend(path_parameter_type_changes(baseline, candidate));
    }
    changes
}

/// Reports path parameters renamed between the two documents, position by
/// position: `/users/{id}` against `/users/{userId}`.
fn renamed_path_parameters(baseline_path: &str, candidate_path: &str) -> Vec<Change> {
    path_parameter_names(baseline_path)
        .into_iter()
        .zip(path_parameter_names(candidate_path))
        .filter(|(before, after)| before != after)
        .map(|(before, after)| {
            Change::new(
                ChangeKind::Parameter,
                format!("path parameter \"{before}\""),
                format!("renamed to \"{after}\""),
            )
        })
        .collect()
}

/// Compares the query, header and cookie parameters as a keyed set.
fn keyed_parameter_changes(
    baseline: &OperationView<'_>,
    candidate: &OperationView<'_>,
    options: &DiffOptions,
) -> Vec<Change> {
    let before = keyed_parameters(baseline);
    let after = keyed_parameters(candidate);

    let mut changes: Vec<Change> = before
        .iter()
        .flat_map(|(key, parameter)| match after.get(key) {
            Some(counterpart) => compare_parameter(parameter, counterpart, options),
            None => vec![parameter.change("removed")],
        })
        .collect();
    changes.extend(
        after
            .iter()
            .filter(|(key, _)| !before.contains_key(*key))
            .map(|(_, parameter)| parameter.change("added")),
    );
    changes
}

/// Reports what differs between two parameters that share an identity.
fn compare_parameter(
    before: &ParameterView,
    after: &ParameterView,
    options: &DiffOptions,
) -> Vec<Change> {
    let requiredness = (before.required != after.required).then(|| {
        after.change(if after.required {
            "is now required"
        } else {
            "is no longer required"
        })
    });
    let retyped = (options.strict && before.declared_type != after.declared_type).then(|| {
        after.change(format!(
            "type changed: {} -> {}",
            before.type_name(),
            after.type_name()
        ))
    });
    requiredness.into_iter().chain(retyped).collect()
}

/// Compares path parameter types position by position.
///
/// Only reachable under `--strict`: Lucyd's exporter declares every path
/// parameter as a `string`, so on the default profile this would fire on every
/// numeric identifier in a real spec without describing a real regression.
fn path_parameter_type_changes(
    baseline: &OperationView<'_>,
    candidate: &OperationView<'_>,
) -> Vec<Change> {
    path_parameter_types(baseline)
        .into_iter()
        .zip(path_parameter_types(candidate))
        .filter(|(before, after)| before.1 != after.1)
        .map(|((name, before), (_, after))| {
            Change::new(
                ChangeKind::Parameter,
                format!("path parameter \"{name}\""),
                format!("type changed: {before} -> {after}"),
            )
        })
        .collect()
}

/// Lists `(name, type)` for each path parameter, in URL template order.
///
/// Driven by the path rather than by the `parameters` array so both sides line
/// up positionally even when one of them declares them in a different order.
fn path_parameter_types(view: &OperationView<'_>) -> Vec<(String, String)> {
    let declared = collect_parameters(view);
    path_parameter_names(view.path)
        .into_iter()
        .map(|name| {
            let declaration = declared
                .iter()
                .find(|parameter| parameter.location == IN_PATH && parameter.name == name);
            let type_name = declaration.map_or(UNTYPED, ParameterView::type_name);
            (name.to_string(), type_name.to_string())
        })
        .collect()
}

/// Indexes an operation's non-path parameters by `(in, name)`.
fn keyed_parameters(view: &OperationView<'_>) -> BTreeMap<(String, String), ParameterView> {
    collect_parameters(view)
        .into_iter()
        .filter(|parameter| parameter.location != IN_PATH)
        .map(|parameter| (parameter.key(), parameter))
        .collect()
}

/// Reads every parameter that applies to an operation, resolving `$ref`s.
///
/// Path Item-level parameters come first so that an operation-level
/// declaration of the same `(in, name)` overrides them, as OpenAPI requires.
fn collect_parameters(view: &OperationView<'_>) -> Vec<ParameterView> {
    let resolver = view.resolver();
    [view.path_item_parameters, view.operation.get("parameters")]
        .into_iter()
        .flatten()
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(|parameter| resolver.resolve(parameter))
        .filter_map(ParameterView::read)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::index::index_operations;
    use super::*;
    use serde_json::json;

    /// Builds a one-operation document around a `get` on `path`.
    fn document(path: &str, operation: Value) -> Value {
        json!({ "paths": { path: { "get": operation } } })
    }

    /// Diffs two single-operation documents, returning `location: detail` lines.
    fn changes(baseline: Value, candidate: Value, strict: bool) -> Vec<String> {
        let baseline_index = index_operations(&baseline);
        let candidate_index = index_operations(&candidate);
        let before = baseline_index.values().next().expect("one operation");
        let after = candidate_index.values().next().expect("one operation");

        parameter_changes(before, after, &DiffOptions { strict })
            .iter()
            .map(|change| format!("{}: {}", change.location, change.detail))
            .collect()
    }

    #[test]
    fn a_renamed_path_parameter_is_reported_once_as_a_rename() {
        let lines = changes(
            document("/users/{id}", json!({})),
            document("/users/{userId}", json!({})),
            false,
        );

        assert_eq!(lines, ["path parameter \"id\": renamed to \"userId\""]);
    }

    #[test]
    fn a_removed_query_parameter_is_reported() {
        let lines = changes(
            document(
                "/users",
                json!({ "parameters": [{ "name": "page", "in": "query" }] }),
            ),
            document("/users", json!({})),
            false,
        );

        assert_eq!(lines, ["query parameter \"page\": removed"]);
    }

    #[test]
    fn an_added_query_parameter_is_reported() {
        let lines = changes(
            document("/users", json!({})),
            document(
                "/users",
                json!({ "parameters": [{ "name": "page", "in": "query" }] }),
            ),
            false,
        );

        assert_eq!(lines, ["query parameter \"page\": added"]);
    }

    #[test]
    fn a_newly_required_query_parameter_is_reported() {
        let optional = json!({ "parameters": [{ "name": "page", "in": "query" }] });
        let required =
            json!({ "parameters": [{ "name": "page", "in": "query", "required": true }] });

        assert_eq!(
            changes(
                document("/users", optional.clone()),
                document("/users", required.clone()),
                false
            ),
            ["query parameter \"page\": is now required"]
        );
        assert_eq!(
            changes(
                document("/users", required),
                document("/users", optional),
                false
            ),
            ["query parameter \"page\": is no longer required"]
        );
    }

    #[test]
    fn path_item_level_parameters_apply_to_every_operation() {
        let baseline = json!({
            "paths": {
                "/users": {
                    "parameters": [{ "name": "trace", "in": "header" }],
                    "get": {}
                }
            }
        });

        let lines = changes(baseline, document("/users", json!({})), false);

        assert_eq!(
            lines,
            ["header parameter \"trace\": removed"],
            "a parameter declared on the path item must not be invisible to the diff"
        );
    }

    #[test]
    fn a_ref_to_a_shared_parameter_is_resolved() {
        let baseline = json!({
            "paths": {
                "/users": {
                    "get": { "parameters": [{ "$ref": "#/components/parameters/Page" }] }
                }
            },
            "components": {
                "parameters": { "Page": { "name": "page", "in": "query", "required": true } }
            }
        });
        let candidate = document(
            "/users",
            json!({ "parameters": [{ "name": "page", "in": "query" }] }),
        );

        assert_eq!(
            changes(baseline, candidate, false),
            ["query parameter \"page\": is no longer required"]
        );
    }

    #[test]
    fn a_path_parameter_type_difference_is_strict_only() {
        let spec = document(
            "/users/{id}",
            json!({ "parameters": [{ "name": "id", "in": "path", "schema": { "type": "integer" } }] }),
        );
        let export = document(
            "/users/{id}",
            json!({ "parameters": [{ "name": "id", "in": "path", "schema": { "type": "string" } }] }),
        );

        assert!(
            changes(spec.clone(), export.clone(), false).is_empty(),
            "Lucyd exports every path parameter as a string; that must not be noise by default"
        );
        assert_eq!(
            changes(spec, export, true),
            ["path parameter \"id\": type changed: integer -> string"]
        );
    }

    #[test]
    fn a_query_parameter_type_difference_is_strict_only() {
        let before = document(
            "/users",
            json!({ "parameters": [{ "name": "page", "in": "query", "schema": { "type": "integer" } }] }),
        );
        let after = document(
            "/users",
            json!({ "parameters": [{ "name": "page", "in": "query", "schema": { "type": "string" } }] }),
        );

        assert!(changes(before.clone(), after.clone(), false).is_empty());
        assert_eq!(
            changes(before, after, true),
            ["query parameter \"page\": type changed: integer -> string"]
        );
    }
}
