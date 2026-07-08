//! Prints the human-readable report at the end of an `import-openapi` run.

use super::merge::MergeReport;
use super::operations::ImportOperation;

/// Prints the `Added`/`Updated`/`Removed`/`Skipped` summary. `Unchanged`
/// operations aren't listed anywhere — they carry nothing worth reporting.
pub fn print_summary(report: &MergeReport, skipped: &[&ImportOperation]) {
    println!("OpenAPI import completed");
    println!();

    println!("Added: {}", report.added.len());
    for id in &report.added {
        println!("  + {id}");
    }

    println!("Updated: {}", report.updated.len());
    for id in &report.updated {
        println!("  ~ {id}");
    }

    println!("Removed: {}", report.removed.len());
    for id in &report.removed {
        println!("  - {id}");
    }

    println!("Skipped: {}", skipped.len());
    for operation in skipped {
        let reason = operation.skip_reason.as_deref().unwrap_or("unknown reason");
        println!("  ! {} ({reason})", operation.operation_id);
    }

    if !skipped.is_empty() {
        println!();
        println!("Finished with warnings.");
    }
}
