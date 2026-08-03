//! Compares two OpenAPI documents and reports what an API lost, gained, or
//! changed between them.
//!
//! This is the validation half of the migration workflow that
//! [`super::generate_openapi_document`] and `cargo xtask import-openapi` make
//! up: an existing spec becomes Lucyd code, the running application exports a
//! spec again, and diffing the two proves nothing was dropped on the way.
//!
//! # The default profile
//!
//! Lucyd's exporter is deliberately narrower than OpenAPI: one `200` response
//! per operation, every path parameter typed `string`, no `summary`, no
//! `security`. Compared literally against a hand-written spec, that reports a
//! difference on nearly every operation and buries the regressions that
//! matter. So by default the diff ignores what Lucyd cannot express and
//! compares what callers actually depend on; [`DiffOptions::strict`] turns the
//! literal comparison back on for diffing two arbitrary documents.
//!
//! # Example
//!
//! ```
//! # use serde_json::json;
//! use lucy_core::openapi::diff::{DiffOptions, diff_documents};
//!
//! let baseline = json!({ "paths": { "/users": { "get": {} } } });
//! let candidate = json!({ "paths": {} });
//!
//! let report = diff_documents(&baseline, &candidate, &DiffOptions::default());
//!
//! assert_eq!(report.missing.len(), 1);
//! assert_eq!(report.missing[0].to_string(), "GET /users");
//! ```

mod index;
mod operation;
mod parameters;
mod report;
mod resolver;
mod schema;

pub use report::{Change, ChangeKind, ChangedEndpoint, DiffReport, EndpointRef, FailOn};

use index::{OperationIndex, index_operations};
use operation::compare_operations;
use serde_json::Value;

/// How strictly two documents should be compared.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiffOptions {
    /// Compare facets Lucyd's exporter cannot express: exact response status
    /// codes, path parameter types, `tags`, `deprecated`.
    ///
    /// Off by default, which is what makes the report readable when the
    /// candidate document came out of Lucyd. Turn it on to diff two documents
    /// that neither side generated.
    pub strict: bool,
}

/// Compares `baseline` against `candidate` and reports their differences.
///
/// `baseline` is the contract being protected (typically the pre-migration
/// spec) and `candidate` the one being checked against it (typically Lucyd's
/// `/docs/openapi.json`). Swapping them turns every `missing` into an `added`.
///
/// Never fails: a document with no `paths`, or one whose `$ref`s don't
/// resolve, yields a report rather than an error. An unusable input should
/// show up as a diff to read, not as a tool crash in the middle of a pipeline.
pub fn diff_documents(baseline: &Value, candidate: &Value, options: &DiffOptions) -> DiffReport {
    let before = index_operations(baseline);
    let after = index_operations(candidate);

    DiffReport {
        missing: endpoints_absent_from(&before, &after),
        added: endpoints_absent_from(&after, &before),
        changed: changed_endpoints(&before, &after, options),
    }
}

/// Collects the endpoints `source` declares and `other` does not.
fn endpoints_absent_from(
    source: &OperationIndex<'_>,
    other: &OperationIndex<'_>,
) -> Vec<EndpointRef> {
    source
        .iter()
        .filter(|(key, _)| !other.contains_key(*key))
        .map(|(key, view)| EndpointRef::new(&key.method, view.path))
        .collect()
}

/// Compares every endpoint both documents declare, keeping the ones that
/// actually differ.
fn changed_endpoints(
    before: &OperationIndex<'_>,
    after: &OperationIndex<'_>,
    options: &DiffOptions,
) -> Vec<ChangedEndpoint> {
    before
        .iter()
        .filter_map(|(key, baseline)| Some((key, baseline, after.get(key)?)))
        .filter_map(|(key, baseline, candidate)| {
            let changes = compare_operations(baseline, candidate, options);
            (!changes.is_empty()).then(|| ChangedEndpoint {
                endpoint: EndpointRef::new(&key.method, candidate.path),
                changes,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::generate_openapi_document;
    use super::super::test_support::http_endpoint;
    use super::*;
    use crate::registry::EndpointRegistry;
    use serde_json::json;

    const USER_PATH: &str = "/api/users/{id}";

    /// Diffs on the default profile.
    fn diff(baseline: &Value, candidate: &Value) -> DiffReport {
        diff_documents(baseline, candidate, &DiffOptions::default())
    }

    /// A hand-written spec of one `GET /api/users/{id}` returning a user,
    /// written the way a real spec is: a numeric path parameter, a documented
    /// `200`, a named component, prose everywhere.
    fn handwritten_spec() -> Value {
        json!({
            "openapi": "3.0.3",
            "paths": {
                USER_PATH: {
                    "get": {
                        "operationId": "getUser",
                        "summary": "Fetch a user",
                        "parameters": [{
                            "name": "id", "in": "path", "required": true,
                            "description": "The user's identifier",
                            "schema": { "type": "integer" }
                        }],
                        "responses": {
                            "200": {
                                "description": "The user",
                                "content": { "application/json": {
                                    "schema": { "$ref": "#/components/schemas/User" }
                                } }
                            }
                        }
                    }
                }
            },
            "components": {
                "schemas": {
                    "User": {
                        "type": "object",
                        "description": "A registered user",
                        "properties": {
                            "id": { "type": "integer" },
                            "name": { "type": "string" }
                        },
                        "required": ["id", "name"]
                    }
                }
            }
        })
    }

    /// The document Lucyd exports for the migrated equivalent of
    /// [`handwritten_spec`], produced by the real exporter rather than by hand.
    fn lucyd_export(response_properties: Value, required: Value) -> Value {
        let mut endpoint = http_endpoint("get_user", "GET", USER_PATH);
        endpoint.response_schema = Some(json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "User",
            "type": "object",
            "properties": response_properties,
            "required": required
        }));

        let mut registry = EndpointRegistry::new();
        registry.register(endpoint);
        generate_openapi_document(&registry)
    }

    #[test]
    fn a_faithful_migration_reports_nothing() {
        let export = lucyd_export(
            json!({ "id": { "type": "integer" }, "name": { "type": "string" } }),
            json!(["id", "name"]),
        );

        let report = diff(&handwritten_spec(), &export);

        assert!(
            report.is_empty(),
            "a faithful migration must produce a clean report, got:\n{report}"
        );
        assert!(!report.is_failure(&FailOn::default()));
    }

    #[test]
    fn strict_mode_surfaces_what_lucyd_cannot_express() {
        let export = lucyd_export(
            json!({ "id": { "type": "integer" }, "name": { "type": "string" } }),
            json!(["id", "name"]),
        );

        let report = diff_documents(&handwritten_spec(), &export, &DiffOptions { strict: true });

        let details: Vec<&str> = report.changed[0]
            .changes
            .iter()
            .map(|change| change.detail.as_str())
            .collect();
        assert_eq!(
            details,
            ["type changed: integer -> string"],
            "the only strict-mode difference is the path parameter Lucyd exports as a string"
        );
    }

    #[test]
    fn a_dropped_response_field_survives_the_default_profile() {
        let export = lucyd_export(json!({ "id": { "type": "integer" } }), json!(["id"]));

        let report = diff(&handwritten_spec(), &export);

        assert_eq!(report.changed.len(), 1);
        assert_eq!(
            report.changed[0].endpoint.to_string(),
            "GET /api/users/{id}"
        );
        let details: Vec<&str> = report.changed[0]
            .changes
            .iter()
            .map(|change| change.detail.as_str())
            .collect();
        assert_eq!(
            details,
            [
                "field \"name\" removed",
                "field \"name\" is no longer required"
            ]
        );
        assert!(report.is_failure(&FailOn::default()));
    }

    #[test]
    fn an_endpoint_lost_in_migration_is_missing() {
        let candidate = json!({ "paths": { "/api/ping": { "post": {} } } });
        let baseline = json!({ "paths": { USER_PATH: { "delete": {} } } });

        let report = diff(&baseline, &candidate);

        assert_eq!(report.missing, [EndpointRef::new("DELETE", USER_PATH)]);
        assert_eq!(report.added, [EndpointRef::new("POST", "/api/ping")]);
        assert!(report.changed.is_empty());
    }

    #[test]
    fn a_method_swap_on_one_path_is_a_removal_and_an_addition() {
        let baseline = json!({ "paths": { "/api/items": { "post": {} } } });
        let candidate = json!({ "paths": { "/api/items": { "put": {} } } });

        let report = diff(&baseline, &candidate);

        assert_eq!(report.missing, [EndpointRef::new("POST", "/api/items")]);
        assert_eq!(report.added, [EndpointRef::new("PUT", "/api/items")]);
    }

    #[test]
    fn a_renamed_path_parameter_stays_one_endpoint() {
        let baseline = json!({ "paths": { "/api/users/{id}": { "get": {} } } });
        let candidate = json!({ "paths": { "/api/users/{userId}": { "get": {} } } });

        let report = diff(&baseline, &candidate);

        assert!(report.missing.is_empty(), "a rename is not a lost endpoint");
        assert!(report.added.is_empty(), "a rename is not a new endpoint");
        assert_eq!(report.changed[0].changes[0].detail, "renamed to \"userId\"");
    }

    #[test]
    fn identical_documents_report_nothing() {
        let document = handwritten_spec();

        assert!(diff(&document, &document).is_empty());
        assert!(
            diff_documents(&document, &document, &DiffOptions { strict: true }).is_empty(),
            "a document must always be identical to itself, in either mode"
        );
    }

    #[test]
    fn an_empty_document_loses_every_endpoint() {
        let report = diff(&handwritten_spec(), &json!({}));

        assert_eq!(report.missing.len(), 1);
        assert!(report.changed.is_empty());
    }

    #[test]
    fn the_report_lists_endpoints_in_a_stable_order() {
        let baseline = json!({
            "paths": {
                "/z": { "get": {}, "delete": {} },
                "/a": { "post": {} }
            }
        });

        let report = diff(&baseline, &json!({}));
        let listed: Vec<String> = report.missing.iter().map(EndpointRef::to_string).collect();

        assert_eq!(
            listed,
            ["POST /a", "DELETE /z", "GET /z"],
            "ordering must not depend on the order either document listed its paths in"
        );
    }
}
