//! What is waiting for a person, and what their answer records (D15).
//!
//! Review never deletes and never rewrites. Each outcome is a record smysl already defines:
//!
//! - **confirm** — an attestation by a person on the edge, which is what takes it out of the queue;
//! - **reject** — a `Withdrawal` naming the edge, with a unit saying why. The edge stays in the log and
//!   stops being followed, packed or counted;
//! - **close** — a `Resolution` naming a contention or a bare `rebuts` edge, with a note unit. It records
//!   that a review happened and decides nothing.
//!
//! The queue itself is smysl's `review_with`, asked to expect confirmation for the edges this tool
//! proposes: `backs`, and `x.code/exercises` for a model linking a test to the claim it verifies. A model
//! attesting its own proposal is not a review, which is why the accepted agent kind is a person (D13).

use cargo_smysl_corpus::REL_EXERCISES;
use smysl::stage::Attest;
use smysl::{
    review_with, AgentId, AgentKind, DetectionContext, Hlc, Record, RelKind, Resolution,
    ReviewItem, ReviewOptions, ReviewSubject, Rung, Store, Uid, UnitCoreBuilder, Withdrawal,
};

#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    #[error("{0}")]
    Smysl(String),
    #[error("no item in the queue is {0}")]
    NotQueued(String),
    #[error(
        "a reason is required: rejecting an edge without saying why leaves the store unreadable"
    )]
    NoReason,
}

/// The queue: everything waiting for a person, unresolved first.
pub fn queue(store: &Store) -> Vec<ReviewItem> {
    let exercises = RelKind::parse(REL_EXERCISES).ok();
    let mut kinds = vec![RelKind::Backs];
    kinds.extend(exercises);
    let mut items = review_with(
        store,
        &ReviewOptions::confirming(kinds)
            .confirmed_by(AgentKind::Human)
            .with_detection(DetectionContext::default()),
    );
    items.sort_by_key(|i| (i.resolved, describe(i)));
    items
}

/// What an item is, in one line a person can act on.
pub fn describe(item: &ReviewItem) -> String {
    match &item.subject {
        ReviewSubject::Contention(c) => format!("contention {}", c.id),
        ReviewSubject::Rebuttal(r) => {
            format!("rebuttal {} rebuts {}", r.from.short(), r.to.short())
        }
        ReviewSubject::Unconfirmed(r) => {
            format!("{} {} -> {}", r.kind, r.from.short(), r.to.short())
        }
        // `ReviewSubject` is non-exhaustive: a kind smysl adds later is listed rather than hidden.
        other => format!("{other:?}"),
    }
}

/// A person, as an agent id. Their answer is attributed to them, not to the tool: an attestation is a
/// claim about who stands behind an edge.
pub fn person(name: &str) -> Result<AgentId, ReviewError> {
    AgentId::new(format!("human:{name}")).map_err(|e| ReviewError::Smysl(e.to_string()))
}

/// Confirm an edge: the person attests it, and it leaves the queue.
pub fn confirm(item: &ReviewItem, who: &AgentId) -> Result<Vec<Record>, ReviewError> {
    let relation = match &item.subject {
        ReviewSubject::Unconfirmed(r) | ReviewSubject::Rebuttal(r) => r,
        ReviewSubject::Contention(_) => {
            return Err(ReviewError::NotQueued(
                "a contention is closed, not confirmed".into(),
            ))
        }
        other => return Err(ReviewError::NotQueued(format!("{other:?}"))),
    };
    // `Rung` says where content came from, not who stands behind it: a person's confirmation is a
    // `document` rung claim, and it is the agent kind (`human:…`) that makes it a review (D13).
    let attest = attest_as(who);
    Ok(vec![Record::Attestation(
        attest.for_relation(relation.uid()),
    )])
}

/// Reject an edge: a withdrawal naming it, and a unit saying why.
pub fn reject(item: &ReviewItem, who: &AgentId, why: &str) -> Result<Vec<Record>, ReviewError> {
    if why.trim().is_empty() {
        return Err(ReviewError::NoReason);
    }
    let relation = match &item.subject {
        ReviewSubject::Unconfirmed(r) | ReviewSubject::Rebuttal(r) => r,
        ReviewSubject::Contention(_) => {
            return Err(ReviewError::NotQueued(
                "a contention is closed, not withdrawn".into(),
            ))
        }
        other => return Err(ReviewError::NotQueued(format!("{other:?}"))),
    };
    let (reason, reason_uid, attestation) = note(why, who)?;
    let withdrawal = Withdrawal::new(relation.uid(), who.clone(), Hlc::zero(who.clone()))
        .with_reason(reason_uid);
    Ok(vec![
        reason,
        Record::Attestation(attestation),
        Record::Withdrawal(withdrawal),
    ])
}

/// Close a disagreement: a resolution naming it, and a note. It records that review happened; it does
/// not decide who was right.
pub fn close(item: &ReviewItem, who: &AgentId, what: &str) -> Result<Vec<Record>, ReviewError> {
    if what.trim().is_empty() {
        return Err(ReviewError::NoReason);
    }
    let (note_record, note_uid, attestation) = note(what, who)?;
    let resolution =
        Resolution::new(item.target(), who.clone(), Hlc::zero(who.clone())).with_note(note_uid);
    Ok(vec![
        note_record,
        Record::Attestation(attestation),
        Record::Resolution(resolution),
    ])
}

/// The unit a withdrawal or a resolution points at: the person's own words, attested by them.
fn attest_as(who: &AgentId) -> Attest {
    Attest::new(who.clone(), Rung::Document, Hlc::zero(who.clone()))
}

fn note(text: &str, who: &AgentId) -> Result<(Record, Uid, smysl::Attestation), ReviewError> {
    let gist = first_sentence(text);
    let schema = smysl::SchemaId::parse("claim").map_err(|e| ReviewError::Smysl(e.to_string()))?;
    let mut builder = UnitCoreBuilder::new(schema, &gist, smysl::Status::Speculative);
    if text.trim() != gist {
        builder = builder.body(text.trim());
    }
    let core = builder
        .build()
        .map_err(|e| ReviewError::Smysl(e.to_string()))?;
    let uid = smysl::canonical_uid(&core);
    Ok((Record::Unit(core), uid, attest_as(who).for_unit(uid)))
}

/// A gist is one sentence, inside smysl's L0 bound; the rest goes in the body.
fn first_sentence(text: &str) -> String {
    let text = text.trim();
    let end = text
        .find(". ")
        .map(|i| i + 1)
        .unwrap_or_else(|| text.len().min(110));
    let mut gist: String = text.chars().take(end).collect();
    if gist.len() < text.len() {
        gist = gist.trim_end().to_string();
    }
    gist
}
