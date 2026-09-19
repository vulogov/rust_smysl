//! Linking a test to the claim it bears on (D13), and what that link is allowed to say.
//!
//! Three kinds of link, and only one of them is evidence:
//!
//! - **verifies** — the test would fail if the claim were false. It becomes a `backs` edge from the
//!   reading to the claim, and it is **pending until a person confirms it** (D15): S1 measured 3 wrong
//!   links and 1 vacuous test in 17, so a model's own say-so is not enough.
//! - **exercises** — the test runs the code the claim is about without testing the claim. It becomes an
//!   `x.code/exercises` edge, which is navigation, not support.
//! - **unrelated** — no edge.
//!
//! **A failing reading never backs a claim.** A test that did not pass says nothing in favour of what it
//! was linked to; if it verifies the claim and failed, that is a `rebuts` edge, which goes to review like
//! any other disagreement.
//!
//! Every edge carries an attestation naming who proposed it — the linking model, at the model rung — so
//! the store says whose claim this is, and the review queue can tell a proposal from a confirmation.

use cargo_smysl_corpus::{REL_EXERCISES, REL_TOUCHES};
use smysl::stage::Attest;
use smysl::{AgentId, Hlc, Record, RelKind, Relation, Rung, Uid};

use crate::run::Reading;

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("{0}")]
    Smysl(String),
}

/// What a model says about a test and a claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The test would fail if the claim were false.
    Verifies,
    /// It runs the code the claim is about, without testing the claim.
    Exercises,
    Unrelated,
}

impl Kind {
    pub fn parse(s: &str) -> Kind {
        match s.trim().to_ascii_lowercase().as_str() {
            "verifies" => Kind::Verifies,
            "exercises" => Kind::Exercises,
            _ => Kind::Unrelated,
        }
    }
}

/// One proposed link: this reading, that claim, this kind.
#[derive(Debug, Clone)]
pub struct Link {
    pub reading: Uid,
    pub claim: Uid,
    pub kind: Kind,
}

/// The edges a set of links contributes, each attested to the agent that proposed it.
///
/// `outcomes` says what each reading did, so a failing test cannot be made to back anything.
pub fn edges(
    links: &[Link],
    outcomes: &dyn Fn(Uid) -> Option<Reading>,
    proposer: &AgentId,
) -> Result<Vec<Record>, LinkError> {
    let attest = Attest::new(proposer.clone(), Rung::Model, Hlc::zero(proposer.clone()));
    let exercises = RelKind::parse(REL_EXERCISES).map_err(|e| LinkError::Smysl(e.to_string()))?;
    let mut out = Vec::new();
    for link in links {
        let passed = outcomes(link.reading).map(|r| r.passed());
        let kind = match (link.kind, passed) {
            (Kind::Unrelated, _) => continue,
            // A reading that was not taken is not evidence either way.
            (_, None) => continue,
            (Kind::Verifies, Some(true)) => RelKind::Backs,
            // It verifies the claim and it failed: that is a disagreement, not support.
            (Kind::Verifies, Some(false)) => RelKind::Rebuts,
            (Kind::Exercises, _) => exercises.clone(),
        };
        let edge = Relation::new(kind, link.reading, link.claim);
        out.push(Record::Attestation(attest.for_relation(edge.uid())));
        out.push(Record::Relation(edge));
    }
    Ok(out)
}

/// The anchor edge a decision keeps to the code it touched, for completeness beside the evidence edges.
pub fn touches(decision: Uid, anchor: Uid, proposer: &AgentId) -> Result<Vec<Record>, LinkError> {
    let attest = Attest::new(
        proposer.clone(),
        Rung::Computed,
        Hlc::zero(proposer.clone()),
    );
    let kind = RelKind::parse(REL_TOUCHES).map_err(|e| LinkError::Smysl(e.to_string()))?;
    let edge = Relation::new(kind, decision, anchor);
    Ok(vec![
        Record::Attestation(attest.for_relation(edge.uid())),
        Record::Relation(edge),
    ])
}
