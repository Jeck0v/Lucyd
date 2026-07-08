//! The incremental-merge engine: reconciles freshly generated code
//! (`codegen::GeneratedFile`) against whatever already exists at the output
//! path, so re-running the importer doesn't clobber handwritten handler
//! bodies.
//!
//! Matching is keyed on the `/// operationId: {id}` doc-comment marker
//! emitted by `codegen.rs::doc_comment_attrs`, not the function's name: a
//! plain `//` comment isn't a token in Rust's grammar and `syn::parse_file`
//! silently drops it, so it couldn't survive a parse -> merge -> re-emit
//! round trip. A `///` doc comment desugars to a real `#[doc = "..."]`
//! attribute that `syn` preserves, which lets a developer rename a stub by
//! hand without a subsequent import reporting a spurious Removed+Added pair
//! for what is still the same operation.
//!
//! Struct/enum definitions carry no such marker and no handwritten-logic
//! risk, so — unlike fn bodies — they are always fully regenerated from the
//! current spec; only their *presence* (kept vs. deleted when orphaned) is
//! subject to the merge decision.
//!
//! Any item this module doesn't specifically manage (a hand-added `impl`
//! block, an unmarked helper fn, a `use` a developer wrote below the fixed
//! header, ...) is preserved verbatim and is never a candidate for removal,
//! `--remove-orphaned` or not — this tool only ever deletes code it can
//! prove it generated itself. One exception: a *top-level* `use` identical
//! in kind to the fixed header's own `use`s is not specifically tracked
//! back across runs (the header is always re-emitted fresh), so hand-added
//! top-level `use` statements are not preserved by this pass. See
//! `docs/11-limitations.md` for that trade-off.

use super::codegen::{self, GeneratedFile};
use quote::ToTokens;
use std::collections::{HashMap, HashSet};
use syn::Item;

/// Operation ids that changed, bucketed by what happened. `Unchanged`
/// operations are deliberately not tracked here — they carry nothing worth
/// reporting in `summary.rs`.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct MergeReport {
    pub added: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
}

/// The result of a merge: the assembled file (ready for
/// `prettyplease::unparse`) plus the report to hand to `summary.rs`.
pub struct MergeOutcome {
    pub file: syn::File,
    pub report: MergeReport,
}

/// Merges `generated` into whatever is at `existing_source` (`None` when the
/// output file doesn't exist yet, in which case everything is `Added`).
pub fn merge(
    existing_source: Option<&str>,
    generated: GeneratedFile,
    remove_orphaned: bool,
) -> Result<MergeOutcome, String> {
    let Some(existing_source) = existing_source else {
        return Ok(fresh_write(generated));
    };

    let existing_file = syn::parse_file(existing_source)
        .map_err(|e| format!("failed to parse the existing file at the output path: {e}"))?;

    let mut existing_fns: HashMap<String, syn::ItemFn> = HashMap::new();
    let mut existing_struct_items: Vec<Item> = Vec::new();
    let mut preserved_other: Vec<Item> = Vec::new();

    for item in existing_file.items {
        match item {
            Item::Fn(item_fn) => match operation_marker(&item_fn) {
                Some(id) => {
                    existing_fns.insert(id, item_fn);
                }
                None => preserved_other.push(Item::Fn(item_fn)),
            },
            Item::Struct(_) | Item::Enum(_) => existing_struct_items.push(item),
            // The fixed header always re-supplies its own `use`s; see the
            // module doc comment for why these aren't tracked across runs.
            Item::Use(_) => {}
            other => preserved_other.push(other),
        }
    }

    let mut report = MergeReport::default();
    let mut final_fns: Vec<Item> = Vec::new();

    for (id, fresh_fn) in generated.operation_fns {
        match existing_fns.remove(&id) {
            None => {
                report.added.push(id);
                final_fns.push(Item::Fn(fresh_fn));
            }
            Some(existing_fn) => {
                let merged_fn = merge_single_fn(&existing_fn, fresh_fn);
                if !fns_render_equal(&existing_fn, &merged_fn) {
                    report.updated.push(id);
                }
                final_fns.push(Item::Fn(merged_fn));
            }
        }
    }

    // Whatever's left in `existing_fns` didn't match any current-spec
    // operation. Always reported; only physically dropped when
    // `remove_orphaned` was requested.
    let mut removed_ids: Vec<String> = existing_fns.keys().cloned().collect();
    removed_ids.sort();
    report.removed = removed_ids.clone();

    if !remove_orphaned {
        for id in &removed_ids {
            if let Some(item_fn) = existing_fns.remove(id) {
                final_fns.push(Item::Fn(item_fn));
            }
        }
    }

    let new_struct_names: HashSet<String> =
        generated.structs.iter().filter_map(item_name).collect();
    let mut final_structs = generated.structs;
    if !remove_orphaned {
        for item in existing_struct_items {
            if let Some(name) = item_name(&item) {
                if !new_struct_names.contains(&name) {
                    final_structs.push(item);
                }
            }
        }
    }

    final_fns.extend(preserved_other);

    let file = codegen::assemble_file(
        generated.header_attrs,
        generated.header_items,
        final_structs,
        final_fns,
    );
    Ok(MergeOutcome { file, report })
}

/// The "no existing file" fast path: everything is `Added`.
fn fresh_write(generated: GeneratedFile) -> MergeOutcome {
    let mut report = MergeReport::default();
    let fn_items: Vec<Item> = generated
        .operation_fns
        .into_iter()
        .map(|(id, item_fn)| {
            report.added.push(id);
            Item::Fn(item_fn)
        })
        .collect();
    let file = codegen::assemble_file(
        generated.header_attrs,
        generated.header_items,
        generated.structs,
        fn_items,
    );
    MergeOutcome { file, report }
}

/// Combines a freshly generated fn with the previously existing one it was
/// matched against: keeps the fresh attributes/signature, but only replaces
/// the body when the existing one was exactly a bare `todo!(...)` call —
/// anything else is handwritten and preserved byte-for-byte.
fn merge_single_fn(existing: &syn::ItemFn, fresh: syn::ItemFn) -> syn::ItemFn {
    let mut merged = fresh;
    if !is_pure_todo_body(&existing.block) {
        merged.block = existing.block.clone();
    }
    merged
}

/// `true` when `block` is exactly a single `todo!(...)` macro call
/// statement — the shape every freshly generated stub has, and therefore
/// the only shape safe to overwrite without discarding real logic.
fn is_pure_todo_body(block: &syn::Block) -> bool {
    if block.stmts.len() != 1 {
        return false;
    }
    let mac = match &block.stmts[0] {
        syn::Stmt::Expr(syn::Expr::Macro(expr_macro), _) => &expr_macro.mac,
        syn::Stmt::Macro(stmt_macro) => &stmt_macro.mac,
        _ => return false,
    };
    mac.path.is_ident("todo")
}

/// Extracts the `operationId` from a fn's `/// operationId: {id}` doc
/// marker (the first matching `#[doc = "operationId: ..."]` attribute).
fn operation_marker(item_fn: &syn::ItemFn) -> Option<String> {
    for attr in &item_fn.attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        let syn::Meta::NameValue(name_value) = &attr.meta else {
            continue;
        };
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(literal),
            ..
        }) = &name_value.value
        else {
            continue;
        };
        if let Some(id) = literal.value().trim().strip_prefix("operationId: ") {
            return Some(id.trim().to_string());
        }
    }
    None
}

/// Token-stream equality, used to decide `Updated` vs. `Unchanged`: two fns
/// with identical attributes, signature, and body render identically.
fn fns_render_equal(a: &syn::ItemFn, b: &syn::ItemFn) -> bool {
    a.to_token_stream().to_string() == b.to_token_stream().to_string()
}

/// The name a struct/enum item was declared under, or `None` for any other
/// item kind.
fn item_name(item: &Item) -> Option<String> {
    match item {
        Item::Struct(item_struct) => Some(item_struct.ident.to_string()),
        Item::Enum(item_enum) => Some(item_enum.ident.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import_openapi::operations::ImportOperation;
    use serde_json::{json, Value};

    fn empty_document() -> Value {
        json!({ "openapi": "3.1.0", "paths": {} })
    }

    fn operation(id: &str) -> ImportOperation {
        ImportOperation {
            operation_id: id.to_string(),
            method: "GET".to_string(),
            path: "/health".to_string(),
            description: None,
            tags: Vec::new(),
            path_params: Vec::new(),
            query_params: Vec::new(),
            request_schema: None,
            response_schema: None,
            skip_reason: None,
        }
    }

    /// Renders a `syn::File` back to (ugly, but valid) Rust source, so it
    /// can be re-parsed the way an on-disk file would be.
    fn render(file: &syn::File) -> String {
        quote::quote! { #file }.to_string()
    }

    #[test]
    fn no_existing_file_reports_everything_as_added() {
        let document = empty_document();
        let generated = codegen::generate(&document, &[operation("health")]).unwrap();

        let outcome = merge(None, generated, false).expect("merge must succeed");
        assert_eq!(outcome.report.added, vec!["health".to_string()]);
        assert!(outcome.report.updated.is_empty());
        assert!(outcome.report.removed.is_empty());
    }

    #[test]
    fn rerunning_with_an_unchanged_spec_reports_nothing() {
        let document = empty_document();
        let first = codegen::generate(&document, &[operation("health")]).unwrap();
        let first_outcome = merge(None, first, false).unwrap();
        let existing_source = render(&first_outcome.file);

        let second = codegen::generate(&document, &[operation("health")]).unwrap();
        let outcome = merge(Some(&existing_source), second, false).expect("merge must succeed");

        assert!(outcome.report.added.is_empty());
        assert!(outcome.report.updated.is_empty());
        assert!(outcome.report.removed.is_empty());
    }

    #[test]
    fn metadata_change_is_reported_as_updated() {
        let document = empty_document();
        let first = codegen::generate(&document, &[operation("health")]).unwrap();
        let first_outcome = merge(None, first, false).unwrap();
        let existing_source = render(&first_outcome.file);

        let mut changed = operation("health");
        changed.description = Some("Now with a description".to_string());
        let second = codegen::generate(&document, &[changed]).unwrap();

        let outcome = merge(Some(&existing_source), second, false).expect("merge must succeed");
        assert_eq!(outcome.report.updated, vec!["health".to_string()]);
        assert!(outcome.report.added.is_empty());

        let rendered = render(&outcome.file);
        assert!(rendered.contains("Now with a description"));
    }

    #[test]
    fn todo_body_is_replaced_when_metadata_changes() {
        let document = empty_document();
        let first = codegen::generate(&document, &[operation("health")]).unwrap();
        let first_outcome = merge(None, first, false).unwrap();
        let existing_source = render(&first_outcome.file);

        let mut changed = operation("health");
        changed.path = "/healthz".to_string();
        let second = codegen::generate(&document, &[changed]).unwrap();

        let outcome = merge(Some(&existing_source), second, false).expect("merge must succeed");
        let rendered = render(&outcome.file);
        assert!(rendered.contains("todo ! (\"Implement handler\")"));
        assert!(rendered.contains("path = \"/healthz\""));
    }

    #[test]
    fn handwritten_body_survives_a_metadata_change() {
        let document = empty_document();
        let first = codegen::generate(&document, &[operation("health")]).unwrap();
        let first_outcome = merge(None, first, false).unwrap();
        let mut existing_source = render(&first_outcome.file);
        existing_source =
            existing_source.replace("todo ! (\"Implement handler\")", "\"ok\" . to_string ()");

        let mut changed = operation("health");
        changed.description = Some("Health check".to_string());
        let second = codegen::generate(&document, &[changed]).unwrap();

        let outcome = merge(Some(&existing_source), second, false).expect("merge must succeed");
        let rendered = render(&outcome.file);
        assert!(rendered.contains("\"ok\" . to_string ()"));
        assert!(!rendered.contains("todo !"));
        assert_eq!(outcome.report.updated, vec!["health".to_string()]);
    }

    #[test]
    fn removed_operation_is_reported_but_kept_by_default() {
        let document = empty_document();
        let first = codegen::generate(&document, &[operation("health"), operation("bye")]).unwrap();
        let first_outcome = merge(None, first, false).unwrap();
        let existing_source = render(&first_outcome.file);

        let second = codegen::generate(&document, &[operation("health")]).unwrap();
        let outcome = merge(Some(&existing_source), second, false).expect("merge must succeed");

        assert_eq!(outcome.report.removed, vec!["bye".to_string()]);
        let rendered = render(&outcome.file);
        assert!(
            rendered.contains("operationId: bye"),
            "default behavior must not delete a removed operation's code"
        );
    }

    #[test]
    fn removed_operation_is_deleted_with_remove_orphaned() {
        let document = empty_document();
        let first = codegen::generate(&document, &[operation("health"), operation("bye")]).unwrap();
        let first_outcome = merge(None, first, false).unwrap();
        let existing_source = render(&first_outcome.file);

        let second = codegen::generate(&document, &[operation("health")]).unwrap();
        let outcome = merge(Some(&existing_source), second, true).expect("merge must succeed");

        assert_eq!(outcome.report.removed, vec!["bye".to_string()]);
        let rendered = render(&outcome.file);
        assert!(
            !rendered.contains("operationId: bye"),
            "--remove-orphaned must physically delete the removed operation's code"
        );
    }

    #[test]
    fn orphaned_struct_is_kept_by_default_and_deleted_with_remove_orphaned() {
        let document = json!({ "openapi": "3.1.0", "paths": {} });
        let mut with_response = operation("create_widget");
        with_response.method = "POST".to_string();
        with_response.response_schema =
            Some(json!({ "type": "object", "properties": { "id": { "type": "integer" } } }));

        let first = codegen::generate(&document, &[with_response]).unwrap();
        let first_outcome = merge(None, first, false).unwrap();
        let existing_source = render(&first_outcome.file);
        assert!(existing_source.contains("CreateWidgetResponse"));

        let second = codegen::generate(&document, &[operation("health")]).unwrap();
        let kept = merge(Some(&existing_source), second, false).expect("merge must succeed");
        assert!(render(&kept.file).contains("CreateWidgetResponse"));

        let second = codegen::generate(&document, &[operation("health")]).unwrap();
        let deleted = merge(Some(&existing_source), second, true).expect("merge must succeed");
        assert!(!render(&deleted.file).contains("CreateWidgetResponse"));
    }
}
