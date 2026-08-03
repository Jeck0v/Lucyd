//! The command-line surface, and nothing else.
//!
//! Every type here exists to turn strings typed in a terminal into the types
//! [`lucyd_core::openapi::diff`] already defines. Keeping that translation in
//! one module is what lets the rest of the crate speak the library's
//! vocabulary instead of clap's.

use clap::{Args, Parser, Subcommand, ValueEnum};
use lucyd_core::openapi::diff::{DiffOptions, FailOn};
use std::path::PathBuf;

/// The `lucyd` binary.
#[derive(Debug, Parser)]
#[command(
    name = "lucyd",
    // Pinned rather than taken from argv[0], which would read `lucyd.exe` on
    // Windows and make the help screen platform-dependent.
    bin_name = "lucyd",
    version,
    about = "Validate an OpenAPI contract against the document a Lucyd application generates",
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Reinstall lucyd from crates.io at its latest published version.
    #[arg(long, exclusive = true)]
    update: bool,
}

/// Refusal shown when `--update` is combined with a subcommand.
///
/// clap's own `exclusive` only guards a flag against other *arguments*, not
/// against a subcommand, so this one rule is enforced here rather than
/// silently letting one of the two requests win.
const UPDATE_STANDS_ALONE: &str =
    "`--update` reinstalls the binary and runs nothing else; use one or the other.";

impl Cli {
    /// Resolves the flag-or-subcommand surface into the single command to run.
    ///
    /// `Ok(None)` means nothing was asked for, which `arg_required_else_help`
    /// has already answered with the help screen.
    pub fn command(self) -> Result<Option<Command>, String> {
        match (self.update, self.command) {
            (true, Some(_)) => Err(UPDATE_STANDS_ALONE.to_string()),
            (true, None) => Ok(Some(Command::Update)),
            (false, command) => Ok(command),
        }
    }
}

/// What the binary was asked to do.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Compare two OpenAPI documents and report what the API lost, gained or
    /// changed between them.
    Diff(DiffArgs),

    /// Reinstall lucyd from crates.io at its latest published version.
    ///
    /// Hidden because `--update` is the documented spelling; accepting this
    /// one too costs nothing and saves anyone who guesses it.
    #[command(hide = true)]
    Update,
}

/// Arguments of `lucyd diff`.
#[derive(Debug, Args)]
pub struct DiffArgs {
    /// Baseline document: the contract being protected.
    ///
    /// Discovered from the project when omitted.
    #[arg(long, value_name = "PATH")]
    pub from: Option<PathBuf>,

    /// Candidate document: what is checked against the baseline.
    ///
    /// Discovered from the project when omitted.
    #[arg(long, value_name = "PATH")]
    pub to: Option<PathBuf>,

    /// How the report is rendered.
    #[arg(long, value_enum, default_value_t = Format::Human)]
    pub format: Format,

    /// Differences that make the command exit with code 1
    /// [default: missing,changed].
    #[arg(long, value_enum, value_delimiter = ',', value_name = "KIND")]
    pub fail_on: Option<Vec<Difference>>,

    /// Also compare the facets Lucyd's exporter cannot express: exact response
    /// status codes, path parameter types, tags, deprecation.
    #[arg(long)]
    pub strict: bool,
}

impl DiffArgs {
    /// The comparison profile these flags select.
    pub fn options(&self) -> DiffOptions {
        DiffOptions {
            strict: self.strict,
        }
    }

    /// The categories of difference that make this run a failure.
    ///
    /// Omitting `--fail-on` defers to the library's own default rather than
    /// restating it here, so the recommended CI setting is defined once.
    pub fn fail_on(&self) -> FailOn {
        let Some(selected) = &self.fail_on else {
            return FailOn::default();
        };
        FailOn {
            missing: selected.contains(&Difference::Missing),
            added: selected.contains(&Difference::Added),
            changed: selected.contains(&Difference::Changed),
        }
    }
}

/// How a report is written to stdout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// Indented sections, for a person reading a terminal.
    Human,
    /// The machine-readable envelope, for a CI job parsing the output.
    Json,
}

/// A category of difference, as named on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Difference {
    /// An endpoint the candidate document no longer declares.
    Missing,
    /// An endpoint only the candidate document declares.
    Added,
    /// An endpoint both declare, with a different contract.
    Changed,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parses a `lucyd diff` invocation, given the arguments after `diff`.
    fn diff_args(arguments: &[&str]) -> DiffArgs {
        let mut line = vec!["lucyd", "diff"];
        line.extend_from_slice(arguments);

        match Cli::try_parse_from(line)
            .expect("the invocation must parse")
            .command()
        {
            Ok(Some(Command::Diff(args))) => args,
            other => panic!("expected a diff command, got {other:?}"),
        }
    }

    #[test]
    fn a_bare_diff_discovers_both_documents_and_uses_the_library_default() {
        let args = diff_args(&[]);

        assert!(args.from.is_none(), "an omitted --from must be discovered");
        assert!(args.to.is_none(), "an omitted --to must be discovered");
        assert_eq!(args.format, Format::Human);
        assert!(!args.strict);
        assert_eq!(
            args.fail_on(),
            FailOn::default(),
            "the CLI must not restate the recommended CI setting"
        );
    }

    #[test]
    fn both_documents_can_be_named_explicitly() {
        let args = diff_args(&["--from", "spec.yaml", "--to", "export.json"]);

        assert_eq!(args.from, Some(PathBuf::from("spec.yaml")));
        assert_eq!(args.to, Some(PathBuf::from("export.json")));
    }

    #[test]
    fn fail_on_is_a_comma_separated_list() {
        let args = diff_args(&["--fail-on", "missing,added,changed"]);

        assert_eq!(
            args.fail_on(),
            FailOn {
                missing: true,
                added: true,
                changed: true
            }
        );
    }

    #[test]
    fn fail_on_replaces_the_default_rather_than_extending_it() {
        let args = diff_args(&["--fail-on", "added"]);

        assert_eq!(
            args.fail_on(),
            FailOn {
                missing: false,
                added: true,
                changed: false
            },
            "naming one category must not silently keep the default ones"
        );
    }

    #[test]
    fn an_unknown_fail_on_category_is_rejected() {
        let error = Cli::try_parse_from(["lucyd", "diff", "--fail-on", "renamed"])
            .expect_err("an unknown category must not be accepted silently");

        assert!(error.to_string().contains("renamed"));
    }

    #[test]
    fn strict_selects_the_literal_comparison() {
        assert!(diff_args(&["--strict"]).options().strict);
        assert!(!diff_args(&[]).options().strict);
    }

    #[test]
    fn the_json_format_is_selectable() {
        assert_eq!(diff_args(&["--format", "json"]).format, Format::Json);
    }

    #[test]
    fn update_is_a_command_of_its_own() {
        let cli = Cli::try_parse_from(["lucyd", "--update"]).expect("--update must parse");

        assert!(matches!(cli.command(), Ok(Some(Command::Update))));
    }

    #[test]
    fn update_cannot_be_combined_with_a_diff() {
        let cli = Cli::try_parse_from(["lucyd", "--update", "diff"]).expect("clap accepts both");

        assert!(
            matches!(cli.command(), Err(message) if message == UPDATE_STANDS_ALONE),
            "one of the two requests must not be silently dropped"
        );
    }

    #[test]
    fn a_bare_invocation_asks_for_nothing() {
        Cli::try_parse_from(["lucyd"]).expect_err("a bare `lucyd` must show the help screen");
    }
}
