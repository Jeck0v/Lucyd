//! The `lucyd` command-line tool.
//!
//! Front-end to the OpenAPI diff implemented in
//! [`lucyd_core::openapi::diff`]. Nothing here decides what counts as a
//! difference or how a report reads: the library owns both, so the two stay
//! consistent whether a report is produced by this binary or by a test.
//!
//! What this crate does own is everything the library deliberately doesn't:
//! finding the documents on disk, parsing YAML as well as JSON, and turning a
//! report into a process exit code a CI job can branch on.

mod cli;
mod diff;
mod discovery;
mod source;
mod update;

use clap::Parser;
use cli::{Cli, Command};

/// The two documents describe the same API, as far as the run was asked to
/// care.
const EXIT_CLEAN: i32 = 0;

/// The report contains a difference in a category `--fail-on` selected.
const EXIT_DIFFERENCES: i32 = 1;

/// The run could not happen: bad usage, unreadable file, unparsable document.
///
/// Distinct from [`EXIT_DIFFERENCES`] so a pipeline can tell "the contract
/// regressed" apart from "the tool was pointed at the wrong path".
const EXIT_ERROR: i32 = 2;

/// What a command produced, in the only terms the exit code cares about.
pub enum Status {
    /// Nothing worth failing the build over.
    Clean,
    /// A difference the caller asked to be failed on.
    Differences,
}

fn main() {
    std::process::exit(run());
}

/// Runs the requested command and reports the process exit code.
///
/// Split out of `main` so every path returns a value rather than calling
/// `exit` from somewhere in the middle of the program.
fn run() -> i32 {
    match Cli::parse().command() {
        Ok(Some(command)) => execute(command),
        // Nothing was asked for; clap has already printed the help screen.
        Ok(None) => EXIT_ERROR,
        Err(message) => fail(message),
    }
}

/// Runs one command and maps what it produced onto an exit code.
fn execute(command: Command) -> i32 {
    let outcome = match command {
        Command::Diff(args) => diff::run(&args),
        Command::Update => update::run().map(|()| Status::Clean),
    };

    match outcome {
        Ok(Status::Clean) => EXIT_CLEAN,
        Ok(Status::Differences) => EXIT_DIFFERENCES,
        Err(message) => fail(message),
    }
}

/// Reports a failure the run could not recover from.
fn fail(message: String) -> i32 {
    eprintln!("error: {message}");
    EXIT_ERROR
}
