//! The review queue and what an answer records (D15): a person's attestation, a withdrawal, a
//! resolution. No model, no network.

use cargo_smysl_corpus::{build, stage, CommitText, Extraction};
use cargo_smysl_verdict::review::{close, confirm, describe, person, queue, reject};
use smysl::{Record, RelKind, Relation, Store, Uid};

const SHA: &str = "90ec2f781421002876548124e9fe02073503372c";

fn corpus() -> Store {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../eval/extractions/research-pro-v2/smysl/90ec2f7.json");
    let ex: Extraction = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let batch = build(
        &ex,
        &CommitText {
            touched: Vec::new(),
            sha: SHA,
            message: "",
            files: vec![],
        },
        0,
    )
    .unwrap();
    let staged = stage(&Store::from_records(Vec::new()), batch, 0);
    Store::from_records(staged.records())
}

/// A `backs` edge as the evidence crate will propose one: a model's claim that this test verifies that
/// prerequisite. It is exactly what a person must confirm (D13).
fn proposed_backs(store: &Store) -> (Store, Relation) {
    let mut uids: Vec<Uid> = store.units().map(|(u, _)| *u).collect();
    uids.sort();
    let edge = Relation::new(RelKind::Backs, uids[0], uids[1]);
    let mut records: Vec<Record> = store.iter().cloned().collect();
    records.push(Record::Relation(edge.clone()));
    (Store::from_records(records), edge)
}

#[test]
fn a_proposed_edge_waits_for_a_person() {
    let (store, edge) = proposed_backs(&corpus());
    let items = queue(&store);
    let waiting: Vec<String> = items.iter().filter(|i| !i.resolved).map(describe).collect();
    assert!(
        waiting
            .iter()
            .any(|d| d.contains("backs") && d.contains(&edge.from.short())),
        "the proposed edge is in the queue: {waiting:?}"
    );
}

#[test]
fn a_person_confirming_it_takes_it_out_of_the_queue() {
    let (store, _) = proposed_backs(&corpus());
    let item = queue(&store)
        .into_iter()
        .find(|i| !i.resolved && describe(i).contains("backs"))
        .expect("the edge is waiting");
    let who = person("owner").unwrap();

    let records = confirm(&item, &who).unwrap();
    assert_eq!(records.len(), 1, "one attestation, nothing rewritten");
    let mut all: Vec<Record> = store.iter().cloned().collect();
    all.extend(records);
    let after = Store::from_records(all);

    let still_waiting = queue(&after)
        .into_iter()
        .filter(|i| !i.resolved)
        .any(|i| describe(&i).contains("backs"));
    assert!(!still_waiting, "a person's attestation is what confirms it");
}

#[test]
fn a_model_attesting_its_own_proposal_is_not_a_review() {
    let (store, edge) = proposed_backs(&corpus());
    // The tool's own agent, not a person.
    let tool = smysl::AgentId::new(cargo_smysl_corpus::AGENT).unwrap();
    let attest = smysl::stage::Attest::new(
        tool.clone(),
        smysl::Rung::Computed,
        smysl::Hlc::zero(tool.clone()),
    );
    let mut all: Vec<Record> = store.iter().cloned().collect();
    all.push(Record::Attestation(attest.for_relation(edge.uid())));
    let after = Store::from_records(all);

    assert!(
        queue(&after)
            .into_iter()
            .any(|i| !i.resolved && describe(&i).contains("backs")),
        "it still waits for a person (D13)"
    );
}

#[test]
fn rejecting_an_edge_withdraws_it_and_says_why() {
    let (store, edge) = proposed_backs(&corpus());
    let item = queue(&store)
        .into_iter()
        .find(|i| !i.resolved && describe(i).contains("backs"))
        .unwrap();
    let who = person("owner").unwrap();

    assert!(reject(&item, &who, "   ").is_err(), "a reason is required");

    let records = reject(&item, &who, "The test asserts the opposite of the claim.").unwrap();
    assert_eq!(
        records.len(),
        3,
        "the reason, its attestation, the withdrawal"
    );
    let withdrawal = records
        .iter()
        .find_map(|r| match r {
            Record::Withdrawal(w) => Some(w),
            _ => None,
        })
        .expect("a withdrawal");
    assert_eq!(withdrawal.relation, edge.uid(), "it names the edge");
    assert!(withdrawal.reason.is_some(), "and the unit saying why");

    let mut all: Vec<Record> = store.iter().cloned().collect();
    all.extend(records);
    let after = Store::from_records(all);
    // The edge is still in the log — nothing is deleted — and no longer followed.
    assert!(
        after
            .iter()
            .any(|r| matches!(r, Record::Relation(r) if r.uid() == edge.uid())),
        "the edge stays in the log"
    );
    assert!(
        !after
            .relations_of_kind(&RelKind::Backs)
            .into_iter()
            .any(|r| r.uid() == edge.uid()),
        "a withdrawn edge is not followed"
    );
}

#[test]
fn closing_a_disagreement_records_that_review_happened() {
    let (store, _) = proposed_backs(&corpus());
    let item = queue(&store)
        .into_iter()
        .find(|i| !i.resolved)
        .expect("something is waiting");
    let who = person("owner").unwrap();

    let records = close(
        &item,
        &who,
        "Discussed with the author; the edge stands as written.",
    )
    .unwrap();
    assert!(
        records
            .iter()
            .any(|r| matches!(r, Record::Resolution(res) if res.note.is_some())),
        "a resolution with a note"
    );
    assert!(
        records.iter().any(|r| matches!(r, Record::Unit(_))),
        "and the note itself is a unit, not a comment in a file"
    );
}

#[test]
fn a_long_reason_keeps_its_first_sentence_as_the_gist() {
    let (store, _) = proposed_backs(&corpus());
    let item = queue(&store).into_iter().find(|i| !i.resolved).unwrap();
    let who = person("owner").unwrap();
    let why = "The link is wrong. The test exercises the parser, not the writer, and the claim is \
               about the writer's output, so nothing here bears on it at all.";
    let records = reject(&item, &who, why).unwrap();
    let unit = records
        .iter()
        .find_map(|r| match r {
            Record::Unit(u) => Some(u),
            _ => None,
        })
        .unwrap();
    assert_eq!(unit.gist, "The link is wrong.");
    assert!(unit.body.as_ref().is_some_and(|b| b.contains("parser")));
}
