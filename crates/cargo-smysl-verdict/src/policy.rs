//! What a run is allowed to conclude (D11, D12).
//!
//! The rule that shapes everything here: **a single run never raises a status.** The research measured
//! why — part-based coverage alone overclaimed four times, and two-run agreement still got one of seven
//! wrong. So `Supported` needs three things at once: every part of the claim covered, every covering
//! fact structural rather than author prose (D10), and either two independent runs agreeing or a
//! person's word. Everything weaker attaches evidence and says what is missing.
//!
//! A **normative** prerequisite — a rule the project states, rather than a fact about the code — can
//! reach `ImplementedBy` at most. Code can show a rule is followed; it cannot make it true.

use std::collections::BTreeSet;

use serde::Serialize;

/// What one run concluded about one claim.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Judgement {
    /// The parts of the claim this run found covered.
    pub covered: BTreeSet<String>,
    /// The parts it could not cover.
    pub uncovered: BTreeSet<String>,
    /// Facts it rested on that are author prose (D10): doc comments, string literals, string constants.
    pub prose_used: bool,
    /// A fact that contradicts the claim.
    pub contradicted: bool,
    /// Who ran it: two runs agree only if they are different runs.
    pub run: String,
}

impl Judgement {
    pub fn full(&self) -> bool {
        !self.covered.is_empty() && self.uncovered.is_empty()
    }
}

/// What the corpus may record after these runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// Every part covered by structural facts, and two runs agree — or a person said so.
    Supported,
    /// Some parts covered. Evidence attaches; the status does not move.
    Partial,
    /// Covered only by what the code says about itself, which never verifies (D10).
    ProseOnly,
    /// A normative rule the code follows: the most a rule can get from code.
    ImplementedBy,
    /// A fact contradicts it. This goes to review, not to a status change.
    Contradicted,
    /// Nothing bears on it.
    Unknown,
}

impl Verdict {
    /// Whether this verdict may change a unit's status, which only `Supported` may.
    pub fn raises(&self) -> bool {
        matches!(self, Verdict::Supported)
    }

    /// Whether it belongs in the review queue rather than in the store as a conclusion.
    pub fn needs_review(&self) -> bool {
        matches!(self, Verdict::Contradicted)
    }
}

/// Why the verdict is what it is, in the words a report uses.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Decision {
    pub verdict: Verdict,
    pub because: String,
    /// Parts no run could cover, so a reader knows what is missing.
    pub uncovered: Vec<String>,
}

/// Apply the policy to what the runs found.
///
/// `normative` marks a rule the project states; `confirmed_by_person` is a person's word, which stands
/// in for agreement because the agreement rule exists to avoid trusting one model, not to outrank a
/// human reviewer.
pub fn decide(runs: &[Judgement], normative: bool, confirmed_by_person: bool) -> Decision {
    if runs.is_empty() {
        return Decision {
            verdict: Verdict::Unknown,
            because: "no run looked at it".into(),
            uncovered: Vec::new(),
        };
    }
    if runs.iter().any(|r| r.contradicted) {
        return Decision {
            verdict: Verdict::Contradicted,
            because: "a fact contradicts it; a person decides what that means".into(),
            uncovered: Vec::new(),
        };
    }

    let uncovered: BTreeSet<String> = runs
        .iter()
        .flat_map(|r| r.uncovered.iter().cloned())
        .filter(|part| !runs.iter().any(|r| r.covered.contains(part)))
        .collect();
    let any_covered = runs.iter().any(|r| !r.covered.is_empty());
    let full = uncovered.is_empty() && any_covered;
    let structural = runs.iter().any(|r| r.full() && !r.prose_used);

    if !any_covered {
        return Decision {
            verdict: Verdict::Unknown,
            because: "nothing found bears on it".into(),
            uncovered: uncovered.into_iter().collect(),
        };
    }
    if full && !structural {
        return Decision {
            verdict: Verdict::ProseOnly,
            because: "covered only by what the code says about itself, which never verifies".into(),
            uncovered: Vec::new(),
        };
    }
    if !full {
        return Decision {
            verdict: Verdict::Partial,
            because: format!("{} part(s) of the claim are uncovered", uncovered.len()),
            uncovered: uncovered.into_iter().collect(),
        };
    }
    if normative {
        return Decision {
            verdict: Verdict::ImplementedBy,
            because: "a rule the project states: the code follows it, which does not make it true"
                .into(),
            uncovered: Vec::new(),
        };
    }
    // Two *independent* runs that each saw the whole claim covered. Coverage that only adds up across
    // runs is not agreement: one run saw what the other missed, which is the disagreement this rule
    // exists to catch. The same run twice is also one run.
    let agreeing: BTreeSet<&str> = runs
        .iter()
        .filter(|r| r.full() && !r.prose_used && !r.contradicted)
        .map(|r| r.run.as_str())
        .collect();
    if confirmed_by_person {
        Decision {
            verdict: Verdict::Supported,
            because: "a person confirmed it".into(),
            uncovered: Vec::new(),
        }
    } else if agreeing.len() >= 2 {
        Decision {
            verdict: Verdict::Supported,
            because: format!(
                "{} independent runs agree, on structural facts",
                agreeing.len()
            ),
            uncovered: Vec::new(),
        }
    } else {
        Decision {
            verdict: Verdict::Partial,
            because: "one run is not enough to raise a status: a second run or a person is needed"
                .into(),
            uncovered: Vec::new(),
        }
    }
}
