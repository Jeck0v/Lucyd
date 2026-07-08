//! Thin CLI dispatcher over `xtask`'s library.
//!
//! All actual logic lives in `src/lib.rs` and `src/import_openapi/` so it can
//! be exercised by `cargo test -p xtask` without spawning this binary as a
//! subprocess. This file only parses `env::args()`, calls into the library,
//! and translates the `Result` into a process exit code.

use std::env;
use xtask::import_openapi;

fn main() {
    let workspace_root = xtask::workspace_root();

    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str);

    let result = match command {
        Some("build-ui") => xtask::build_ui(&workspace_root),
        Some("import-openapi") => run_import_openapi(&args[1..]),
        Some(unknown) => {
            eprintln!("Unknown command: {unknown}");
            xtask::print_usage();
            std::process::exit(1);
        }
        None => {
            xtask::print_usage();
            return;
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

/// Parses `import-openapi` arguments and runs the import pipeline.
fn run_import_openapi(args: &[String]) -> Result<(), String> {
    let parsed = xtask::parse_import_openapi_args(args)?;
    import_openapi::run(&parsed.input, &parsed.out, parsed.remove_orphaned)
}
