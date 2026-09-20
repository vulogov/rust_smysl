//! Staleness: what counts as the code having moved, and what does not.

use cargo_smysl_facts::facts;
use cargo_smysl_verdict::stale::{compare, Moved};

const BEFORE: &str = r#"
pub fn within(value: usize, limit: usize) -> bool {
    value < limit
}

pub fn describe() -> &'static str {
    "a value and a limit"
}

pub fn retired() -> usize {
    7
}
"#;

#[test]
fn a_function_that_only_moved_is_not_stale() {
    // Same bodies, different order, a comment and a new function in between.
    let after = r#"
/// Added since, and nothing to do with the reasoning.
pub fn added() -> usize {
    1
}

pub fn describe() -> &'static str {
    "a value and a limit"
}

// A comment that did not exist before.
pub fn within(value: usize, limit: usize) -> bool {
    value < limit
}

pub fn retired() -> usize {
    7
}
"#;
    let changes = compare(
        &facts("src/lib.rs", BEFORE).unwrap(),
        &facts("src/lib.rs", after).unwrap(),
    );
    assert!(
        changes.is_empty(),
        "moving a function is not changing it: {changes:?}"
    );
}

#[test]
fn a_changed_body_and_a_removed_item_are_told_apart() {
    let after = r#"
pub fn within(value: usize, limit: usize) -> bool {
    value <= limit
}

pub fn describe() -> &'static str {
    "a value and a limit"
}
"#;
    let changes = compare(
        &facts("src/lib.rs", BEFORE).unwrap(),
        &facts("src/lib.rs", after).unwrap(),
    );
    assert_eq!(changes.len(), 2, "{changes:?}");
    let within = changes.iter().find(|c| c.label.contains("within")).unwrap();
    assert_eq!(within.moved, Moved::Changed);
    assert!(within.because().contains("changed in src/lib.rs"));
    let retired = changes
        .iter()
        .find(|c| c.label.contains("retired"))
        .unwrap();
    assert_eq!(retired.moved, Moved::Gone);
    assert!(retired.because().contains("no longer in"));
}

#[test]
fn a_file_that_is_gone_leaves_its_items_gone() {
    let changes = compare(&facts("src/lib.rs", BEFORE).unwrap(), &[]);
    assert_eq!(changes.len(), 3);
    assert!(changes.iter().all(|c| c.moved == Moved::Gone));
}
