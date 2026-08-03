//! End-to-end proof for `query = T`: a hand-written OpenAPI document declaring
//! query parameters, and the Lucyd export of its migrated equivalent, must
//! diff clean.
//!
//! This is the acceptance criterion the feature was specified against, and it
//! is the one that closes the loop the other tests only cover a segment of:
//! `diff/parameters.rs` already knew how to compare `in: query` parameters long
//! before `EndpointMeta` could declare any, so if the exporter's Parameter
//! Objects are shaped even slightly differently from what a human would write,
//! the diff says so here rather than in a user's migration.

#![cfg(feature = "openapi-diff")]

use lucyd_core::openapi::diff::{DiffOptions, diff_documents};
use lucyd_core::openapi::generate_openapi_document;
use lucyd_core::registry::EndpointRegistry;
use lucyd_types::endpoint::{EndpointMeta, Protocol};
use serde_json::{Value, json};

/// The spec as a human wrote it before the migration: one required parameter,
/// one optional one, each with a plain scalar type.
fn hand_written_spec() -> Value {
    json!({
        "openapi": "3.1.0",
        "info": { "title": "Scores API", "version": "1.0.0" },
        "paths": {
            "/api/boards/{board_id}/scores": {
                "get": {
                    "operationId": "scores",
                    "parameters": [
                        {
                            "name": "board_id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string" }
                        },
                        {
                            "name": "limit",
                            "in": "query",
                            "required": false,
                            "description": "Maximum number of rows returned.",
                            "schema": { "type": "integer" }
                        },
                        {
                            "name": "since",
                            "in": "query",
                            "required": true,
                            "schema": { "type": "string" }
                        }
                    ],
                    "responses": { "200": { "description": "Successful response" } }
                }
            }
        }
    })
}

/// What `schemars::schema_for!(ScoreFilters)` emits for the struct the
/// migration produces:
///
/// ```ignore
/// #[derive(Deserialize, JsonSchema)]
/// pub struct ScoreFilters {
///     /// Maximum number of rows returned.
///     pub limit: Option<u32>,
///     pub since: String,
/// }
/// ```
///
/// Written out literally rather than derived, so the test states the exact
/// input the exporter has to cope with — including the `["integer", "null"]`
/// union schemars gives an `Option<T>`, which is the whole reason the exporter
/// collapses nullable unions.
fn migrated_query_schema() -> Value {
    json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "ScoreFilters",
        "type": "object",
        "required": ["since"],
        "properties": {
            "limit": {
                "description": "Maximum number of rows returned.",
                "type": ["integer", "null"],
                "format": "uint32",
                "minimum": 0.0
            },
            "since": { "type": "string" }
        }
    })
}

/// The document the migrated application exports at `/docs/openapi.json`.
fn lucyd_export() -> Value {
    let mut endpoint = EndpointMeta::new("scores", "/api/boards/{board_id}/scores", Protocol::Http);
    endpoint.method = Some("GET".to_string());
    endpoint.query_schema = Some(migrated_query_schema());

    let mut registry = EndpointRegistry::new();
    registry.register(endpoint);
    generate_openapi_document(&registry)
}

/// Renders a report's changes as `METHOD /path — location: detail` lines.
fn change_lines(report: &lucyd_core::openapi::diff::DiffReport) -> Vec<String> {
    report
        .changed
        .iter()
        .flat_map(|endpoint| {
            endpoint.changes.iter().map(move |change| {
                format!(
                    "{} — {}: {}",
                    endpoint.endpoint, change.location, change.detail
                )
            })
        })
        .collect()
}

#[test]
fn the_export_of_a_migrated_endpoint_diffs_clean_against_the_hand_written_spec() {
    let report = diff_documents(
        &hand_written_spec(),
        &lucyd_export(),
        &DiffOptions::default(),
    );

    assert!(
        report.missing.is_empty(),
        "the migrated endpoint must still be there: {:?}",
        report.missing
    );
    assert!(
        report.added.is_empty(),
        "the export must not invent an endpoint: {:?}",
        report.added
    );
    assert_eq!(
        change_lines(&report),
        Vec::<String>::new(),
        "a faithfully migrated query string must produce no differences at all"
    );
}

#[test]
fn the_declared_types_survive_a_strict_diff_too() {
    let report = diff_documents(
        &hand_written_spec(),
        &lucyd_export(),
        &DiffOptions { strict: true },
    );

    assert_eq!(
        change_lines(&report),
        // Lucyd types every path parameter as a string and emits a single 200,
        // both of which `--strict` compares literally — documented exporter
        // limitations, not something `query = T` introduced. No `query
        // parameter "..."` line may appear among them.
        Vec::<String>::new(),
        "`--strict` must not report a query parameter type: schemars' \
         `[\"integer\", \"null\"]` has to reach the export as a plain `integer`"
    );
}

#[test]
fn a_dropped_query_parameter_is_still_caught() {
    // Guardrail: the two tests above would also pass if the diff were blind to
    // query parameters. Removing one from the export must be reported.
    let mut endpoint = EndpointMeta::new("scores", "/api/boards/{board_id}/scores", Protocol::Http);
    endpoint.method = Some("GET".to_string());
    endpoint.query_schema = Some(json!({
        "title": "ScoreFilters",
        "type": "object",
        "required": ["since"],
        "properties": { "since": { "type": "string" } }
    }));

    let mut registry = EndpointRegistry::new();
    registry.register(endpoint);

    let report = diff_documents(
        &hand_written_spec(),
        &generate_openapi_document(&registry),
        &DiffOptions::default(),
    );

    assert_eq!(
        change_lines(&report),
        ["GET /api/boards/{board_id}/scores — query parameter \"limit\": removed"]
    );
}
