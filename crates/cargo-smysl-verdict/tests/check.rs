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
