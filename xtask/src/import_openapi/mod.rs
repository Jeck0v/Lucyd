//! `cargo xtask import-openapi <file>`: reads an OpenAPI 3.x document and
//! generates Rust scaffolding (structs + `#[lucy_http]` handler stubs with
//! `todo!()` bodies) that bootstraps a Lucyd project from it.
//!
//! This is the reverse of `crates/lucy-core/src/openapi/`, which turns a
//! running application's registered endpoints *into* an OpenAPI document;
//! this module turns an OpenAPI document *into* the Rust code that would
//! register those endpoints. Re-running it against an updated spec merges
//! into the existing output file rather than overwriting it wholesale — see
//! `merge.rs` for how handwritten handler bodies survive that.
//!
//! Pipeline: [`document::load_document`] -> [`operations::extract_operations`]
//! -> [`codegen::generate`] -> [`merge::merge`] -> written to disk, then
//! [`summary::print_summary`].

mod codegen;
mod document;
mod merge;
mod operations;
mod rust_type;
mod summary;

use std::{fs, path::Path};

/// Runs the full import pipeline: load, extract, generate, merge, write,
/// report. `out_path` is created (along with any missing parent
/// directories) if it doesn't exist yet.
pub fn run(input_path: &Path, out_path: &Path, remove_orphaned: bool) -> Result<(), String> {
    let document = document::load_document(input_path)?;
    let operations = operations::extract_operations(&document);

    let generated = codegen::generate(&document, &operations)?;

    let existing_source = if out_path.exists() {
        Some(
            fs::read_to_string(out_path)
                .map_err(|e| format!("failed to read '{}': {e}", out_path.display()))?,
        )
    } else {
        None
    };

    let outcome = merge::merge(existing_source.as_deref(), generated, remove_orphaned)?;

    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create '{}': {e}", parent.display()))?;
        }
    }

    let formatted = prettyplease::unparse(&outcome.file);
    fs::write(out_path, formatted)
        .map_err(|e| format!("failed to write '{}': {e}", out_path.display()))?;

    let skipped: Vec<&operations::ImportOperation> = operations
        .iter()
        .filter(|op| op.skip_reason.is_some())
        .collect();
    summary::print_summary(&outcome.report, &skipped);

    Ok(())
}
