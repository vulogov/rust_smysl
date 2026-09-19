//! Which tests might bear on a claim (D14).
//!
//! Deterministic, and only a shortlist: the ranking is lexical overlap between the claim and what a
//! test *is* — its name, what it calls, what it asserts, its doc — with a bonus for tests in the files a
//! change touched and for tests naming an item the claim names. A model then classifies the shortlist
//! (verifies / exercises / unrelated); it never goes looking on its own, because the research measured
//! that a model asked to find tests invents plausible ones.
//!
//! Scoring is deliberately simple and stated here rather than tuned: a term the claim and the test share
//! scores once, rarer terms score more (a term in every test says nothing), and the two bonuses are flat.

use std::collections::{BTreeMap, BTreeSet};

use cargo_smysl_facts::item::Function;
use cargo_smysl_facts::Fact;

/// A test worth asking about, with why it is here.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// `owner::name`, as the facts see it.
    pub label: String,
    pub file: String,
    pub line: usize,
    pub score: f32,
    /// The terms the claim and this test share, strongest first.
    pub shared: Vec<String>,
    /// It is in a file the change touched.
    pub touched: bool,
}

/// The tests among these facts: `#[test]`, or under a `cfg(test)` module.
pub fn tests(facts: &[Fact]) -> Vec<&Function> {
    facts
        .iter()
        .filter_map(|f| match f {
            Fact::Function(x) if x.is_test() || x.file_cfg.iter().any(|c| c.contains("test")) => {
                Some(x)
            }
            _ => None,
        })
        .collect()
}

/// The shortlist for one claim, best first, at most `limit`.
pub fn candidates(facts: &[Fact], claim: &str, touched: &[String], limit: usize) -> Vec<Candidate> {
    let tests = tests(facts);
    if tests.is_empty() {
        return Vec::new();
    }
    let claim_terms = terms(claim);
    if claim_terms.is_empty() {
        return Vec::new();
    }

    // How rare a term is among the tests: a term every test uses tells us nothing about which one.
    let mut seen_in: BTreeMap<String, usize> = BTreeMap::new();
    let profiles: Vec<BTreeSet<String>> = tests.iter().map(|t| profile(t)).collect();
    for p in &profiles {
        for term in p {
            *seen_in.entry(term.clone()).or_default() += 1;
        }
    }

    let mut out: Vec<Candidate> = tests
        .iter()
        .zip(&profiles)
        .map(|(test, p)| {
            let mut shared: Vec<(String, f32)> = claim_terms
                .iter()
                .filter(|t| p.contains(*t))
                .map(|t| {
                    let rarity =
                        tests.len() as f32 / (1 + seen_in.get(t).copied().unwrap_or(0)) as f32;
                    (t.clone(), rarity.ln().max(0.1))
                })
                .collect();
            shared.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let mut score: f32 = shared.iter().map(|(_, w)| *w).sum();
            let is_touched = touched.contains(&test.file);
            if is_touched {
                score += 1.0;
            }
            Candidate {
                label: test.label(),
                file: test.file.clone(),
                line: test.line,
                score,
                shared: shared.into_iter().map(|(t, _)| t).collect(),
                touched: is_touched,
            }
        })
        .filter(|c| c.score > 0.0)
        .collect();
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.label.cmp(&b.label))
    });
    out.truncate(limit);
    out
}

/// What a test is, as terms: its name, what it calls, what it asserts, its doc.
fn profile(test: &Function) -> BTreeSet<String> {
    let mut out = terms(&test.label());
    out.extend(terms(&test.doc));
    for e in &test.events {
        for (key, value) in &e.detail {
            // A string literal in a test is usually the thing under test, so it counts here even though
            // prose never *verifies* (D10): this is retrieval, not a verdict.
            if matches!(
                key.as_str(),
                "callee" | "method" | "name" | "tokens" | "text"
            ) {
                out.extend(terms(value));
            }
        }
    }
    out
}

/// Words and identifier parts, lowercased: `root_beside`, `rootBeside` and `root beside` all meet.
fn terms(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for raw in text.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        if raw.is_empty() {
            continue;
        }
        let lower = raw.to_lowercase();
        if lower.len() >= 3 && !lower.chars().all(|c| c.is_numeric()) {
            out.insert(lower.clone());
        }
        for part in split_identifier(raw) {
            if part.len() >= 3 {
                out.insert(part);
            }
        }
    }
    // Words that say nothing about which test: they are in the claim and in every test.
    for noise in [
        "the", "and", "not", "for", "with", "that", "this", "test", "tests", "fn",
    ] {
        out.remove(noise);
    }
    out
}

fn split_identifier(raw: &str) -> Vec<String> {
    let mut parts = Vec::new();
    for chunk in raw.split('_') {
        let mut current = String::new();
        for c in chunk.chars() {
            if c.is_uppercase() && !current.is_empty() {
                parts.push(std::mem::take(&mut current).to_lowercase());
            }
            current.push(c);
        }
        if !current.is_empty() {
            parts.push(current.to_lowercase());
        }
    }
    parts
}
