//! Extraction → units, against smysl: statuses come from the commit's own text, edges follow the
//! data model, and every staged batch checks without errors.

use std::path::Path;

use cargo_smysl_corpus::{build, stage, CommitText, Extraction, Labels};
use smysl::{dependents_via, resolve_label, EdgeSet, Record, SourceKind, Status, Store};

const MESSAGE: &str = "\
dispatch: seven commands could have stopped working

cli() registers all twenty-two subcommands from the COMMANDS table unconditionally.
A list derived from the binary shrinks silently when the binary does.
make ci green";

const DIFF: &str = "fn scratch(name: &str) -> PathBuf { std::env::temp_dir().join(name) }";

fn extraction(prerequisite: &str) -> Extraction {
    serde_json::from_value(serde_json::json!({
        "decisions": [
            {"decision": "Add a test that runs every command", "kind": "act", "rationale": "Seven commands had no test.",
             "quote": "seven commands could have stopped working"},
            {"decision": "Hardcode the command list", "kind": "act", "rationale": "", "quote": "not in the text at all"}
        ],
        "prerequisites": [
            {"decision": 1, "text": prerequisite, "kind": "existing-behaviour",
             "quote": "cli() registers all twenty-two subcommands from the COMMANDS table unconditionally"},
            {"decision": 1, "text": "Each test gets its own scratch directory.", "kind": "assumption",
             "quote": "std::env::temp_dir().join(name)"},
            {"decision": 9, "text": "Names a decision that does not exist.", "kind": "assumption", "quote": ""}
        ],
        "alternatives": [
            {"decision": 2, "alternative": "Derive the list from the binary", "reason": "It shrinks silently.",
             "quote": "A list derived from the binary shrinks silently when the binary does"}
        ],
        "consequences": [
            {"decision": 1, "text": "Every command dispatches.", "quote": "seven commands could have stopped working",
             "verified": true, "verification_quote": "make ci green"},
            {"decision": 2, "text": "The list cannot shrink unnoticed.", "quote": "", "verified": false, "verification_quote": ""}
        ]
    }))
    .unwrap()
}

fn commit() -> CommitText<'static> {
    CommitText {
        touched: Vec::new(),
        sha: "90ec2f781421002876548124",
        message: MESSAGE,
        files: vec![("tests/dispatch.rs", DIFF)],
    }
}

fn staged_store(prerequisite: &str) -> (Store, cargo_smysl_corpus::Batch) {
    let batch = build(&extraction(prerequisite), &commit(), 0).unwrap();
    let quotes = batch.quotes;
    let dropped = batch.dropped.clone();
    let staged = stage(&Store::from_records(Vec::new()), batch, 0);
    assert!(
        !staged.has_errors(),
        "{:?}",
        staged.report.iter().collect::<Vec<_>>()
    );
    let records: Vec<Record> = staged.records();
    (
        Store::from_records(records),
        cargo_smysl_corpus::Batch {
            quotes,
            dropped,
            ..Default::default()
        },
    )
}

#[test]
fn statuses_and_sources_come_from_the_commit_text() {
    let (store, batch) = staged_store("cli() registers every command in COMMANDS.");
    let l = Labels::new("90ec2f781421002876548124", 0).unwrap();
    let unit = |label: &smysl::Label| {
        store
            .get(&resolve_label(&store, label).unwrap())
            .unwrap()
            .core
            .clone()
    };

    let d1 = unit(&l.decision(1));
    assert_eq!(d1.status, Status::Cited);
    let src = d1.source.clone().unwrap();
    assert_eq!(
        (src.kind, src.reference.as_str()),
        (SourceKind::Doc, "git:90ec2f781421002876548124")
    );

    assert_eq!(
        unit(&l.decision(2)).status,
        Status::Speculative,
        "a quote not in the text caps the unit"
    );
    assert!(unit(&l.decision(2)).source.is_none());

    let p2 = unit(&l.prerequisite(1, 2)).source.unwrap();
    assert_eq!(
        (p2.kind, p2.reference.as_str()),
        (SourceKind::File, "tests/dispatch.rs@90ec2f781421")
    );

    assert_eq!(
        unit(&l.consequence(1, 1)).status,
        Status::Cited,
        "a verified consequence is a cited finding"
    );
    assert_eq!(
        unit(&l.consequence(2, 1)).status,
        Status::Speculative,
        "an inference is never stronger than its decision"
    );

    assert_eq!(batch.dropped.len(), 1, "{:?}", batch.dropped);
    // One quote is checked and absent: decision 2's. The dropped prerequisite is never sourced, and an
    // unverified consequence is an inference with no quote check.
    assert_eq!(
        (batch.quotes.absent, batch.quotes.in_diff),
        (1, 1),
        "{:?}",
        batch.quotes
    );
}

#[test]
fn prerequisites_reach_their_decision_and_rewording_does_not_move_it() {
    let l = Labels::new("90ec2f781421002876548124", 0).unwrap();
    let (a, _) = staged_store("cli() registers every command in COMMANDS.");
    let (b, _) = staged_store("Every command in COMMANDS is registered by cli().");
    let p = resolve_label(&a, &l.prerequisite(1, 1)).unwrap();
    let d = resolve_label(&a, &l.decision(1)).unwrap();
    assert!(dependents_via(&a, p, &EdgeSet::premises()).contains(&d));
    assert_eq!(resolve_label(&b, &l.decision(1)).unwrap(), d);
}

/// The research extractions committed for S0, built with no commit text: every quote is then absent,
/// so every unit must degrade to speculative and the batch must still stage without errors.
#[test]
fn the_research_extractions_stage_without_errors() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../eval/extractions/research-pro-v2");
    let mut seen = 0;
    for repo in std::fs::read_dir(&dir).unwrap().flatten() {
        for file in std::fs::read_dir(repo.path()).unwrap().flatten() {
            let ex: Extraction =
                serde_json::from_str(&std::fs::read_to_string(file.path()).unwrap()).unwrap();
            let sha = file
                .path()
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let batch = build(
                &ex,
                &CommitText {
                    touched: Vec::new(),
                    sha: &sha,
                    message: "",
                    files: vec![],
                },
                0,
            )
            .unwrap();
            assert!(
                batch.units.iter().all(|u| u.status <= Status::Inferred),
                "{sha}: nothing may be cited without text"
            );
            let staged = stage(&Store::from_records(Vec::new()), batch, 0);
            assert!(
                !staged.has_errors(),
                "{sha}: {:?}",
                staged
                    .report
                    .iter()
                    .filter(|d| d.severity == smysl::Severity::Error)
                    .collect::<Vec<_>>()
            );
            seen += 1;
        }
    }
    assert_eq!(seen, 6, "the six studied commits");
}

/// A decision quoted from a file is linked to the items that file's commit changed (D4).
#[test]
fn a_decision_is_anchored_to_the_code_its_quote_came_from() {
    use cargo_smysl_corpus::{TouchedItem, REL_TOUCHES};

    let touched = vec![
        TouchedItem {
            path: "tests/dispatch.rs".into(),
            item: "tests::every_command_dispatches".into(),
            body_hash: "abcd1234".into(),
        },
        // Another file's item: no decision here quotes it, so nothing links to it.
        TouchedItem {
            path: "src/main.rs".into(),
            item: "main".into(),
            body_hash: "beef5678".into(),
        },
    ];
    let commit = CommitText {
        touched,
        sha: "90ec2f781421002876548124",
        message: MESSAGE,
        files: vec![("tests/dispatch.rs", DIFF)],
    };
    // One decision quoted from the file, one from the message: only the first can be anchored, because
    // only the first says where in the code it is about. A decision quoted from the message keeps the
    // commit as its scope, which is what `stale` reports for it.
    let ex: Extraction = serde_json::from_value(serde_json::json!({
        "decisions": [
            {"decision": "Give each test its own scratch directory", "kind": "act", "rationale": "",
             "quote": "std::env::temp_dir().join(name)"},
            {"decision": "Add a test that runs every command", "kind": "act", "rationale": "",
             "quote": "seven commands could have stopped working"}
        ],
        "prerequisites": [], "alternatives": [], "consequences": []
    }))
    .unwrap();
    let batch = build(&ex, &commit, 0).unwrap();

    let anchors: Vec<&smysl::UnitCore> = batch
        .units
        .iter()
        .filter(|u| format!("{}", u.schema).contains("artifact-ref"))
        .collect();
    assert_eq!(anchors.len(), 2, "one anchor per changed item");
    assert!(
        anchors
            .iter()
            .any(|u| u.gist.contains("every_command_dispatches")),
        "an anchor names the item: {:?}",
        anchors.iter().map(|u| &u.gist).collect::<Vec<_>>()
    );

    let touches = batch
        .relations
        .iter()
        .filter(|r| r.kind.to_string() == REL_TOUCHES)
        .count();
    assert_eq!(
        touches, 1,
        "the file-quoted decision is anchored to that file's item, and the message-quoted one to \
         nothing"
    );

    // The staged document still checks: anchors and their edges are part of the record, not beside it.
    let staged = stage(&Store::from_records(Vec::new()), batch, 0);
    assert!(!staged.has_errors(), "{:?}", staged.report);
}
