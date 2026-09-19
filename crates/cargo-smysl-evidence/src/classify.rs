//! Asking what a shortlisted test actually does about a claim (D13, D14).
//!
//! The shortlist is the tool's (deterministic, lexical); the classification is the model's, and it is a
//! proposal rather than a finding: S1 measured 3 wrong links and 1 vacuous test in 17, so every edge this
//! produces waits for a person (D15).
//!
//! The model is given the claim and the tests as facts — name, what they call, what they assert — and
//! must answer for each by name. A name it was not shown is dropped, as everywhere else.

use std::collections::BTreeSet;

use cargo_smysl_extract::{Judge, JudgeError};
use cargo_smysl_facts::{render, Fact};
use serde::Deserialize;

use crate::candidate::Candidate;
use crate::link::Kind;

pub const SYSTEM: &str = "You are given one claim about a Rust codebase and several tests, and you say \
what each test does about the claim.\n\
- `verifies`: the test would fail if the claim were false. This is the strong answer, and it is rare: the \
test must assert the thing the claim asserts, not merely run the code the claim is about.\n\
- `exercises`: it runs that code without testing the claim.\n\
- `unrelated`: it does not bear on the claim.\n\
Answer for each test by the name you were given, and for nothing else. If you are unsure between \
`verifies` and `exercises`, answer `exercises`: a wrong `verifies` becomes evidence that is not there.\n\
Return one JSON object: {\"links\": [{\"test\": string, \"kind\": \"verifies\"|\"exercises\"|\"unrelated\", \
\"because\": string}]}";

#[derive(Debug, Default, Deserialize)]
struct Answer {
    #[serde(default)]
    links: Vec<LinkAnswer>,
}

#[derive(Debug, Deserialize)]
struct LinkAnswer {
    #[serde(default)]
    test: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    because: String,
}

/// What the model said about one test.
#[derive(Debug, Clone, PartialEq)]
pub struct Classified {
    pub test: String,
    pub kind: Kind,
    pub because: String,
}

/// Classify a shortlist. Tests the model did not answer for are left out rather than guessed at.
pub fn classify(
    claim: &str,
    shortlist: &[Candidate],
    facts: &[Fact],
    judge: &dyn Judge,
) -> Result<(Vec<Classified>, Vec<String>), JudgeError> {
    if shortlist.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let known: BTreeSet<&str> = shortlist.iter().map(|c| c.label.as_str()).collect();
    let mut user = format!("CLAIM: {claim}\n\nTESTS:\n\n");
    for candidate in shortlist {
        let text = facts
            .iter()
            .find(|f| matches!(f, Fact::Function(x) if x.label() == candidate.label))
            .map(render)
            .unwrap_or_else(|| candidate.label.clone());
        user.push_str(&format!("{}\n{}\n\n", candidate.label, text));
    }
    let (answer, _charged) = judge.ask_text(SYSTEM, &user)?;
    let answer: Answer =
        serde_json::from_str(&answer).map_err(|e| JudgeError::Shape(e.to_string()))?;

    let (mut out, mut invented) = (Vec::new(), Vec::new());
    for link in answer.links {
        let name = link.test.trim().to_string();
        if !known.contains(name.as_str()) {
            invented.push(name);
            continue;
        }
        if out.iter().any(|c: &Classified| c.test == name) {
            continue;
        }
        out.push(Classified {
            test: name,
            kind: Kind::parse(&link.kind),
            because: link.because,
        });
    }
    Ok((out, invented))
}
