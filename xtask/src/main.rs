//! Lucy build automation scripts.
//!
//! Usage: `cargo xtask <command>`
//!
//! Available commands:
//!   build-ui   — Installs npm dependencies and compiles the React frontend
//!                into `ui/dist/` for embedding in lucy-core.

use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

/// Directory containing the React/Vite frontend, relative to workspace root.
const UI_DIR: &str = "ui";

/// The package manager binary used to install and build the frontend.
const BUILD_CMD: &str = "npm";

/// Argument to `npm` for installing dependencies.
const NPM_INSTALL_ARGS: &[&str] = &["install"];

/// Arguments to `npm` for building the frontend.
const NPM_BUILD_ARGS: &[&str] = &["run", "build"];

fn main() {
    // Resolve workspace root as the directory containing this xtask crate's parent
    let workspace_root = workspace_root();

    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str);

    match command {
        Some("build-ui") => {
            if let Err(e) = build_ui(&workspace_root) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Some(unknown) => {
            eprintln!("Unknown command: {unknown}");
            print_usage();
            std::process::exit(1);
        }
        None => {
            print_usage();
        }
    }
}

/// Compiles the React frontend.
///
/// Steps:
/// 1. Run `npm install` inside `ui/`
/// 2. Run `npm run build` inside `ui/`
/// 3. Print success message with output path
fn build_ui(workspace_root: &Path) -> Result<(), String> {
    let ui_dir = workspace_root.join(UI_DIR);

    if !ui_dir.exists() {
        return Err(format!(
            "UI directory not found at `{}`. \
             Make sure `{UI_DIR}/` exists before running build-ui.",
            ui_dir.display()
        ));
    }

    println!("==> Installing npm dependencies in `{}`...", ui_dir.display());
    run_command(BUILD_CMD, NPM_INSTALL_ARGS, &ui_dir)?;

    println!("==> Building React frontend...");
    run_command(BUILD_CMD, NPM_BUILD_ARGS, &ui_dir)?;

    println!(
        "==> UI built successfully. Output: `{}`",
        ui_dir.join("dist").display()
    );
    Ok(())
}

/// Runs an external command in the given working directory, inheriting stdio.
///
/// Returns an error if the command fails to launch or exits with a non-zero code.
fn run_command(program: &str, args: &[&str], cwd: &Path) -> Result<(), String> {
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
fn print_usage() {
    println!("Usage: cargo xtask <command>");
    println!();
    println!("Commands:");
    println!("  build-ui   Compile the React frontend into ui/dist/");
}

/// Resolves the workspace root directory.
///
/// The xtask binary is compiled into `target/`, which lives at the workspace root.
/// We use the `CARGO_MANIFEST_DIR` env var (set by Cargo at build time) to locate
/// the xtask crate, then walk up one level to the workspace root.
fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR points to xtask/, so parent() is the workspace root.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir)
        .parent()
        .expect("xtask must be inside a workspace directory")
        .to_path_buf()
}
