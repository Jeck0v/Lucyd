//! Finds the documents `lucyd diff` compares when the flags are omitted.
//!
//! A migration is checked over and over while it progresses, so the common
//! invocation is a bare `lucyd diff` from inside the project. Both sides are
//! looked up by convention, independently, which also makes naming only one
//! of them work without any extra handling.
//!
//! The search walks upwards, from the working directory to the project root,
//! the way `cargo` and `git` resolve their own files. Walking downwards would
//! be both slower and surprising: it would happily pick a fixture out of
//! `tests/` or a vendored specification out of `target/`.

use std::path::{Path, PathBuf};

/// File whose presence marks the directory an upward search stops at.
const PROJECT_MARKER: &str = "Cargo.toml";

/// A document the CLI can find on its own, and what to say when it can't.
pub struct Convention {
    /// What this document is, as named in an error message.
    role: &'static str,
    /// The flag that names it explicitly.
    flag: &'static str,
    /// Accepted file names, most conventional first.
    filenames: &'static [&'static str],
    /// How to obtain the document, appended to a failed search.
    hint: &'static str,
}

/// The pre-existing specification a migration must not regress against.
pub const BASELINE: Convention = Convention {
    role: "baseline specification",
    flag: "--from",
    filenames: &["openapi.json", "openapi.yaml", "openapi.yml"],
    hint: "This is the document you are migrating away from.",
};

/// The document a Lucyd application serves at `/docs/openapi.json`, saved to
/// disk.
///
/// Only JSON is listed because that endpoint only ever serves JSON.
pub const CANDIDATE: Convention = Convention {
    role: "Lucyd export",
    flag: "--to",
    filenames: &["lucyd-openapi.json"],
    hint: "Export it from your running application first:\n    \
           curl -o lucyd-openapi.json http://localhost:3000/docs/openapi.json",
};

/// Returns the explicitly named path, or the one `convention` finds from
/// `start`.
pub fn resolve(
    explicit: Option<&Path>,
    convention: &Convention,
    start: &Path,
) -> Result<PathBuf, String> {
    match explicit {
        Some(path) => Ok(path.to_path_buf()),
        None => search(convention, start).ok_or_else(|| not_found(convention, start)),
    }
}

/// Looks for one of `convention`'s file names in `start`, then in each of its
/// parents, up to and including the directory holding the project manifest.
fn search(convention: &Convention, start: &Path) -> Option<PathBuf> {
    for directory in start.ancestors() {
        if let Some(found) = first_match(convention, directory) {
            return Some(found);
        }
        if directory.join(PROJECT_MARKER).is_file() {
            break;
        }
    }
    None
}

/// The first of `convention`'s file names that exists in `directory`.
fn first_match(convention: &Convention, directory: &Path) -> Option<PathBuf> {
    convention
        .filenames
        .iter()
        .map(|filename| directory.join(filename))
        .find(|candidate| candidate.is_file())
}

/// Explains what was looked for, where, and the two ways to supply it.
///
/// A discovery failure is the most likely way a bare `lucyd diff` ends, so it
/// is worth more than one line.
fn not_found(convention: &Convention, start: &Path) -> String {
    format!(
        "no {} found.\n  \
         Looked for {} in '{}' and its parent directories, up to the project root.\n  \
         {}\n  \
         Or name it explicitly with `{} <path>`.",
        convention.role,
        convention.filenames.join(", "),
        start.display(),
        convention.hint,
        convention.flag,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Builds a project directory tree, creating every named file empty.
    ///
    /// Paths are relative and slash-separated, e.g. `crates/api/openapi.json`.
    fn project(files: &[&str]) -> TempDir {
        let root = TempDir::new().expect("a temporary directory must be available");
        for file in files {
            let path = root.path().join(file);
            fs::create_dir_all(path.parent().expect("a file has a parent"))
                .expect("the parent directory must be creatable");
            fs::write(&path, "").expect("the file must be writable");
        }
        root
    }

    #[test]
    fn an_explicit_path_is_used_as_is_and_never_searched_for() {
        let root = project(&[]);
        let named = Path::new("somewhere/else/spec.json");

        let resolved = resolve(Some(named), &BASELINE, root.path());

        assert_eq!(resolved, Ok(named.to_path_buf()));
    }

    #[test]
    fn a_baseline_in_the_working_directory_is_found() {
        let root = project(&["openapi.json"]);

        let resolved = resolve(None, &BASELINE, root.path());

        assert_eq!(resolved, Ok(root.path().join("openapi.json")));
    }

    #[test]
    fn json_wins_over_yaml_when_both_are_present() {
        let root = project(&["openapi.json", "openapi.yaml"]);

        let resolved = resolve(None, &BASELINE, root.path());

        assert_eq!(
            resolved,
            Ok(root.path().join("openapi.json")),
            "the order of the accepted names must decide, not the filesystem"
        );
    }

    #[test]
    fn a_yaml_baseline_is_found_when_there_is_no_json_one() {
        let root = project(&["openapi.yml"]);

        let resolved = resolve(None, &BASELINE, root.path());

        assert_eq!(resolved, Ok(root.path().join("openapi.yml")));
    }

    #[test]
    fn a_document_at_the_project_root_is_found_from_a_subdirectory() {
        let root = project(&["Cargo.toml", "openapi.json", "crates/api/src/lib.rs"]);
        let deep = root.path().join("crates/api/src");

        let resolved = resolve(None, &BASELINE, &deep);

        assert_eq!(
            resolved,
            Ok(root.path().join("openapi.json")),
            "a bare `lucyd diff` must work from anywhere inside the project"
        );
    }

    #[test]
    fn the_nearest_document_wins_over_one_further_up() {
        let root = project(&["Cargo.toml", "openapi.json", "crates/api/openapi.json"]);
        let nested = root.path().join("crates/api");

        let resolved = resolve(None, &BASELINE, &nested);

        assert_eq!(resolved, Ok(nested.join("openapi.json")));
    }

    #[test]
    fn the_search_stops_at_the_project_root() {
        let root = project(&["openapi.json", "workspace/Cargo.toml"]);
        let inner = root.path().join("workspace");

        let resolved = resolve(None, &BASELINE, &inner);

        assert!(
            resolved.is_err(),
            "a document outside the project must not be picked up by accident"
        );
    }

    #[test]
    fn the_lucyd_export_has_its_own_conventional_name() {
        let root = project(&["openapi.json", "lucyd-openapi.json"]);

        assert_eq!(
            resolve(None, &CANDIDATE, root.path()),
            Ok(root.path().join("lucyd-openapi.json")),
            "the two sides must never resolve to the same file"
        );
    }

    #[test]
    fn a_failed_search_says_what_was_looked_for_and_how_to_supply_it() {
        let root = project(&["Cargo.toml"]);

        let error = resolve(None, &CANDIDATE, root.path()).expect_err("nothing to find");

        assert!(error.contains("Lucyd export"), "{error}");
        assert!(error.contains("lucyd-openapi.json"), "{error}");
        assert!(error.contains("curl -o"), "{error}");
        assert!(error.contains("--to"), "{error}");
    }
}
