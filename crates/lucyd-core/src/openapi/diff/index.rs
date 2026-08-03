//! Flattens a document's `paths` object into a lookup keyed on endpoint
//! identity, so the two sides of a diff can be matched up.
//!
//! Identity is `(method, path template)` rather than `(method, path)`: a spec
//! that names its parameter `{userId}` and a Lucyd export that names the same
//! one `{id}` describe the same route, and reporting that as one removed plus
//! one added endpoint would be both wrong and noisy. The rename is reported on
//! its own by [`super::parameters`].

use super::resolver::SchemaResolver;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Every operation of a document, keyed by endpoint identity.
pub(super) type OperationIndex<'a> = BTreeMap<OperationKey, OperationView<'a>>;

/// HTTP verbs recognised as Path Item Object keys, in the order OpenAPI 3.1
/// lists them.
const HTTP_METHODS: &[&str] = &[
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

/// The media type Lucyd exports and the diff compares.
const APPLICATION_JSON: &str = "application/json";

/// Placeholder every path parameter collapses to in a path template.
const PARAMETER_PLACEHOLDER: &str = "{}";

/// Response key used when an operation declares no explicit success code.
const DEFAULT_RESPONSE: &str = "default";

/// The identity under which two operations are considered the same endpoint.
///
/// Ordering is derived so the report comes out in a stable, path-then-method
/// order regardless of the order either document happened to list them in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct OperationKey {
    /// The path with every parameter name erased, e.g. `/api/users/{}`.
    pub(super) template: String,
    /// Uppercase HTTP verb.
    pub(super) method: String,
}

/// One operation, plus everything needed to compare it without going back to
/// the document it came from.
pub(super) struct OperationView<'a> {
    /// The document this operation lives in, for resolving its `$ref`s.
    pub(super) document: &'a Value,
    /// The path template as written, kept for reporting.
    pub(super) path: &'a str,
    /// The Operation Object itself.
    pub(super) operation: &'a Map<String, Value>,
    /// The `parameters` declared on the enclosing Path Item Object, which
    /// apply to every operation under it.
    pub(super) path_item_parameters: Option<&'a Value>,
}

impl<'a> OperationView<'a> {
    /// A resolver for this operation's document.
    pub(super) fn resolver(&self) -> SchemaResolver<'a> {
        SchemaResolver::new(self.document)
    }

    /// The `application/json` request body schema, with a `$ref`'d body
    /// container resolved. `None` when the operation declares no JSON body.
    pub(super) fn request_schema(&self) -> Option<&'a Value> {
        let body = self
            .resolver()
            .resolve(self.operation.get("requestBody")?)?;
        json_schema_of(body)
    }

    /// The `application/json` schema declared for one specific status code.
    pub(super) fn response_schema(&self, code: &str) -> Option<&'a Value> {
        let response = self.resolver().resolve(self.responses()?.get(code)?)?;
        json_schema_of(response)
    }

    /// The schema of whichever response carries success on this side.
    ///
    /// Lucyd's exporter always emits `200`; a hand-written spec commonly uses
    /// `201` or `204`. Matching on "the success response" rather than on the
    /// literal code is what keeps a migration diff readable.
    pub(super) fn success_response_schema(&self) -> Option<&'a Value> {
        self.response_schema(self.success_status_code()?)
    }

    /// Every status code this operation declares, in ascending order.
    pub(super) fn status_codes(&self) -> BTreeSet<&'a str> {
        self.responses()
            .map(|responses| responses.keys().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// The lowest 2xx code declared, falling back to `default`.
    fn success_status_code(&self) -> Option<&'a str> {
        let codes = self.status_codes();
        codes
            .iter()
            .find(|code| code.starts_with('2'))
            .or_else(|| codes.get(DEFAULT_RESPONSE))
            .copied()
    }

    /// The Responses Object, when the operation declares one.
    fn responses(&self) -> Option<&'a Map<String, Value>> {
        self.operation.get("responses")?.as_object()
    }
}

/// Indexes every operation in `document["paths"]` by endpoint identity.
///
/// Two operations colliding on the same identity (only possible when a
/// document declares both `/users/{id}` and `/users/{key}`) keep the last one
/// seen. The alternative, silently dropping one, would hide an endpoint.
pub(super) fn index_operations(document: &Value) -> OperationIndex<'_> {
    let mut index = OperationIndex::new();
    let Some(paths) = document.get("paths").and_then(Value::as_object) else {
        return index;
    };

    for (path, path_item) in paths {
        let Some(item) = path_item.as_object() else {
            continue;
        };
        for method in HTTP_METHODS {
            let Some(operation) = item.get(*method).and_then(Value::as_object) else {
                continue;
            };
            let key = OperationKey {
                template: path_template(path),
                method: method.to_uppercase(),
            };
            index.insert(key, view_of(document, path, operation, item));
        }
    }

    index
}

/// Assembles the [`OperationView`] for one operation of a Path Item Object.
fn view_of<'a>(
    document: &'a Value,
    path: &'a str,
    operation: &'a Map<String, Value>,
    path_item: &'a Map<String, Value>,
) -> OperationView<'a> {
    OperationView {
        document,
        path,
        operation,
        path_item_parameters: path_item.get("parameters"),
    }
}

/// Erases parameter names from a path, so `/users/{id}` and `/users/{userId}`
/// share the template `/users/{}`.
fn path_template(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            if is_parameter_segment(segment) {
                PARAMETER_PLACEHOLDER
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Lists a path's parameter names in the order they appear, e.g.
/// `["orgId", "userId"]` for `/orgs/{orgId}/users/{userId}`.
pub(super) fn path_parameter_names(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|segment| is_parameter_segment(segment))
        .map(|segment| segment.trim_matches(['{', '}']))
        .collect()
}

/// Returns `true` for a whole segment wrapped in braces, e.g. `{id}`.
fn is_parameter_segment(segment: &str) -> bool {
    segment.len() > 2 && segment.starts_with('{') && segment.ends_with('}')
}

/// Reads `.content["application/json"].schema` off an already-resolved
/// Request Body or Response Object.
fn json_schema_of(body: &Value) -> Option<&Value> {
    body.get("content")?.get(APPLICATION_JSON)?.get("schema")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn every_verb_of_every_path_is_indexed() {
        let document = json!({
            "paths": {
                "/a": { "get": {}, "post": {} },
                "/b": { "delete": {} }
            }
        });

        let index = index_operations(&document);
        let keys: Vec<String> = index
            .keys()
            .map(|key| format!("{} {}", key.method, key.template))
            .collect();

        assert_eq!(keys, ["GET /a", "POST /a", "DELETE /b"]);
    }

    #[test]
    fn non_verb_keys_of_a_path_item_are_not_operations() {
        let document = json!({
            "paths": { "/a": { "summary": "ignored", "parameters": [], "get": {} } }
        });

        assert_eq!(index_operations(&document).len(), 1);
    }

    #[test]
    fn a_document_without_paths_indexes_nothing() {
        assert!(index_operations(&json!({ "openapi": "3.1.0" })).is_empty());
    }

    #[test]
    fn differently_named_parameters_share_an_identity() {
        assert_eq!(path_template("/users/{id}/posts"), "/users/{}/posts");
        assert_eq!(
            path_template("/users/{userId}/posts"),
            path_template("/users/{id}/posts"),
            "a renamed path parameter must not split one endpoint into two"
        );
    }

    #[test]
    fn a_literal_segment_is_never_mistaken_for_a_parameter() {
        assert_eq!(path_template("/api/users"), "/api/users");
        assert_eq!(path_template("/{}"), "/{}");
    }

    #[test]
    fn parameter_names_are_listed_in_path_order() {
        assert_eq!(
            path_parameter_names("/orgs/{orgId}/users/{userId}"),
            ["orgId", "userId"]
        );
        assert!(path_parameter_names("/health").is_empty());
    }

    #[test]
    fn request_and_response_schemas_are_read_through_their_containers() {
        let document = json!({
            "paths": {
                "/users": {
                    "post": {
                        "requestBody": { "$ref": "#/components/requestBodies/NewUser" },
                        "responses": {
                            "201": {
                                "content": { "application/json": { "schema": { "type": "object" } } }
                            }
                        }
                    }
                }
            },
            "components": {
                "requestBodies": {
                    "NewUser": {
                        "content": { "application/json": { "schema": { "type": "string" } } }
                    }
                }
            }
        });

        let index = index_operations(&document);
        let view = index.values().next().expect("one operation is indexed");

        assert_eq!(view.request_schema(), Some(&json!({ "type": "string" })));
        assert_eq!(
            view.success_response_schema(),
            Some(&json!({ "type": "object" })),
            "a 201-only operation must still expose its success schema"
        );
    }

    #[test]
    fn the_lowest_2xx_code_carries_success() {
        let document = json!({
            "paths": {
                "/a": {
                    "get": {
                        "responses": {
                            "404": { "content": { "application/json": { "schema": { "type": "null" } } } },
                            "202": { "content": { "application/json": { "schema": { "type": "integer" } } } },
                            "200": { "content": { "application/json": { "schema": { "type": "boolean" } } } }
                        }
                    }
                }
            }
        });

        let index = index_operations(&document);
        let view = index.values().next().expect("one operation is indexed");

        assert_eq!(
            view.success_response_schema(),
            Some(&json!({ "type": "boolean" }))
        );
        assert_eq!(
            view.status_codes().into_iter().collect::<Vec<_>>(),
            ["200", "202", "404"]
        );
    }

    #[test]
    fn default_is_the_success_response_of_last_resort() {
        let document = json!({
            "paths": {
                "/a": {
                    "get": {
                        "responses": {
                            "default": { "content": { "application/json": { "schema": { "type": "string" } } } }
                        }
                    }
                }
            }
        });

        let index = index_operations(&document);
        let view = index.values().next().expect("one operation is indexed");

        assert_eq!(
            view.success_response_schema(),
            Some(&json!({ "type": "string" }))
        );
    }
}
