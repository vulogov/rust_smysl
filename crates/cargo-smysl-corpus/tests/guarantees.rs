//! Guarantees the corpus relies on, checked on real extractions (two studied smysl commits):
//! identity survives surface and CBOR round trips, merge is idempotent and order-independent, and a
//! second extraction of the same commit never collides with the first.

use std::collections::BTreeSet;
use std::path::Path;

use cargo_smysl_corpus::{build, stage, CommitText, Extraction};
use smysl::{
    from_cbor_seq, merge, parse_surface, to_cbor_seq, MergeOptions, Record, Staged, Store, Uid,
};

fn extraction(sha: &str) -> Extraction {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../eval/extractions/research-pro-v2/smysl/{sha}.json"
    ));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn staged(ex: &Extraction, sha: &str, run: u32) -> Staged {
    let batch = build(
        ex,
        &CommitText {
            touched: Vec::new(),
            sha,
            message: "",
            files: vec![],
        },
        run,
    )
    .unwrap();
    let s = stage(&Store::from_records(Vec::new()), batch, 0);
    assert!(
        !s.has_errors(),
        "{sha} run {run}: {:?}",
        s.report.iter().collect::<Vec<_>>()
    );
    s
}

fn store(s: &Staged) -> Store {
    Store::from_records(s.records())
}

fn uids(store: &Store) -> BTreeSet<Uid> {
    store.units().map(|(u, _)| *u).collect()
}

fn edges(store: &Store) -> BTreeSet<(Uid, String, Uid)> {
    store
        .iter()
        .filter_map(|r| match r {
            Record::Relation(rel) => Some((rel.from, rel.kind.to_string(), rel.to)),
            _ => None,
        })
        .collect()
}

#[test]
fn surface_text_round_trips_to_the_same_units_edges_and_labels() {
    let s = staged(&extraction("90ec2f7"), "90ec2f7", 0);
    let original = store(&s);
    let reparsed = parse_surface(&s.to_surface()).expect("staged surface parses");
    let again = Store::from_records(reparsed.records);
    assert_eq!(
        uids(&again),
        uids(&original),
        "identity must survive the surface form"
    );
    assert_eq!(edges(&again), edges(&original));
    for (label, uid) in &s.labels {
        assert_eq!(
            reparsed.labels.get(label),
            Some(uid),
            "label {label} must bind the same unit"
        );
    }
}

#[test]
fn cbor_round_trips_byte_for_byte() {
    let records = staged(&extraction("532e4d2"), "532e4d2", 0).records();
    let bytes = to_cbor_seq(&records);
    let (decoded, _) = from_cbor_seq(&bytes).expect("decodes");
    assert_eq!(to_cbor_seq(&decoded), bytes);
}

#[test]
fn merge_is_idempotent_and_order_independent() {
    let a = store(&staged(&extraction("90ec2f7"), "90ec2f7", 0));
    let b = store(&staged(&extraction("532e4d2"), "532e4d2", 0));

    let mut ab = a.clone();
    let r = merge(&mut ab, &b, MergeOptions::default()).unwrap();
    assert!(
        r.new_contentions.is_empty(),
        "two commits must not contend: {:?}",
        r.new_contentions
    );
    let mut ba = b.clone();
    merge(&mut ba, &a, MergeOptions::default()).unwrap();
    assert_eq!(uids(&ab), uids(&ba));
    assert_eq!(edges(&ab), edges(&ba));

    let (before_units, before_edges) = (uids(&ab), edges(&ab));
    merge(&mut ab, &a, MergeOptions::default()).unwrap();
    assert_eq!(
        uids(&ab),
        before_units,
        "merging what is already there adds no unit"
    );
    assert_eq!(
        edges(&ab),
        before_edges,
        "merging what is already there adds no edge"
    );
}

/// Record-level idempotence (rule U at the record level). smysl 1.3 re-appended every LabelBinding and
/// SchemaDecl on each merge of a store that already held them; fixed in 1.4.0 (R10).
#[test]
fn merging_a_store_into_itself_adds_no_records() {
    let a = store(&staged(&extraction("90ec2f7"), "90ec2f7", 0));
    let mut aa = a.clone();
    let r = merge(&mut aa, &a, MergeOptions::default()).unwrap();
    assert_eq!(r.added, 0, "re-merging must append nothing");
    assert_eq!(aa.iter().count(), a.iter().count());
}

#[test]
fn a_second_extraction_of_the_same_commit_does_not_collide_with_the_first() {
    let ex = extraction("90ec2f7");
    let first = store(&staged(&ex, "90ec2f7", 0));

    // A reworded second run: different wording is a different unit, under run-qualified labels.
    let mut reworded = extraction("90ec2f7");
    reworded.prerequisites[0].text.push_str(" (reworded)");
    let second = store(&staged(&reworded, "90ec2f7", 1));

    let mut merged = first.clone();
    let r = merge(&mut merged, &second, MergeOptions::default()).unwrap();
    assert!(
        r.new_contentions.is_empty(),
        "runs must not produce label collisions: {:?}",
        r.new_contentions
    );
    assert!(
        uids(&merged).len() > uids(&first).len(),
        "the reworded prerequisite is a new unit"
    );
}

/// The constants this crate asserts on are valid, so CI finds a broken one rather than a person.
///
/// `build` and `Labels` parse fixed strings — the schema id, the relation kinds, the tool's agent id —
/// and treat failure as impossible. It is impossible only while smysl's rules stay as they are, and
/// this is what notices if they change.
#[test]
fn every_constant_this_crate_takes_for_granted_parses() {
    use cargo_smysl_corpus::{AGENT, CODE_SCHEMA_ID, REL_EXERCISES, REL_TOUCHES};

    assert!(
        smysl::SchemaId::parse(CODE_SCHEMA_ID).is_ok(),
        "{CODE_SCHEMA_ID} is the schema every unit is declared under"
    );
    for kind in [REL_TOUCHES, REL_EXERCISES] {
        assert!(
            smysl::RelKind::parse(kind).is_ok(),
            "{kind} is an edge this tool writes"
        );
    }
    assert!(
        smysl::AgentId::new(AGENT).is_ok(),
        "{AGENT} attests everything this tool records"
    );
    // And a label built from a real revision, which `Labels` also takes for granted.
    let labels = cargo_smysl_corpus::Labels::new("90ec2f781421002876548124", 0)
        .expect("a hex revision makes a label");
    assert_eq!(labels.decision(1).to_string(), "d/g90ec2f781421-1");
}
