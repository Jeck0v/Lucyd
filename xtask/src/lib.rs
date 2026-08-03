//! Lucyd build automation scripts.
//!
//! This crate backs the `xtask` binary (`src/main.rs`, a thin `env::args()`
//! dispatcher) with a testable library. Keeping the logic here rather than in
//! `main.rs` lets `cargo test -p xtask` exercise `build_ui`'s argument
//! handling and the whole `import_openapi` pipeline without spawning a
//! subprocess.
//!
//! Available commands:
//!   build-ui          Installs npm dependencies and compiles the React
//!                     frontend into `ui/dist/` for embedding in lucyd-core.
//!   build-docs        Rebuilds `docs.md` from the numbered pages in `docs/`.
//!   import-openapi    Reads an OpenAPI 3.x document and generates/merges
//!                     Rust scaffolding (structs + `#[lucyd_http]` stubs).

mod build_docs;
pub mod import_openapi;

pub use build_docs::build_docs;

use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

/// Directory containing the React/Vite frontend, relative to workspace root.
const UI_DIR: &str = "ui";

/// Output directory for the built UI assets (inside lucyd-core for crates.io packaging).
const UI_DIST_DIR: &str = "crates/lucyd-core/ui/dist";

/// The package manager binary used to install and build the frontend.
const BUILD_CMD: &str = "npm";

/// Argument to `npm` for installing dependencies.
const NPM_INSTALL_ARGS: &[&str] = &["install"];

/// Arguments to `npm` for building the frontend.
const NPM_BUILD_ARGS: &[&str] = &["run", "build"];

/// File the built bundle must contain for `lucyd-core` to serve anything.
///
/// `rust-embed` embeds whatever `UI_DIST_DIR` holds at compile time, including
/// nothing at all: an empty directory compiles fine and yields a binary that
/// answers `404 UI not built` on every `/docs` request. npm's exit code only
/// says the bundler ran, not that its output landed where lucyd-core reads it
/// from, so the two have to be checked separately.
const UI_ENTRY_FILE: &str = "index.html";

/// Compiles the React frontend.
///
/// Steps:
/// 1. Run `npm install` inside `ui/`
/// 2. Run `npm run build` inside `ui/`
/// 3. Verify the bundle landed in `UI_DIST_DIR`
/// 4. Print success message with output path
pub fn build_ui(workspace_root: &Path) -> Result<(), String> {
    let ui_dir = workspace_root.join(UI_DIR);

    if !ui_dir.exists() {
        return Err(format!(
            "UI directory not found at `{}`. \
             Make sure `{UI_DIR}/` exists before running build-ui.",
            ui_dir.display()
        ));
    }

    println!(
        "==> Installing npm dependencies in `{}`...",
        ui_dir.display()
    );
    run_command(BUILD_CMD, NPM_INSTALL_ARGS, &ui_dir)?;

    println!("==> Building React frontend...");
    run_command(BUILD_CMD, NPM_BUILD_ARGS, &ui_dir)?;

    let dist_dir = workspace_root.join(UI_DIST_DIR);
    verify_bundle(&dist_dir)?;

    println!(
        "==> UI built successfully. Output: `{}`",
        dist_dir.display()
    );
    Ok(())
}

/// Confirms the built bundle landed where `lucyd-core` embeds it from.
///
/// The two paths involved are configured in different files — `build.outDir` in
/// `ui/vite.config.ts` and `#[folder]` in `lucyd-core/src/assets.rs` — so
/// nothing but this check couples them. Without it, a drift between the two
/// makes `build-ui` report success while producing a binary with no UI in it,
/// and the first sign of trouble is a `404` in a browser after release.
fn verify_bundle(dist_dir: &Path) -> Result<(), String> {
    if dist_dir.join(UI_ENTRY_FILE).is_file() {
        return Ok(());
    }
    Err(format!(
        "the frontend build reported success but `{}` holds no `{UI_ENTRY_FILE}`.\n  \
         `lucyd-core` embeds that directory verbatim, so shipping this build would \
         produce a binary that answers `404 UI not built` on every /docs request.\n  \
         Check `build.outDir` in `{UI_DIR}/vite.config.ts`: it must resolve to that path.",
        dist_dir.display()
    ))
}

/// Runs an external command in the given working directory, inheriting stdio.
///
/// Returns an error if the command fails to launch or exits with a non-zero code.
pub fn run_command(program: &str, args: &[&str], cwd: &Path) -> Result<(), String> {
    let status: ExitStatus = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .status()
        .map_err(|e| format!("Failed to launch `{program}`: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "`{program} {}` exited with status {status}",
            args.join(" ")
        ))
    }
}

/// Prints usage help to stdout.
pub fn print_usage() {
    println!("Usage: cargo xtask <command>");
    println!();
    println!("Commands:");
    println!(
        "  build-ui                                    Compile the React frontend into ui/dist/"
    );
    println!(
        "  build-docs                                  Rebuild docs.md from the pages in docs/"
    );
    println!(
        "  import-openapi <file> [--out <path>] [--remove-orphaned]   Generate Rust scaffolding from an OpenAPI 3.x document"
    );
    println!();
    println!("import-openapi options:");
    println!("  --out <path>       Output file (default: src/generated_endpoints.rs)");
    println!("  --remove-orphaned  Physically delete handlers/structs no longer present in <file>");
    println!("                     (default: report only, leave the code in place)");
}

/// Resolves the workspace root directory.
///
/// The xtask binary is compiled into `target/`, which lives at the workspace root.
/// We use the `CARGO_MANIFEST_DIR` env var (set by Cargo at build time) to locate
/// the xtask crate, then walk up one level to the workspace root.
pub fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR points to xtask/, so parent() is the workspace root.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir)
        .parent()
        .expect("xtask must be inside a workspace directory")
        .to_path_buf()
}

/// Parsed command-line arguments for `cargo xtask import-openapi`.
#[derive(Debug)]
pub struct ImportOpenApiArgs {
    /// Path to the input OpenAPI 3.x document (JSON or YAML).
    pub input: PathBuf,
    /// Path to the generated/merged Rust file.
    pub out: PathBuf,
    /// Whether to physically delete orphaned handlers/structs rather than
    /// only reporting them.
    pub remove_orphaned: bool,
}

/// Default value of `--out` when not supplied.
const DEFAULT_OUT: &str = "src/generated_endpoints.rs";

/// Parses `cargo xtask import-openapi <file> [--out <path>] [--remove-orphaned]`.
///
/// `args` is the argument list *after* the `import-openapi` command word.
pub fn parse_import_openapi_args(args: &[String]) -> Result<ImportOpenApiArgs, String> {
    let mut input = None;
    let mut out = None;
    let mut remove_orphaned = false;

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--out" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "missing value for `--out`".to_string())?;
                out = Some(PathBuf::from(value));
            }
            "--remove-orphaned" => remove_orphaned = true,
            other if input.is_none() => input = Some(PathBuf::from(other)),
            other => return Err(format!("unexpected argument `{other}`")),
        }
    }

    let input = input.ok_or_else(|| "missing required <file> argument".to_string())?;
    let out = out.unwrap_or_else(|| PathBuf::from(DEFAULT_OUT));

    Ok(ImportOpenApiArgs {
        input,
        out,
        remove_orphaned,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const FIXTURE_INPUT: &str = "openapi.yaml";
    const FIXTURE_OUT: &str = "src/custom_out.rs";

    #[test]
    fn a_bundle_carrying_the_entry_file_passes_verification() {
        let dist = TempDir::new().expect("a temporary directory must be available");
        fs::write(dist.path().join(UI_ENTRY_FILE), "<!doctype html>")
            .expect("the entry file must be writable");

        assert_eq!(verify_bundle(dist.path()), Ok(()));
    }

    #[test]
    fn an_empty_dist_directory_fails_verification() {
        let dist = TempDir::new().expect("a temporary directory must be available");

        let error = verify_bundle(dist.path())
            .expect_err("an empty bundle must not be reported as a successful build");

        assert!(error.contains(UI_ENTRY_FILE), "{error}");
        assert!(
            error.contains("outDir"),
            "the message must name the setting that fixes it, got: {error}"
        );
    }

    #[test]
    fn a_missing_dist_directory_fails_verification() {
        let root = TempDir::new().expect("a temporary directory must be available");

        assert!(
            verify_bundle(&root.path().join("never-created")).is_err(),
            "a build that produced no output directory at all must be an error"
        );
    }

    #[test]
    fn parse_minimal_args_uses_default_out() {
        let args = vec![FIXTURE_INPUT.to_string()];
        let parsed = parse_import_openapi_args(&args).expect("parsing must succeed");

        assert_eq!(parsed.input, PathBuf::from(FIXTURE_INPUT));
        assert_eq!(parsed.out, PathBuf::from(DEFAULT_OUT));
        assert!(!parsed.remove_orphaned);
    }

    #[test]
    fn parse_all_options() {
        let args = vec![
            FIXTURE_INPUT.to_string(),
            "--out".to_string(),
            FIXTURE_OUT.to_string(),
            "--remove-orphaned".to_string(),
        ];
        let parsed = parse_import_openapi_args(&args).expect("parsing must succeed");

        assert_eq!(parsed.input, PathBuf::from(FIXTURE_INPUT));
        assert_eq!(parsed.out, PathBuf::from(FIXTURE_OUT));
        assert!(parsed.remove_orphaned);
    }

    #[test]
    fn missing_file_argument_is_an_error() {
        let args: Vec<String> = vec![];
        let err = parse_import_openapi_args(&args).expect_err("must reject missing <file>");
        assert!(err.contains("missing required"));
    }

    #[test]
    fn missing_out_value_is_an_error() {
        let args = vec![FIXTURE_INPUT.to_string(), "--out".to_string()];
        let err = parse_import_openapi_args(&args).expect_err("must reject dangling --out");
        assert!(err.contains("--out"));
    }
}
