//! The benchmark end to end, without a model: a label, an extraction, a pairing, a score.

use cargo_smysl_bench::{adjudication, extracted_items, score, sheet, Kind, LabelFile};

const LABEL: &str = r#"
schema = 1
repo = "demo"
commit = "abc123def456"
status = "done"

[[decision]]
id = "D1"
text = "Pin the dependency so builds are reproducible"
kind = "act"

[[decision]]
id = "D2"
text = "Do not vendor the dependency"
kind = "decline"

[[prerequisite]]
id = "P1"
decision = "D1"
text = "the registry keeps the pinned version available"
kind = "assumption"
"#;

/// What the tool extracted: one decision that is one of the labelled ones, one that is not, and a
/// prerequisite nobody labelled.
const EXTRACTION: &str = r#"{
  "decisions": [
    {"decision": "Pin the dependency for reproducible builds", "kind": "act"},
    {"decision": "The manifest lists the dependency", "kind": "act"}
  ],
  "prerequisites": [
    {"decision": 1, "text": "the crates.io index is reachable at build time"}
  ],
  "alternatives": []
}"#;

#[test]
fn a_label_and_an_extraction_become_a_score() {
    let labels: LabelFile = toml::from_str(LABEL).unwrap();
    labels.validate().expect("the label file holds together");
    let items = extracted_items(&serde_json::from_str(EXTRACTION).unwrap());
    assert_eq!(items.len(), 3);

    // Fresh: nothing is paired, so precision has nothing to say yet.
    let mut adj = adjudication("v1", &labels, &items, None);
    assert!(adj.items.iter().all(|i| i.matched.is_empty()));
    let pending = score(&labels, &adj).unwrap();
    assert_eq!(pending[&Kind::Decision].pending, 2);
    assert_eq!(pending[&Kind::Decision].precision(), None, "nothing judged");

    // The suggestion is word overlap, and it proposes the right decision first.
    assert_eq!(adj.items[0].suggest.first().map(String::as_str), Some("D1"));

    // A person pairs them: the first is D1, the second is not a decision at all, and the extracted
    // prerequisite is not the labelled one.
    adj.items[0].matched = "D1".into();
    adj.items[1].matched = "none".into();
    adj.items[2].matched = "none".into();

    let scored = score(&labels, &adj).unwrap();
    let d = scored[&Kind::Decision];
    assert_eq!((d.extracted, d.matched, d.labels, d.covered), (2, 1, 2, 1));
    assert_eq!(d.precision(), Some(0.5), "one of two extracted is real");
    assert_eq!(d.recall(), Some(0.5), "one of two labelled was found");
    let p = scored[&Kind::Prerequisite];
    assert_eq!(
        p.precision(),
        Some(0.0),
        "the one extracted is not the one labelled"
    );
    assert_eq!(p.recall(), Some(0.0));

    // A pairing already made survives the file being rebuilt.
    let again = adjudication("v1", &labels, &items, Some(&adj));
    assert_eq!(again.items[0].matched, "D1");
}

#[test]
fn an_adjudication_naming_a_label_that_does_not_exist_is_an_error() {
    let labels: LabelFile = toml::from_str(LABEL).unwrap();
    let items = extracted_items(&serde_json::from_str(EXTRACTION).unwrap());
    let mut adj = adjudication("v1", &labels, &items, None);
    adj.items[0].matched = "D9".into();
    assert!(score(&labels, &adj).is_err());
}

#[test]
fn the_sheet_holds_the_commit_and_never_an_extraction() {
    let files = vec![(
        "src/lib.rs".to_string(),
        "+ pinned = \"1.2.3\"\n".repeat(600),
    )];
    let reading = sheet::reading(
        "demo",
        "abc123",
        "Pin it\n\nBecause builds drifted.",
        &files,
        400,
    );
    assert!(
        reading.contains("Because builds drifted"),
        "the message is there"
    );
    assert!(
        reading.contains("200 more line(s)"),
        "and a long file says what was cut"
    );
    assert!(
        reading.contains("do not read what the tool extracted"),
        "labelling blind is stated where it is read"
    );
    let template = sheet::template("demo", "abc123", "Pin it", &["src/lib.rs".into()]);
    assert!(template.contains("status = \"todo\""));
    assert!(
        template.contains("not the motivation"),
        "the boundary the models keep crossing is in front of the labeller"
    );
}
