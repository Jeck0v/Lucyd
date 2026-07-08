//! Lucy build automation scripts.
//!
//! This crate backs the `xtask` binary (`src/main.rs`, a thin `env::args()`
//! dispatcher) with a testable library. Keeping the logic here rather than in
//! `main.rs` lets `cargo test -p xtask` exercise `build_ui`'s argument
//! handling and the whole `import_openapi` pipeline without spawning a
//! subprocess.
//!
//! Available commands:
//!   build-ui         — Installs npm dependencies and compiles the React
//!                       frontend into `ui/dist/` for embedding in lucy-core.
//!   import-openapi    — Reads an OpenAPI 3.x document and generates/merges
//!                       Rust scaffolding (structs + `#[lucy_http]` stubs).

pub mod import_openapi;

use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

/// Directory containing the React/Vite frontend, relative to workspace root.
const UI_DIR: &str = "ui";

/// Output directory for the built UI assets (inside lucy-core for crates.io packaging).
const UI_DIST_DIR: &str = "crates/lucy-core/ui/dist";

/// The package manager binary used to install and build the frontend.
const BUILD_CMD: &str = "npm";

/// Argument to `npm` for installing dependencies.
const NPM_INSTALL_ARGS: &[&str] = &["install"];

/// Arguments to `npm` for building the frontend.
const NPM_BUILD_ARGS: &[&str] = &["run", "build"];

/// Compiles the React frontend.
///
/// Steps:
/// 1. Run `npm install` inside `ui/`
/// 2. Run `npm run build` inside `ui/`
/// 3. Print success message with output path
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

    println!(
        "==> UI built successfully. Output: `{}`",
        workspace_root.join(UI_DIST_DIR).display()
    );
    Ok(())
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

    const FIXTURE_INPUT: &str = "openapi.yaml";
    const FIXTURE_OUT: &str = "src/custom_out.rs";

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
