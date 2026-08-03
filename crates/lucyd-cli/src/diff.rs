//! The `diff` command: resolve both documents, compare them, print the report.
//!
//! Every step here delegates. The comparison is
//! [`lucyd_core::openapi::diff::diff_documents`], both renderings belong to
//! [`DiffReport`], and the pass/fail rule is
//! [`DiffReport::is_failure`]. What is left is the sequence, which is the only
//! thing a command should own.

use crate::Status;
use crate::cli::{DiffArgs, Format};
use crate::discovery::{self, BASELINE, CANDIDATE};
use crate::source;
use lucyd_core::openapi::diff::{DiffReport, diff_documents};
use std::{env, path::PathBuf};

/// Compares the two documents `args` selects, prints the report, and says
/// whether the run should fail.
pub fn run(args: &DiffArgs) -> Result<Status, String> {
    let report = compare(args)?;
    println!("{}", render(&report, args.format)?);

    if report.is_failure(&args.fail_on()) {
        Ok(Status::Differences)
    } else {
        Ok(Status::Clean)
    }
}

/// Resolves both sides, loads them, and diffs them.
fn compare(args: &DiffArgs) -> Result<DiffReport, String> {
    let directory = working_directory()?;
    let baseline_path = discovery::resolve(args.from.as_deref(), &BASELINE, &directory)?;
    let candidate_path = discovery::resolve(args.to.as_deref(), &CANDIDATE, &directory)?;

    let baseline = source::load_document(&baseline_path)?;
    let candidate = source::load_document(&candidate_path)?;

    Ok(diff_documents(&baseline, &candidate, &args.options()))
}

/// Renders the report in the requested format.
fn render(report: &DiffReport, format: Format) -> Result<String, String> {
    match format {
        Format::Human => Ok(report.to_string()),
        Format::Json => serde_json::to_string_pretty(&report.to_json())
            .map_err(|error| format!("failed to serialise the report: {error}")),
    }
}

/// The directory a discovery walk starts from.
fn working_directory() -> Result<PathBuf, String> {
    env::current_dir().map_err(|error| format!("failed to read the working directory: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lucyd_core::openapi::diff::{EndpointRef, FailOn};

    /// A report with one missing endpoint and nothing else.
    fn report_with_one_missing_endpoint() -> DiffReport {
        DiffReport {
            missing: vec![EndpointRef {
                method: "GET".to_string(),
                path: "/api/users".to_string(),
            }],
            ..DiffReport::default()
        }
    }

    #[test]
    fn the_human_rendering_is_the_librarys_own() {
        let report = report_with_one_missing_endpoint();

        let rendered = render(&report, Format::Human).expect("the human format never fails");

        assert_eq!(
            rendered,
            report.to_string(),
            "the CLI must not reformat what the library already renders"
        );
    }

    #[test]
    fn the_json_rendering_is_indented_and_parses_back() {
        let report = report_with_one_missing_endpoint();

        let rendered = render(&report, Format::Json).expect("a report always serialises");

        assert!(rendered.contains('\n'), "CI logs deserve readable JSON");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rendered).expect("valid JSON"),
            report.to_json()
        );
    }

    #[test]
    fn a_clean_report_is_not_a_failure_whatever_is_selected() {
        let clean = DiffReport::default();

        assert!(!clean.is_failure(&FailOn {
            missing: true,
            added: true,
            changed: true
        }));
    }
}
