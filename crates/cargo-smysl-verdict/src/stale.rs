//! Reasoning whose code has moved since it was recorded.
//!
//! A decision was made about code as it stood. When that code changes, the decision is not wrong — it is
//! **unexamined**: nobody has said whether it still holds. This reports that, and reports nothing else.
//! It never withdraws a unit, never lowers a status and never guesses: a moved item is a question for a
//! person, and the answer belongs in the review queue (D15).
//!
//! **The comparison is by item, not by file.** A file changes for many reasons; the question is whether
//! *this function* is still the function the reasoning was about. Facts carry `body_hash` — the body's
//! tokens hashed — so a function that moved down a file without changing is recognised as unchanged,
//! which is the whole point of hashing the body rather than the line range.
//!
//! **What this does not do yet.** The plan has `x.code/touches` edges from a decision to a code anchor,
//! withdrawn with a reason naming the commit when the anchor moves. The corpus does not record anchors
//! yet, so staleness is reported per commit and the files it touched, not per decision and its own
//! anchor. The report says which items moved, so the narrowing is visible when anchors arrive.

use std::collections::BTreeMap;

use cargo_smysl_facts::item::Function;
use cargo_smysl_facts::Fact;

/// What happened to one item since the reasoning was recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moved {
    /// The body's tokens differ: the code the reasoning was about is not this code.
    Changed,
    /// The item is not there any more, under that name.
    Gone,
}

/// One item that no longer matches what was recorded.
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    /// `owner::name`, as the facts see it.
    pub label: String,
    pub file: String,
    pub moved: Moved,
}

impl Change {
    /// The sentence a report uses.
    pub fn because(&self) -> String {
        match self.moved {
            Moved::Changed => format!("{} changed in {}", self.label, self.file),
            Moved::Gone => format!("{} is no longer in {}", self.label, self.file),
        }
    }
}

/// Compare the code as it was when the reasoning was recorded with the code as it is now.
///
/// Only items present in `recorded` are asked about: a function added since is not staleness, it is work.
/// An item whose body hashes the same is unchanged however far it has moved in the file.
pub fn compare(recorded: &[Fact], now: &[Fact]) -> Vec<Change> {
    let current: BTreeMap<String, &Function> =
        functions(now).into_iter().map(|f| (f.label(), f)).collect();
    let mut out = Vec::new();
    for was in functions(recorded) {
        let label = was.label();
        match current.get(&label) {
            None => out.push(Change {
                label,
                file: was.file.clone(),
                moved: Moved::Gone,
            }),
            Some(is) if is.body_hash != was.body_hash => out.push(Change {
                label,
                file: was.file.clone(),
                moved: Moved::Changed,
            }),
            Some(_) => {}
        }
    }
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

fn functions(facts: &[Fact]) -> Vec<&Function> {
    facts
        .iter()
        .filter_map(|f| match f {
            Fact::Function(x) => Some(x),
            _ => None,
        })
        .collect()
}

/// What a commit's recorded reasoning rests on, and what has moved under it.
#[derive(Debug, Clone)]
pub struct Report {
    /// The commit the reasoning was recorded from.
    pub commit: String,
    /// Units recorded for that commit: what is left unexamined when its code moves.
    pub units: usize,
    pub changes: Vec<Change>,
    /// Files the commit touched that could not be read now, with why.
    pub unreadable: Vec<String>,
}

impl Report {
    pub fn is_stale(&self) -> bool {
        !self.changes.is_empty()
    }

    /// How many items moved, and how many went away: the two are different questions for a reader.
    pub fn counts(&self) -> (usize, usize) {
        let gone = self
            .changes
            .iter()
            .filter(|c| c.moved == Moved::Gone)
            .count();
        (self.changes.len() - gone, gone)
    }
}
