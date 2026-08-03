//! The result of an OpenAPI diff, and how it renders.
//!
//! Two renderings are provided, both here so that a CLI front-end never has to
//! reimplement either: [`fmt::Display`] for the human-readable report, and
//! [`DiffReport::to_json`] for the machine-readable one consumed by CI.

use serde_json::{Value, json};
use std::fmt;

/// Version of the JSON report envelope. Bumped whenever the shape changes in
/// a way an existing consumer could not parse.
const REPORT_VERSION: u32 = 1;

/// Rendered in place of a keyword or field that one side doesn't declare.
const ABSENT: &str = "absent";

/// The facet of an operation a [`Change`] was found on.
///
/// Callers use it to filter or colour a report; it is also what makes the
/// JSON output greppable in a CI log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// The `application/json` request body schema.
    RequestSchema,
    /// The `application/json` response body schema.
    ResponseSchema,
    /// A response status code declared on only one side.
    StatusCode,
    /// A path, query, header or cookie parameter.
    Parameter,
    /// Operation metadata: `tags`, `deprecated`.
    Metadata,
    /// A `$ref` that could not be followed, so the schema below it was never
    /// compared. Reported rather than silently treated as "no difference".
    Unresolved,
}

impl ChangeKind {
    /// Stable identifier used in the JSON report.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RequestSchema => "requestSchema",
            Self::ResponseSchema => "responseSchema",
            Self::StatusCode => "statusCode",
            Self::Parameter => "parameter",
            Self::Metadata => "metadata",
            Self::Unresolved => "unresolved",
        }
    }
}

/// A single difference found on one endpoint.
#[derive(Debug, Clone)]
pub struct Change {
    /// What kind of thing changed.
    pub kind: ChangeKind,
    /// Where the difference sits, e.g. `response.createdAt`, `request[]`, or
    /// `query parameter "page"`.
    pub location: String,
    /// Human-readable statement of the difference, e.g. `field "id" removed`.
    pub detail: String,
}

impl Change {
    /// Builds a change. Both text fields accept anything string-like so call
    /// sites can pass a literal or a `format!` without ceremony.
    pub(super) fn new(
        kind: ChangeKind,
        location: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            location: location.into(),
            detail: detail.into(),
        }
    }

    /// Serialises this change into its JSON report entry.
    fn to_json(&self) -> Value {
        json!({
            "kind": self.kind.as_str(),
            "location": self.location,
            "detail": self.detail,
        })
    }
}

/// An endpoint, identified the way a reader of the report expects to see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointRef {
    /// Uppercase HTTP verb.
    pub method: String,
    /// The path template as written in the document it came from.
    pub path: String,
}

impl EndpointRef {
    pub(super) fn new(method: &str, path: &str) -> Self {
        Self {
            method: method.to_string(),
            path: path.to_string(),
        }
    }

    /// Serialises this endpoint into its JSON report entry.
    fn to_json(&self) -> Value {
        json!({ "method": self.method, "path": self.path })
    }
}

impl fmt::Display for EndpointRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.method, self.path)
    }
}

/// An endpoint that exists on both sides but whose contract differs.
#[derive(Debug, Clone)]
pub struct ChangedEndpoint {
    /// The endpoint, named after the candidate document.
    pub endpoint: EndpointRef,
    /// Every difference found on it, never empty.
    pub changes: Vec<Change>,
}

impl ChangedEndpoint {
    /// Serialises this endpoint and its changes into its JSON report entry.
    fn to_json(&self) -> Value {
        json!({
            "method": self.endpoint.method,
            "path": self.endpoint.path,
            "changes": self.changes.iter().map(Change::to_json).collect::<Vec<_>>(),
        })
    }
}

/// Which categories of difference should be treated as a failure.
///
/// The default matches the recommended CI setting: a lost or altered endpoint
/// breaks callers, whereas a newly added one usually does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailOn {
    /// Fail when an endpoint disappeared.
    pub missing: bool,
    /// Fail when an endpoint appeared.
    pub added: bool,
    /// Fail when an endpoint's contract changed.
    pub changed: bool,
}

impl Default for FailOn {
    fn default() -> Self {
        Self {
            missing: true,
            added: false,
            changed: true,
        }
    }
}

/// Everything one document has that the other doesn't, and everything they
/// both have but describe differently.
#[derive(Debug, Clone, Default)]
pub struct DiffReport {
    /// Endpoints in the baseline document, absent from the candidate.
    pub missing: Vec<EndpointRef>,
    /// Endpoints in the candidate document, absent from the baseline.
    pub added: Vec<EndpointRef>,
    /// Endpoints present on both sides whose contract differs.
    pub changed: Vec<ChangedEndpoint>,
}

impl DiffReport {
    /// Returns `true` when the two documents describe the same API.
    pub fn is_empty(&self) -> bool {
        self.missing.is_empty() && self.added.is_empty() && self.changed.is_empty()
    }

    /// Returns `true` when the report contains a difference in a category the
    /// caller asked to fail on.
    pub fn is_failure(&self, fail_on: &FailOn) -> bool {
        (fail_on.missing && !self.missing.is_empty())
            || (fail_on.added && !self.added.is_empty())
            || (fail_on.changed && !self.changed.is_empty())
    }

    /// Renders the machine-readable report consumed by CI pipelines.
    pub fn to_json(&self) -> Value {
        json!({
            "lucydDiffVersion": REPORT_VERSION,
            "summary": {
                "missing": self.missing.len(),
                "added": self.added.len(),
                "changed": self.changed.len(),
            },
            "missing": self.missing.iter().map(EndpointRef::to_json).collect::<Vec<_>>(),
            "added": self.added.iter().map(EndpointRef::to_json).collect::<Vec<_>>(),
            "changed": self.changed.iter().map(ChangedEndpoint::to_json).collect::<Vec<_>>(),
        })
    }
}

impl fmt::Display for DiffReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "OpenAPI diff report")?;
        if self.is_empty() {
            return write!(f, "\nNo differences found.");
        }
        write_endpoint_section(f, "Missing", &self.missing)?;
        write_endpoint_section(f, "Added", &self.added)?;
        write_changed_section(f, &self.changed)?;
        write!(
            f,
            "\nSummary:\n{} missing, {} added, {} changed",
            self.missing.len(),
            self.added.len(),
            self.changed.len()
        )
    }
}

/// Writes a `Missing:`/`Added:` block, or nothing when the list is empty.
fn write_endpoint_section(
    f: &mut fmt::Formatter<'_>,
    title: &str,
    endpoints: &[EndpointRef],
) -> fmt::Result {
    if endpoints.is_empty() {
        return Ok(());
    }
    writeln!(f, "\n{title}:")?;
    for endpoint in endpoints {
        writeln!(f, "  {endpoint}")?;
    }
    Ok(())
}

/// Writes the `Changed:` block, nesting each endpoint's differences under it.
fn write_changed_section(f: &mut fmt::Formatter<'_>, changed: &[ChangedEndpoint]) -> fmt::Result {
    if changed.is_empty() {
        return Ok(());
    }
    writeln!(f, "\nChanged:")?;
    for entry in changed {
        writeln!(f, "  {}", entry.endpoint)?;
        for change in &entry.changes {
            writeln!(f, "    {}: {}", change.location, change.detail)?;
        }
    }
    Ok(())
}

/// Renders a JSON value for a report line: compact JSON, or `absent` when the
/// side being described doesn't declare it at all.
///
/// Shared by every comparator so that "what changed from what" reads the same
/// whether it came from a schema keyword or an operation's metadata.
pub(super) fn describe(value: Option<&Value>) -> String {
    value.map_or_else(|| ABSENT.to_string(), Value::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_report() -> DiffReport {
        DiffReport {
            missing: vec![EndpointRef::new("DELETE", "/api/users/{id}")],
            added: vec![EndpointRef::new("POST", "/api/ping")],
            changed: vec![ChangedEndpoint {
                endpoint: EndpointRef::new("GET", "/api/users/{id}"),
                changes: vec![Change::new(
                    ChangeKind::ResponseSchema,
                    "response",
                    "field \"createdAt\" removed",
                )],
            }],
        }
    }

    #[test]
    fn empty_report_is_empty_and_never_fails() {
        let report = DiffReport::default();

        assert!(report.is_empty());
        assert!(!report.is_failure(&FailOn::default()));
        assert!(report.to_string().contains("No differences found."));
    }

    #[test]
    fn added_endpoints_alone_do_not_fail_by_default() {
        let report = DiffReport {
            added: vec![EndpointRef::new("POST", "/api/ping")],
            ..DiffReport::default()
        };

        assert!(!report.is_empty());
        assert!(
            !report.is_failure(&FailOn::default()),
            "a purely additive change must not break a CI pipeline by default"
        );
        assert!(report.is_failure(&FailOn {
            added: true,
            ..FailOn::default()
        }));
    }

    #[test]
    fn human_output_lists_every_section_and_a_summary() {
        let rendered = sample_report().to_string();

        assert!(rendered.contains("Missing:\n  DELETE /api/users/{id}"));
        assert!(rendered.contains("Added:\n  POST /api/ping"));
        assert!(rendered.contains("Changed:\n  GET /api/users/{id}"));
        assert!(rendered.contains("    response: field \"createdAt\" removed"));
        assert!(rendered.contains("1 missing, 1 added, 1 changed"));
    }

    #[test]
    fn empty_sections_are_omitted_from_human_output() {
        let report = DiffReport {
            missing: vec![EndpointRef::new("DELETE", "/api/users/{id}")],
            ..DiffReport::default()
        };
        let rendered = report.to_string();

        assert!(rendered.contains("Missing:"));
        assert!(!rendered.contains("Added:"));
        assert!(!rendered.contains("Changed:"));
    }

    #[test]
    fn json_output_carries_a_version_summary_and_every_entry() {
        let json = sample_report().to_json();

        assert_eq!(json["lucydDiffVersion"], REPORT_VERSION);
        assert_eq!(json["summary"]["missing"], 1);
        assert_eq!(json["missing"][0]["method"], "DELETE");
        assert_eq!(json["added"][0]["path"], "/api/ping");
        assert_eq!(json["changed"][0]["changes"][0]["kind"], "responseSchema");
        assert_eq!(json["changed"][0]["changes"][0]["location"], "response");
    }

    #[test]
    fn describe_marks_an_absent_value() {
        assert_eq!(describe(None), ABSENT);
        assert_eq!(describe(Some(&json!("string"))), "\"string\"");
    }
}
