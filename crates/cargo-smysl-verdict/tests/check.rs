//! The deterministic half of `check`, on a real corpus and without a model: what it asks about, what it
//! accepts as an answer, and what it refuses.

use std::cell::RefCell;
use std::path::Path;

use cargo_smysl_corpus::{build, stage, CommitText, Extraction};
use cargo_smysl_verdict::check::{check_agreed, AGREEMENT_SEEDS};
use cargo_smysl_verdict::{check, Change, Charged, Judge, JudgeError, RawVerdict, Settings};
use smysl::Store;

const SHA: &str = "90ec2f781421002876548124e9fe02073503372c";

/// A judge that answers from a script, and records what it was asked.
struct Scripted {
    answers: RefCell<Vec<Vec<RawVerdict>>>,
    asked: RefCell<Vec<String>>,
}

impl Scripted {
    fn new(answers: Vec<Vec<RawVerdict>>) -> Scripted {
        Scripted {
            answers: RefCell::new(answers),
            asked: RefCell::new(Vec::new()),
        }
    }
}

impl Judge for Scripted {
    fn describe(&self) -> String {
        "scripted".into()
    }
    // The trait's primitive is the model's text, so a scripted judge answers as a model would.
    fn ask_text(&self, _system: &str, user: &str) -> Result<(String, Charged), JudgeError> {
        self.asked.borrow_mut().push(user.to_string());
        let mut answers = self.answers.borrow_mut();
        let next = if answers.is_empty() {
            Vec::new()
        } else {
            answers.remove(0)
        };
        let json = serde_json::json!({"verdicts": next.iter().map(|v: &RawVerdict| serde_json::json!({
            "label": v.label, "verdict": v.verdict, "line": v.line,
            "diff_line": v.diff_line, "reason": v.reason,
        })).collect::<Vec<_>>()});
        Ok((json.to_string(), Charged::default()))
    }
}

fn corpus() -> Store {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
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

/// A raw string: a `\` continuation would strip the leading space a context line needs.
const DIFF: &str = r#"diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,4 +1,4 @@
 fn cmd_check(m: &ArgMatches) -> ExitCode {
-    let files = m.get_many("files").unwrap_or_else(|| vec!["-"]);
+    let files = m.get_many("files").unwrap_or_else(|| vec![".smysl/store"]);
     run(files)
 }
"#;

fn verdict(label: &str, line: u32, text: &str) -> RawVerdict {
    RawVerdict {
        label: label.into(),
        verdict: "contradicts".into(),
        line: Some(line),
        diff_line: text.into(),
        reason: "the default is no longer stdin".into(),
    }
}

#[test]
fn it_asks_about_recorded_units_and_shows_the_change_with_numbered_lines() {
    let store = corpus();
    let settings = Settings::default();
    let change = Change::from_diff(DIFF, &settings);
    let judge = Scripted::new(vec![]);
    let outcome = check(&store, &change, &judge, &settings);

    let asked = judge.asked.borrow();
    assert!(!asked.is_empty(), "the judge was asked something");
    assert!(asked[0].contains("RECORDED UNITS:"), "{}", asked[0]);
    assert!(
        asked[0].contains("JUDGE: "),
        "the labels it must answer for"
    );
    let numbered = asked[0]
        .lines()
        .find(|l| l.contains(".smysl/store"))
        .expect("the change is shown");
    assert!(
        numbered
            .split('|')
            .next()
            .is_some_and(|n| n.trim().parse::<u32>().is_ok()),
        "each line carries the number a verdict quotes by: {numbered:?}"
    );
    assert!(outcome.units_judged > 0);
    assert_eq!(outcome.findings.len(), 0, "an empty answer finds nothing");
}

#[test]
fn a_verdict_stands_when_its_label_was_judged_and_its_line_is_in_the_change() {
    let store = corpus();
    let settings = Settings::default();
    let change = Change::from_diff(DIFF, &settings);
    // Ask once to learn which labels this change puts in front of the model.
    let probe = Scripted::new(vec![]);
    check(&store, &change, &probe, &settings);
    let asked = probe.asked.borrow()[0].clone();
    let judged: Vec<String> = asked
        .lines()
        .find(|l| l.starts_with("JUDGE: "))
        .unwrap()
        .trim_start_matches("JUDGE: ")
        .split(", ")
        .map(str::to_string)
        .collect();
    let line_no = asked
        .lines()
        .find(|l| l.contains("vec![\".smysl/store\"]"))
        .and_then(|l| l.split('|').next())
        .and_then(|n| n.trim().parse::<u32>().ok())
        .expect("the added line is numbered");

    let judge = Scripted::new(vec![vec![verdict(&judged[0], line_no, "")]]);
    let outcome = check(&store, &change, &judge, &settings);
    assert_eq!(outcome.findings.len(), 1, "{:?}", outcome.dropped);
    let f = &outcome.findings[0];
    assert_eq!(f.label, judged[0]);
    assert!(
        f.diff_line.contains(".smysl/store"),
        "quoted by number: {f:?}"
    );
    // The unit as recorded. `source` is empty for a speculative unit, whose quote was not found in the
    // commit it came from (D8), so only the text is guaranteed.
    assert!(!f.text.is_empty(), "the unit as recorded: {f:?}");
}

#[test]
fn a_verdict_is_refused_when_the_label_was_not_judged_or_the_quote_is_invented() {
    let store = corpus();
    let settings = Settings::default();
    let change = Change::from_diff(DIFF, &settings);
    let probe = Scripted::new(vec![]);
    check(&store, &change, &probe, &settings);
    let judged = probe.asked.borrow()[0]
        .lines()
        .find(|l| l.starts_with("JUDGE: "))
        .unwrap()
        .trim_start_matches("JUDGE: ")
        .split(", ")
        .next()
        .unwrap()
        .to_string();

    let judge = Scripted::new(vec![vec![
        verdict("d/g0000000000-9", 5, "let files = m.get_many"),
        RawVerdict {
            label: judged,
            verdict: "contradicts".into(),
            line: None,
            diff_line: "a line that is not in the change".into(),
            reason: "invented".into(),
        },
    ]]);
    let outcome = check(&store, &change, &judge, &settings);
    assert!(outcome.findings.is_empty(), "{:?}", outcome.findings);
    assert_eq!(outcome.dropped.len(), 2, "{:?}", outcome.dropped);
    assert!(outcome
        .dropped
        .iter()
        .any(|d| d.contains("not a unit this run judged")));
    assert!(outcome
        .dropped
        .iter()
        .any(|d| d.contains("neither the line number nor the quote")));
}

#[test]
fn an_unreadable_answer_is_reported_not_swallowed() {
    struct Broken;
    impl Judge for Broken {
        fn describe(&self) -> String {
            "broken".into()
        }
        fn ask_text(&self, _s: &str, _u: &str) -> Result<(String, Charged), JudgeError> {
            Err(JudgeError::Shape("expected struct Verdict".into()))
        }
    }
    let store = corpus();
    let settings = Settings::default();
    let outcome = check(
        &store,
        &Change::from_diff(DIFF, &settings),
        &Broken,
        &settings,
    );
    assert!(outcome.findings.is_empty());
    assert!(
        outcome
            .warnings
            .iter()
            .any(|w| w.contains("unreadable shape")),
        "{:?}",
        outcome.warnings
    );
}

#[test]
fn findings_are_advisory_unless_strict() {
    let store = corpus();
    let settings = Settings::default();
    let change = Change::from_diff(DIFF, &settings);
    let probe = Scripted::new(vec![]);
    check(&store, &change, &probe, &settings);
    let asked = probe.asked.borrow()[0].clone();
    let judged = asked
        .lines()
        .find(|l| l.starts_with("JUDGE: "))
        .unwrap()
        .trim_start_matches("JUDGE: ")
        .split(", ")
        .next()
        .unwrap()
        .to_string();
    let line_no = asked
        .lines()
        .find(|l| l.contains("vec![\".smysl/store\"]"))
        .and_then(|l| l.split('|').next())
        .and_then(|n| n.trim().parse::<u32>().ok())
        .unwrap();

    let judge = Scripted::new(vec![vec![verdict(&judged, line_no, "")]]);
    let outcome = check(&store, &change, &judge, &settings);
    assert_eq!(outcome.findings.len(), 1);
    assert_eq!(outcome.exit_code(false), 0, "advisory by default");
    assert_eq!(outcome.exit_code(true), 5, "--strict is the gate");
}

#[test]
fn a_unified_diff_is_built_from_the_two_texts() {
    let before = "fn main() {\n    println!(\"a\");\n}\n";
    let after = "fn main() {\n    println!(\"b\");\n}\n";
    let diff = cargo_smysl_verdict::check::unified("src/main.rs", before, after, 3);
    assert!(
        diff.starts_with("diff --git a/src/main.rs b/src/main.rs"),
        "{diff}"
    );
    assert!(diff.contains("-    println!(\"a\");"), "{diff}");
    assert!(diff.contains("+    println!(\"b\");"), "{diff}");

    let settings = Settings::default();
    let change = Change::from_diff(&diff, &settings);
    assert_eq!(change.files, vec!["src/main.rs".to_string()]);
    assert!(cargo_smysl_verdict::check::unified("x.rs", before, before, 3).is_empty());
}

#[test]
fn a_finding_only_one_pass_reports_is_not_kept() {
    let store = corpus();
    let settings = Settings::default();
    let change = Change::from_diff(DIFF, &settings);
    let probe = Scripted::new(vec![]);
    check(&store, &change, &probe, &settings);
    let asked = probe.asked.borrow()[0].clone();
    let judged: Vec<String> = asked
        .lines()
        .find(|l| l.starts_with("JUDGE: "))
        .unwrap()
        .trim_start_matches("JUDGE: ")
        .split(", ")
        .map(str::to_string)
        .collect();
    let line_no = asked
        .lines()
        .find(|l| l.contains("vec![\".smysl/store\"]"))
        .and_then(|l| l.split('|').next())
        .and_then(|n| n.trim().parse::<u32>().ok())
        .unwrap();

    // Two passes: the first call of each pass answers, the rest find nothing. The two passes see the
    // units in different company, so they name different labels — and neither survives agreement.
    let both = Scripted::new(vec![
        vec![verdict(&judged[0], line_no, "")],
        vec![],
        vec![],
        vec![],
        vec![verdict(&judged[1], line_no, "")],
        vec![],
        vec![],
        vec![],
    ]);
    let outcome = check_agreed(&store, &change, &both, &settings, &AGREEMENT_SEEDS);
    assert!(
        outcome.findings.is_empty(),
        "one pass each: {:?}",
        outcome.findings
    );
    assert!(
        outcome.dropped.iter().any(|d| d.contains("only one pass")),
        "and it says so: {:?}",
        outcome.dropped
    );

    // The same label from both passes stands.
    let agreeing = Scripted::new(vec![
        vec![verdict(&judged[0], line_no, "")],
        vec![],
        vec![],
        vec![],
        vec![verdict(&judged[0], line_no, "")],
        vec![],
        vec![],
        vec![],
    ]);
    let outcome = check_agreed(&store, &change, &agreeing, &settings, &AGREEMENT_SEEDS);
    assert_eq!(outcome.findings.len(), 1, "{:?}", outcome.dropped);
    assert_eq!(outcome.findings[0].label, judged[0]);
}

/// A change too large to show is reported by name: which files the model never saw.
#[test]
fn what_was_not_examined_is_named() {
    // Three files, and a cap that cannot reach the third.
    let mut diff = String::new();
    for n in 1..=3 {
        diff.push_str(&format!(
            "diff --git a/src/f{n}.rs b/src/f{n}.rs\n--- a/src/f{n}.rs\n+++ b/src/f{n}.rs\n"
        ));
        for line in 0..60 {
            diff.push_str(&format!("+    let v{n}_{line} = {line};\n"));
        }
    }
    let settings = Settings {
        diff_lines: 70,
        ..Settings::default()
    };
    let change = Change::from_diff(&diff, &settings);
    assert!(change.truncated, "the cap is reached");
    assert_eq!(
        change.unexamined(),
        vec!["src/f3.rs".to_string()],
        "the file no line of which was shown is named, and the others are not"
    );

    let store = corpus();
    let judge = Scripted::new(vec![]);
    let outcome = check(&store, &change, &judge, &settings);
    assert_eq!(outcome.unexamined, vec!["src/f3.rs".to_string()]);
    assert!(
        outcome
            .warnings
            .iter()
            .any(|w| w.contains("not examined at all") && w.contains("src/f3.rs")),
        "and the warning names it: {:?}",
        outcome.warnings
    );
}

/// A change that fits leaves nothing unexamined, and says nothing about it.
#[test]
fn a_change_that_fits_reports_no_unexamined_files() {
    let store = corpus();
    let settings = Settings::default();
    let change = Change::from_diff(DIFF, &settings);
    assert!(!change.truncated);
    assert!(change.unexamined().is_empty());
    let outcome = check(&store, &change, &Scripted::new(vec![]), &settings);
    assert!(outcome.unexamined.is_empty());
    assert!(
        !outcome.warnings.iter().any(|w| w.contains("not examined")),
        "{:?}",
        outcome.warnings
    );
}

/// When only part of a change fits, the files the corpus knows about are the ones shown.
#[test]
fn a_cut_change_shows_what_the_corpus_knows_about() {
    // A corpus whose units are anchored to a file: quotes that are in the file get `path@sha` sources,
    // which is what "the corpus knows about this file" means.
    let store = {
        let ex: Extraction = serde_json::from_value(serde_json::json!({
            "decisions": [{"decision": "Give each test its own scratch directory", "kind": "act",
                           "rationale": "", "quote": "vec![\".smysl/store\"]"}],
            "prerequisites": [], "alternatives": [], "consequences": []
        }))
        .unwrap();
        let batch = build(
            &ex,
            &CommitText {
                touched: Vec::new(),
                sha: SHA,
                message: "",
                files: vec![("tests/dispatch.rs", "let scratch = vec![\".smysl/store\"];")],
            },
            0,
        )
        .unwrap();
        let staged = stage(&Store::from_records(Vec::new()), batch, 0);
        Store::from_records(staged.records())
    };
    assert_eq!(
        store.units_with_source_prefix("tests/dispatch.rs@").len(),
        1,
        "the corpus is anchored to that file"
    );
    // The corpus for this test was built from a commit touching tests/dispatch.rs. Put that file last
    // in the diff, behind enough unrelated lines to push it out of view.
    let mut diff = String::new();
    for n in 1..=3 {
        diff.push_str(&format!(
            "diff --git a/src/unrelated{n}.rs b/src/unrelated{n}.rs\n--- a/src/unrelated{n}.rs\n+++ b/src/unrelated{n}.rs\n"
        ));
        for line in 0..40 {
            diff.push_str(&format!("+    let x{n}_{line} = {line};\n"));
        }
    }
    diff.push_str("diff --git a/tests/dispatch.rs b/tests/dispatch.rs\n--- a/tests/dispatch.rs\n+++ b/tests/dispatch.rs\n");
    diff.push_str("+    let scratch = vec![\".smysl/store\"];\n");

    let settings = Settings {
        diff_lines: 50,
        ..Settings::default()
    };
    let change = Change::from_diff(&diff, &settings);
    assert!(change.truncated);
    assert!(
        change
            .unexamined()
            .contains(&"tests/dispatch.rs".to_string()),
        "in diff order the file the corpus knows about is never reached"
    );

    let weight = |path: &str| store.units_with_source_prefix(&format!("{path}@")).len();
    let ordered = change.ordered_by(&weight, &settings);
    assert!(
        !ordered
            .unexamined()
            .contains(&"tests/dispatch.rs".to_string()),
        "ordered by what the corpus knows, it is shown: unexamined {:?}",
        ordered.unexamined()
    );
    assert_eq!(
        ordered.files.len(),
        change.files.len(),
        "and no file is lost from the report, only from the view"
    );

    // `check` does this itself when a change did not fit, so no caller has to remember to.
    let probe = Scripted::new(vec![]);
    let outcome = check(&store, &change, &probe, &settings);
    assert!(
        !outcome
            .unexamined
            .contains(&"tests/dispatch.rs".to_string()),
        "check reorders before judging: {:?}",
        outcome.unexamined
    );
}

/// A change too large to show is read in parts, and a file unseen in one part may be seen in another.
#[test]
fn a_change_read_in_parts_examines_what_one_view_could_not() {
    let store = corpus();
    let mut diff = String::new();
    for n in 1..=4 {
        diff.push_str(&format!(
            "diff --git a/src/f{n}.rs b/src/f{n}.rs\n--- a/src/f{n}.rs\n+++ b/src/f{n}.rs\n"
        ));
        for line in 0..40 {
            diff.push_str(&format!("+    let v{n}_{line} = {line};\n"));
        }
    }
    let one = Settings {
        diff_lines: 50,
        ..Settings::default()
    };
    let change = Change::from_diff(&diff, &one);
    assert!(change.truncated);
    let unseen_with_one = change.unexamined().len();
    assert!(unseen_with_one >= 2, "one view cannot reach them all");

    let many = Settings {
        max_parts: 4,
        ..one.clone()
    };
    let parts = change.parts(&many);
    assert!(parts.len() > 1, "it is read in parts: {}", parts.len());
    for part in &parts {
        assert!(
            !part.files.is_empty(),
            "each part carries whole files of the change"
        );
    }
    let examined_by_parts: std::collections::BTreeSet<String> = parts
        .iter()
        .flat_map(|p| {
            p.files
                .iter()
                .filter(|f| !p.unexamined().contains(*f))
                .cloned()
        })
        .collect();
    assert!(
        examined_by_parts.len() > change.files.len() - unseen_with_one,
        "more files are examined across parts than in one view: {examined_by_parts:?}"
    );

    // And `check` reports the reading, and counts each part's calls.
    let judge = Scripted::new(vec![]);
    let outcome = check(&store, &change, &judge, &many);
    assert!(
        outcome.warnings.iter().any(|w| w.contains("read in")),
        "the parts are reported: {:?}",
        outcome.warnings
    );
    // Nothing here is anchored to these files, so no part had anything to judge — and that is the
    // honest outcome, not a failure. What parts change is coverage: every file was examined by some
    // part, so nothing is left named as unexamined.
    assert!(
        outcome.unexamined.is_empty(),
        "read in parts, no file is left unexamined: {:?}",
        outcome.unexamined
    );
    assert!(outcome
        .warnings
        .iter()
        .any(|w| w.contains("nothing recorded bears on")));
}

/// One part is the old behaviour exactly: what fits, and the rest named.
#[test]
fn one_part_is_what_fits_and_the_rest_named() {
    let store = corpus();
    let settings = Settings {
        diff_lines: 50,
        ..Settings::default()
    };
    let mut diff = String::new();
    for n in 1..=3 {
        diff.push_str(&format!(
            "diff --git a/src/f{n}.rs b/src/f{n}.rs\n--- a/src/f{n}.rs\n+++ b/src/f{n}.rs\n"
        ));
        for line in 0..40 {
            diff.push_str(&format!("+    let v{n}_{line} = {line};\n"));
        }
    }
    let change = Change::from_diff(&diff, &settings);
    assert_eq!(change.parts(&settings).len(), 1);
    let outcome = check(&store, &change, &Scripted::new(vec![]), &settings);
    assert!(
        !outcome.unexamined.is_empty(),
        "and it says what it skipped"
    );
    assert!(!outcome.warnings.iter().any(|w| w.contains("read in")));
}
