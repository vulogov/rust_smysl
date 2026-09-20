//! Phase 1's "done when" (docs/implementation-plan.md §5), on real extractions:
//! research outputs convert into a store that checks with 0 errors, and "what depends on prerequisite X"
//! returns the decision and its consequences.

use std::path::{Path, PathBuf};

use cargo_smysl_corpus::query::dependents_of;
use cargo_smysl_corpus::store::Corpus;
use cargo_smysl_corpus::{build, stage, CommitText, Extraction};
use smysl::{check, CheckOptions, GranularityProfile, Severity, Staged, Store};

/// The full revisions the labels are built from: a label carries `g<sha12>` (D5), so a test that used a
/// short sha would assert against labels no real run produces.
const SMYSL_90EC2F7: &str = "90ec2f781421002876548124e9fe02073503372c";
const SMYSL_532E4D2: &str = "532e4d229a6c1a1c56cd6a0d2f1e04e1b1bbd6b6";

fn extraction(system: &str, repo: &str, sha: &str) -> Extraction {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../eval/extractions/{system}/{repo}/{sha}.json"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn staged_against(store: &Store, ex: &Extraction, sha: &str) -> Staged {
    let batch = build(
        ex,
        &CommitText {
            touched: Vec::new(),
            sha,
            message: "",
            files: vec![],
        },
        0,
    )
    .unwrap();
    let s = stage(store, batch, 0);
    assert!(
        !s.has_errors(),
        "{sha}: {:?}",
        s.report.iter().collect::<Vec<_>>()
    );
    s
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cargo-smysl-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_recorded_commit_is_a_readable_document_and_a_store_that_checks() {
    let root = scratch("record");
    let corpus = Corpus::at(&root);
    let ex = extraction("research-pro-v2", "smysl", "90ec2f7");
    let staged = staged_against(&Store::from_records(Vec::new()), &ex, SMYSL_90EC2F7);

    let recorded = corpus.record(SMYSL_90EC2F7, &staged).unwrap();
    assert!(recorded.document.exists(), "the commit document is written");
    assert_eq!(recorded.contentions, 0);

    let text = std::fs::read_to_string(&recorded.document).unwrap();
    assert!(text.contains("@schema"), "the document declares x.code/v1");
    assert!(
        text.contains("d/g90ec2f781421-"),
        "and reads in its own labels"
    );

    // The store checks clean under the granularity the units were produced at (D4: `fine`).
    let store = corpus.load().unwrap();
    let mut opts = CheckOptions::default();
    opts.granularity = Some(GranularityProfile::fine());
    opts.labels = staged.labels.clone();
    let report = check(&store, opts);
    let errors: Vec<String> = report
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn recording_the_same_commit_twice_leaves_the_store_as_it_was() {
    let root = scratch("twice");
    let corpus = Corpus::at(&root);
    let ex = extraction("research-pro-v2", "smysl", "532e4d2");
    let staged = staged_against(&Store::from_records(Vec::new()), &ex, SMYSL_532E4D2);

    let first = corpus.record(SMYSL_532E4D2, &staged).unwrap();
    let second = corpus.record(SMYSL_532E4D2, &staged).unwrap();
    assert!(first.grew > 0, "the first recording adds the commit");
    assert_eq!(second.grew, 0, "the second adds nothing (rule U)");
    assert_eq!(second.added, 0);
    assert_eq!(first.records, second.records);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn the_store_rebuilds_from_the_commit_documents() {
    let root = scratch("rebuild");
    let corpus = Corpus::at(&root);
    let mut store = Store::from_records(Vec::new());
    for (short, sha) in [("90ec2f7", SMYSL_90EC2F7), ("532e4d2", SMYSL_532E4D2)] {
        let staged = staged_against(&store, &extraction("research-pro-v2", "smysl", short), sha);
        corpus.record(sha, &staged).unwrap();
        store = corpus.load().unwrap();
    }
    let recorded = corpus.load().unwrap();
    std::fs::remove_file(corpus.store_path()).unwrap();

    let rebuilt = corpus.rebuild().unwrap();
    let units = |s: &Store| -> Vec<smysl::Uid> { s.units().map(|(u, _)| *u).collect() };
    assert_eq!(
        units(&rebuilt),
        units(&recorded),
        "the documents are the record"
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn what_depends_on_a_prerequisite_is_its_decision_and_what_that_causes() {
    let root = scratch("depends");
    let corpus = Corpus::at(&root);
    let ex = extraction("research-pro-v2", "smysl", "90ec2f7");
    let staged = staged_against(&Store::from_records(Vec::new()), &ex, SMYSL_90EC2F7);
    corpus.record(SMYSL_90EC2F7, &staged).unwrap();
    let store = corpus.load().unwrap();

    // A prerequisite conditions its decision (D3), and the decision causes its consequences.
    let dependents = dependents_of(&store, "p/g90ec2f781421-8-1").unwrap();
    let labels: Vec<String> = dependents
        .iter()
        .flat_map(|d| d.labels.iter().map(|l| l.as_str().to_string()))
        .collect();
    assert!(
        labels.iter().any(|l| l == "d/g90ec2f781421-8"),
        "the decision it conditions: {labels:?}"
    );
    assert!(
        labels.iter().any(|l| l.starts_with("q/g90ec2f781421-8-")),
        "and what that decision causes: {labels:?}"
    );
    assert!(
        !labels.iter().any(|l| l == "p/g90ec2f781421-8-1"),
        "the subject is not its own dependent"
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn an_unknown_or_ambiguous_label_is_an_error_not_an_empty_answer() {
    let root = scratch("labels");
    let corpus = Corpus::at(&root);
    let ex = extraction("research-pro-v2", "smysl", "90ec2f7");
    let staged = staged_against(&Store::from_records(Vec::new()), &ex, SMYSL_90EC2F7);
    corpus.record(SMYSL_90EC2F7, &staged).unwrap();
    let store = corpus.load().unwrap();

    assert!(
        dependents_of(&store, "p/nosuch-1-1").is_err(),
        "unbound label"
    );
    assert!(
        dependents_of(&store, "not a label").is_err(),
        "malformed label"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// Phase 1's other "done when": the research extractions convert into a store that checks with 0
/// errors — every commit of every system, merged per repository as the tool would keep them.
#[test]
fn every_research_extraction_records_into_a_store_that_checks() {
    let eval = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../eval/extractions");
    let mut checked = 0;
    for system in [
        "research-pro-v2",
        "research-flash-v2",
        "research-deepseek-v2",
    ] {
        for repo in ["smysl", "ucal", "clap"] {
            let dir = eval.join(system).join(repo);
            if !dir.exists() {
                continue;
            }
            let root = scratch(&format!("all-{system}-{repo}"));
            let corpus = Corpus::at(&root);
            let mut store = Store::from_records(Vec::new());
            let mut labels = std::collections::BTreeMap::new();
            let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
                .unwrap()
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "json"))
                .collect();
            files.sort();
            for path in files {
                let short = path.file_stem().unwrap().to_string_lossy().to_string();
                // A label needs 12 hex characters; the research files are named by short sha.
                let sha = format!("{short}{}", "0".repeat(40 - short.len()));
                let ex: Extraction =
                    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
                let staged = staged_against(&store, &ex, &sha);
                labels.extend(staged.labels.clone());
                corpus.record(&sha, &staged).unwrap();
                store = corpus.load().unwrap();
            }
            let mut opts = CheckOptions::default();
            opts.granularity = Some(GranularityProfile::fine());
            opts.labels = labels;
            let errors: Vec<String> = check(&store, opts)
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .map(|d| d.to_string())
                .collect();
            assert!(
                errors.is_empty(),
                "{system}/{repo}: {:?}",
                &errors[..errors.len().min(3)]
            );
            checked += 1;
            std::fs::remove_dir_all(&root).ok();
        }
    }
    assert!(
        checked >= 7,
        "every system and repository with extractions was recorded"
    );
}
