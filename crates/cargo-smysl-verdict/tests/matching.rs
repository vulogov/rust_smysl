//! Checking one claim against facts (D11): what is retrieved, what is asked, and what a citation must be.

use std::cell::RefCell;

use cargo_smysl_extract::{Charged, Judge, JudgeError};
use cargo_smysl_facts::facts;
use cargo_smysl_verdict::matching::{match_claim, retrieve, Retrieval};
use cargo_smysl_verdict::policy::{decide, Verdict};

struct Scripted {
    answer: String,
    asked: RefCell<Vec<String>>,
}

impl Judge for Scripted {
    fn describe(&self) -> String {
        "scripted".into()
    }
    fn ask_text(&self, _system: &str, user: &str) -> Result<(String, Charged), JudgeError> {
        self.asked.borrow_mut().push(user.to_string());
        Ok((self.answer.clone(), Charged::default()))
    }
}

fn scripted(answer: &str) -> Scripted {
    Scripted {
        answer: answer.into(),
        asked: RefCell::new(Vec::new()),
    }
}

const SOURCE: &str = r#"
/// The ledger lives beside the store, never in the user's home directory.
pub const LEDGER: &str = "usage.log";

pub fn root_beside(store: Option<&str>) -> String {
    match store {
        Some(path) => parent_of(path),
        None => ".".to_string(),
    }
}

pub fn unrelated_renderer() -> u32 { 7 }
"#;

#[test]
fn what_is_shown_is_the_facts_that_share_the_claim_s_words_prose_apart() {
    let all = facts("src/main.rs", SOURCE).unwrap();
    let shown = retrieve(
        "the ledger is resolved beside the store",
        &all,
        &Retrieval::default(),
    );
    let text = shown.text();
    assert!(
        text.contains("F1:"),
        "structural facts are numbered F…: {text}"
    );
    assert!(
        shown
            .structural
            .iter()
            .any(|f| cargo_smysl_facts::render(f).contains("root_beside")),
        "{text}"
    );
    assert!(
        !text.contains("unrelated_renderer"),
        "and a fact with nothing in common is not shown: {text}"
    );
    assert!(text.contains("P1:"), "author prose is shown apart: {text}");
}

#[test]
fn the_model_is_asked_about_one_claim_with_its_own_facts() {
    let all = facts("src/main.rs", SOURCE).unwrap();
    let shown = retrieve(
        "the ledger is resolved beside the store",
        &all,
        &Retrieval::default(),
    );
    let judge = scripted(
        r#"{"covered":[{"part":"resolved beside the store","facts":["F1"]}],"uncovered":[]}"#,
    );
    let matched = match_claim(
        "the ledger is resolved beside the store",
        &shown,
        &judge,
        &Retrieval::default(),
    )
    .unwrap();
    let asked = judge.asked.borrow();
    assert!(asked[0].starts_with("CLAIM: the ledger"), "{}", asked[0]);
    assert_eq!(asked.len(), 1, "one claim, one call");
    assert!(matched.judgement.full());
    assert!(!matched.judgement.prose_used);
}

#[test]
fn a_part_covered_only_by_prose_marks_the_run_and_cannot_be_supported() {
    let all = facts("src/main.rs", SOURCE).unwrap();
    let shown = retrieve(
        "the ledger is resolved beside the store",
        &all,
        &Retrieval::default(),
    );
    let judge =
        scripted(r#"{"covered":[{"part":"beside the store","facts":["P1"]}],"uncovered":[]}"#);
    let matched = match_claim(
        "the ledger is beside the store",
        &shown,
        &judge,
        &Retrieval::default(),
    )
    .unwrap();
    assert!(
        matched.judgement.prose_used,
        "a doc comment is not a verification"
    );

    let both = [matched.judgement.clone(), matched.judgement];
    assert_eq!(decide(&both, false, false).verdict, Verdict::ProseOnly);
}

#[test]
fn a_cited_fact_the_model_was_not_shown_does_not_count() {
    let all = facts("src/main.rs", SOURCE).unwrap();
    let shown = retrieve(
        "the ledger is resolved beside the store",
        &all,
        &Retrieval::default(),
    );
    let judge = scripted(
        r#"{"covered":[{"part":"beside the store","facts":["F99"]},
             {"part":"never in home","facts":["F1","F98"]}],"uncovered":[]}"#,
    );
    let matched = match_claim(
        "the ledger is beside the store",
        &shown,
        &judge,
        &Retrieval::default(),
    )
    .unwrap();
    assert!(
        matched.judgement.uncovered.contains("beside the store"),
        "a part resting only on an invented fact is uncovered: {:?}",
        matched.judgement
    );
    assert!(
        matched.judgement.covered.contains("never in home"),
        "a real citation stands"
    );
    assert_eq!(matched.invented, vec!["F99".to_string(), "F98".to_string()]);
}

#[test]
fn a_contradiction_counts_only_when_it_names_a_fact_that_exists() {
    let all = facts("src/main.rs", SOURCE).unwrap();
    let shown = retrieve(
        "the ledger is resolved beside the store",
        &all,
        &Retrieval::default(),
    );

    let invented = scripted(
        r#"{"covered":[],"uncovered":["beside"],"contradicted":true,"contradicted_by":["F42"]}"#,
    );
    let m = match_claim(
        "the ledger is beside the store",
        &shown,
        &invented,
        &Retrieval::default(),
    )
    .unwrap();
    assert!(
        !m.judgement.contradicted,
        "an invented contradiction is not one"
    );

    let real = scripted(
        r#"{"covered":[],"uncovered":["beside"],"contradicted":true,"contradicted_by":["F1"]}"#,
    );
    let m = match_claim(
        "the ledger is beside the store",
        &shown,
        &real,
        &Retrieval::default(),
    )
    .unwrap();
    assert!(m.judgement.contradicted);
    assert_eq!(
        decide(&[m.judgement], false, false).verdict,
        Verdict::Contradicted
    );
}

#[test]
fn a_claim_nothing_bears_on_is_not_asked_about_at_all() {
    let all = facts("src/main.rs", SOURCE).unwrap();
    let shown = retrieve(
        "quantum chromodynamics in the renderer",
        &all,
        &Retrieval::default(),
    );
    let judge = scripted("{}");
    let matched = match_claim(
        "quantum chromodynamics",
        &shown,
        &judge,
        &Retrieval::default(),
    )
    .unwrap();
    assert!(
        judge.asked.borrow().is_empty(),
        "no facts, no call, no tokens"
    );
    assert!(!matched.judgement.covered.iter().any(|_| true));
    assert_eq!(
        decide(&[matched.judgement], false, false).verdict,
        Verdict::Unknown
    );
}
