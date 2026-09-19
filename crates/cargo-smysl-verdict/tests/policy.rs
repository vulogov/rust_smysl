//! The verdict policy (D12): what a run may conclude, and what it may not.

use std::collections::BTreeSet;

use cargo_smysl_verdict::policy::{decide, Judgement, Verdict};

fn parts(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn run(name: &str, covered: &[&str], uncovered: &[&str]) -> Judgement {
    Judgement {
        covered: parts(covered),
        uncovered: parts(uncovered),
        prose_used: false,
        contradicted: false,
        run: name.into(),
    }
}

#[test]
fn one_run_never_raises_a_status() {
    let d = decide(
        &[run("a", &["the ledger is beside the store"], &[])],
        false,
        false,
    );
    assert_eq!(d.verdict, Verdict::Partial, "{}", d.because);
    assert!(!d.verdict.raises());
    assert!(d.because.contains("second run or a person"));
}

#[test]
fn two_independent_runs_agreeing_on_structural_facts_do() {
    let claim = ["the ledger is beside the store"];
    let d = decide(
        &[run("a", &claim, &[]), run("b", &claim, &[])],
        false,
        false,
    );
    assert_eq!(d.verdict, Verdict::Supported, "{}", d.because);
    assert!(d.verdict.raises());

    // The same run twice is one run.
    let same = decide(
        &[run("a", &claim, &[]), run("a", &claim, &[])],
        false,
        false,
    );
    assert_eq!(same.verdict, Verdict::Partial, "{}", same.because);
}

#[test]
fn a_person_stands_in_for_agreement() {
    let d = decide(&[run("a", &["covered"], &[])], false, true);
    assert_eq!(d.verdict, Verdict::Supported);
    assert!(d.because.contains("person"));
}

#[test]
fn prose_alone_never_verifies() {
    let mut only_prose = run("a", &["covered"], &[]);
    only_prose.prose_used = true;
    let d = decide(&[only_prose.clone(), only_prose], false, false);
    assert_eq!(d.verdict, Verdict::ProseOnly, "{}", d.because);
    assert!(!d.verdict.raises());
}

#[test]
fn a_normative_rule_reaches_implemented_by_at_most() {
    let claim = ["ticks are unsigned"];
    let d = decide(&[run("a", &claim, &[]), run("b", &claim, &[])], true, false);
    assert_eq!(d.verdict, Verdict::ImplementedBy, "{}", d.because);
    assert!(!d.verdict.raises(), "code cannot make a rule true");
}

#[test]
fn a_part_no_run_covered_keeps_the_verdict_partial_and_says_which() {
    let d = decide(
        &[
            run("a", &["reads the ledger"], &["writes it beside the store"]),
            run("b", &["reads the ledger"], &["writes it beside the store"]),
        ],
        false,
        false,
    );
    assert_eq!(d.verdict, Verdict::Partial);
    assert_eq!(d.uncovered, vec!["writes it beside the store".to_string()]);

    // Coverage that only adds up across runs is not agreement: one run saw the whole claim covered and
    // the other did not, which is the disagreement the two-run rule exists to catch.
    let together = decide(
        &[
            run("a", &["reads the ledger"], &["writes it beside the store"]),
            run(
                "b",
                &["reads the ledger", "writes it beside the store"],
                &[],
            ),
        ],
        false,
        false,
    );
    assert_eq!(together.verdict, Verdict::Partial, "{}", together.because);
    assert!(
        together.uncovered.is_empty(),
        "nothing is left uncovered between them"
    );
}

#[test]
fn a_contradiction_goes_to_review_not_to_a_status() {
    let mut against = run("a", &["covered"], &[]);
    against.contradicted = true;
    let d = decide(&[against, run("b", &["covered"], &[])], false, true);
    assert_eq!(d.verdict, Verdict::Contradicted);
    assert!(d.verdict.needs_review());
    assert!(
        !d.verdict.raises(),
        "not even a person's word skips review here"
    );
}

#[test]
fn nothing_found_is_unknown_not_false() {
    assert_eq!(decide(&[], false, false).verdict, Verdict::Unknown);
    assert_eq!(
        decide(&[run("a", &[], &["everything"])], false, false).verdict,
        Verdict::Unknown
    );
}
