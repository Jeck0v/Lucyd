//! End-to-end test of `cargo xtask import-openapi`'s incremental-merge
//! guarantee: running the importer twice against a fixture spec, with a
//! hand-edit made to the generated output in between, must (a) preserve the
//! hand-edited handler body byte-for-byte, and (b) reflect an unrelated
//! field change from the second run's spec in that same, otherwise-untouched
//! operation's attribute.

use std::fs;

const FIXTURE: &str = include_str!("fixtures/sample_openapi.yaml");

const ORIGINAL_DESCRIPTION: &str = "Health check endpoint";
const UPDATED_DESCRIPTION: &str = "Liveness probe endpoint";

const HANDWRITTEN_BODY: &str = "\"ok\".to_string()";

#[test]
fn hand_edited_body_survives_an_unrelated_spec_change() {
    let workdir = tempfile::tempdir().expect("failed to create a temp workdir");
    let spec_path = workdir.path().join("openapi.yaml");
    let out_path = workdir.path().join("generated_endpoints.rs");

    // First run: generates the file from scratch.
    fs::write(&spec_path, FIXTURE).expect("failed to write the fixture spec");
    xtask::import_openapi::run(&spec_path, &out_path, false).expect("first run must succeed");

    let first_output = fs::read_to_string(&out_path).expect("generated file must exist");
    assert!(first_output.contains("fn health_check"));
    assert!(first_output.contains("fn create_user"));
    assert!(first_output.contains(ORIGINAL_DESCRIPTION));

    // Simulate a developer implementing the `health_check` stub by hand.
    // `paths` iterates in `serde_json::Map`'s (alphabetical, `BTreeMap`-backed)
    // order rather than YAML declaration order, so target the replacement by
    // function name instead of assuming which stub comes first in the file.
    let hand_edited = replace_todo_body_of(&first_output, "health_check");
    assert!(
        hand_edited.contains(HANDWRITTEN_BODY),
        "test setup must actually replace a todo!() body"
    );
    fs::write(&out_path, &hand_edited).expect("failed to write the hand-edited file");

    // Second run: the spec changes in one unrelated field (health_check's
    // description) — create_user is untouched.
    let updated_spec = FIXTURE.replace(ORIGINAL_DESCRIPTION, UPDATED_DESCRIPTION);
    fs::write(&spec_path, &updated_spec).expect("failed to write the updated spec");
    xtask::import_openapi::run(&spec_path, &out_path, false).expect("second run must succeed");

    let second_output = fs::read_to_string(&out_path).expect("generated file must exist");

    assert!(
        second_output.contains(HANDWRITTEN_BODY),
        "a hand-edited body must survive re-running the importer:\n{second_output}"
    );
    assert_eq!(
        second_output
            .matches("todo!(\"Implement handler\")")
            .count(),
        1,
        "only the untouched create_user stub may still carry a todo!() body:\n{second_output}"
    );
    assert!(
        second_output.contains(UPDATED_DESCRIPTION),
        "the changed description must show up in the regenerated attribute:\n{second_output}"
    );
    assert!(
        !second_output.contains(ORIGINAL_DESCRIPTION),
        "the stale description must not linger"
    );

    // Re-running a third time against the identical (already-updated) spec
    // must be a no-op: no todo!() bodies got reset, no descriptions moved.
    xtask::import_openapi::run(&spec_path, &out_path, false).expect("third run must succeed");
    let third_output = fs::read_to_string(&out_path).expect("generated file must exist");
    assert_eq!(
        second_output, third_output,
        "re-running against an unchanged spec must be idempotent"
    );
}

/// Replaces the `todo!("Implement handler")` body belonging to `fn_name`
/// with a trivial handwritten body, simulating a developer implementing
/// exactly that one stub (and leaving every other stub's body alone).
fn replace_todo_body_of(source: &str, fn_name: &str) -> String {
    let fn_marker = format!("fn {fn_name}(");
    let fn_start = source
        .find(&fn_marker)
        .unwrap_or_else(|| panic!("generated file must contain `{fn_marker}`"));
    let todo_offset = source[fn_start..]
        .find("todo!(\"Implement handler\")")
        .expect("the target stub must still have its default todo!() body");
    let todo_start = fn_start + todo_offset;
    let todo_end = todo_start + "todo!(\"Implement handler\")".len();

    let mut result = String::with_capacity(source.len());
    result.push_str(&source[..todo_start]);
    result.push_str(HANDWRITTEN_BODY);
    result.push_str(&source[todo_end..]);
    result
}
