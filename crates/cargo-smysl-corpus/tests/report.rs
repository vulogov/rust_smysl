//! Reading a commit's record back out: the grouping the labels already carry, and the statuses a
//! reader needs to weigh what they are reading.

use cargo_smysl_corpus::report::{as_markdown, as_text, commit_record};
use cargo_smysl_corpus::store::Corpus;
use cargo_smysl_corpus::{build, stage, CommitText, Extraction, TouchedItem};
use smysl::Store;

const MESSAGE: &str = "\
pin the dependency

Builds drifted between machines, so the version is pinned rather than floated.";

const DIFF: &str = "const VERSION: &str = \"1.2.3\";";

fn record() -> cargo_smysl_corpus::report::CommitRecord {
    let ex: Extraction = serde_json::from_value(serde_json::json!({
        "decisions": [
            {"decision": "Pin the dependency", "kind": "act",
             "rationale": "Builds drifted between machines.",
             "quote": "the version is pinned rather than floated"},
            {"decision": "Invent something", "kind": "act", "rationale": "",
             "quote": "nothing like this is in the commit"}
        ],
        "prerequisites": [
            {"decision": 1, "text": "the registry keeps that version", "kind": "assumption",
             "quote": "const VERSION: &str = \"1.2.3\";"}
        ],
        "alternatives": [
            {"decision": 1, "alternative": "Vendor it instead", "reason": "size",
             "quote": "Builds drifted between machines"}
        ],
        "consequences": [
            {"decision": 1, "text": "Two machines build the same thing.", "quote": "",
             "verified": false, "verification_quote": ""}
        ]
    }))
    .unwrap();
    let commit = CommitText {
        touched: vec![TouchedItem {
            path: "src/lib.rs".into(),
            item: "version".into(),
            body_hash: "abcd1234".into(),
        }],
        sha: "b2dbe4820872aaaabbbbcccc",
        message: MESSAGE,
        files: vec![("src/lib.rs", DIFF)],
    };
    let batch = build(&ex, &commit, 0).unwrap();
    let staged = stage(&Store::from_records(Vec::new()), batch, 0);
    let store = Store::from_records(staged.records());
    let labels = Corpus::at(std::env::temp_dir()).labels(&store);
    commit_record(&store, &labels, "b2dbe4820872")
}

#[test]
fn a_commit_reads_back_as_decisions_with_what_they_rest_on() {
    let record = record();
    let (d, p, r, q) = record.counts();
    assert_eq!((d, p, r, q), (2, 1, 1, 1), "{record:#?}");

    let first = &record.decisions[0];
    let item = first.item.as_ref().unwrap();
    assert_eq!(item.gist, "Pin the dependency");
    assert_eq!(item.status, "cited", "its quote is in the commit");
    assert!(item.body.contains("drifted"), "the rationale comes with it");
    assert_eq!(first.prerequisites.len(), 1);
    assert_eq!(first.alternatives.len(), 1);
    assert_eq!(first.consequences.len(), 1);

    // The second decision quoted something that is not in the commit (D8).
    let second = record.decisions[1].item.as_ref().unwrap();
    assert_eq!(second.status, "speculative");
    assert!(
        second.source.is_empty(),
        "and it has no source, because there is none"
    );
}

#[test]
fn what_a_reader_sees_says_which_lines_to_trust() {
    let record = record();
    let text = as_text(&record);
    assert!(text.contains("Pin the dependency"));
    assert!(
        text.contains("[speculative]"),
        "the status is on the line, not in a footnote: {text}"
    );
    assert!(text.contains("needs:") && text.contains("not:") && text.contains("so:"));

    let md = as_markdown(&record);
    assert!(md.starts_with("### Why this change"));
    assert!(md.contains("**Pin the dependency**"));
    assert!(md.contains("Rests on:") && md.contains("Turned down:"));
    assert!(
        md.contains("_(speculative)_"),
        "and it survives into Markdown: {md}"
    );
    assert!(
        md.contains("quoted something that is not in the commit"),
        "with a line saying what that means"
    );
}

#[test]
fn a_commit_with_nothing_recorded_is_empty_rather_than_wrong() {
    let store = Store::from_records(Vec::new());
    let labels = Corpus::at(std::env::temp_dir()).labels(&store);
    let record = commit_record(&store, &labels, "000000000000");
    assert!(record.is_empty());
    assert_eq!(record.counts(), (0, 0, 0, 0));
}
