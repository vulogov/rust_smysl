//! Extraction against a scripted model: what the passes ask, what they keep, and what they refuse.
//! No network, no tokens.

use std::cell::RefCell;

use cargo_smysl_extract::{extract, Cache, Charged, Judge, JudgeError, Recipe};

struct Scripted {
    answers: RefCell<Vec<String>>,
    asked: RefCell<Vec<(String, String)>>,
}

impl Scripted {
    fn new(answers: &[&str]) -> Scripted {
        Scripted {
            answers: RefCell::new(answers.iter().map(|s| s.to_string()).collect()),
            asked: RefCell::new(Vec::new()),
        }
    }
}

/// A scripted judge that also states a window, so the splitting can be tested without a model.
struct Sized {
    inner: Scripted,
    chars: usize,
}

impl Sized {
    fn new(chars: usize, answers: &[&str]) -> Sized {
        Sized {
            inner: Scripted::new(answers),
            chars,
        }
    }
}

impl std::ops::Deref for Sized {
    type Target = Scripted;
    fn deref(&self) -> &Scripted {
        &self.inner
    }
}

impl Judge for Sized {
    fn describe(&self) -> String {
        self.inner.describe()
    }
    fn ask_text(&self, system: &str, user: &str) -> Result<(String, Charged), JudgeError> {
        self.inner.ask_text(system, user)
    }
    fn input_chars(&self) -> Option<usize> {
        Some(self.chars)
    }
}

impl Judge for Scripted {
    fn describe(&self) -> String {
        "scripted".into()
    }
    fn ask_text(&self, system: &str, user: &str) -> Result<(String, Charged), JudgeError> {
        self.asked
            .borrow_mut()
            .push((system.to_string(), user.to_string()));
        let mut answers = self.answers.borrow_mut();
        if answers.is_empty() {
            return Err(JudgeError::Shape("the script ran out".into()));
        }
        Ok((answers.remove(0), Charged::default()))
    }
}

const COMMIT: &str = "Keep the ledger beside the store\n\n\
The usage ledger is resolved against the working directory, so a shared checkout is not dirtied.\n\n\
diff --git a/src/main.rs b/src/main.rs\n\
-    let ledger = home().join(\".smysl/usage.log\");\n\
+    let ledger = root_beside(store).join(\"usage.log\");\n";

const DECISIONS: &str = r#"{"decisions":[
  {"decision":"Resolve the usage ledger beside the store","kind":"act",
   "rationale":"a shared checkout must not be dirtied","quote":"resolved against the working directory"},
  {"decision":"","kind":"act","rationale":"","quote":""}
]}"#;

const ITEMS: &str = r#"{"prerequisites":[
  {"text":"ingest and usage write under the working directory","kind":"existing-behaviour",
   "normative":false,"quote":"resolved against the working directory"},
  {"text":"","kind":"invariant","normative":true,"quote":""}
 ],
 "alternatives":[{"alternative":"Keep the ledger in the user's home directory","quote":"home()"}],
 "consequences":[{"consequence":"a shared checkout stays clean","verified":true,
   "verification_quote":"root_beside(store)","quote":"not dirtied"}]}"#;

#[test]
fn decisions_come_first_and_each_one_is_asked_about_on_its_own() {
    let judge = Scripted::new(&[DECISIONS, ITEMS]);
    let (extraction, report) = extract(COMMIT, &judge, &Recipe::default()).unwrap();

    let asked = judge.asked.borrow();
    assert_eq!(
        asked.len(),
        2,
        "one pass for decisions, one for the decision's items"
    );
    assert!(
        asked[0].0.contains("report the decisions"),
        "the first pass asks for decisions"
    );
    assert!(
        asked[0].1.contains("Keep the ledger beside the store"),
        "and shows the commit"
    );
    assert!(
        asked[1]
            .1
            .contains("DECISION 1: Resolve the usage ledger beside the store"),
        "the second pass names the decision it is asking about:\n{}",
        asked[1].1
    );
    assert!(asked[1]
        .1
        .contains("RATIONALE: a shared checkout must not be dirtied"));

    assert_eq!(
        extraction.decisions.len(),
        1,
        "the empty decision is dropped"
    );
    assert_eq!(
        extraction.prerequisites.len(),
        1,
        "and so is the empty prerequisite"
    );
    assert_eq!(report.empty, 2);
    assert_eq!(report.calls, 2);
}

#[test]
fn every_item_names_the_decision_it_belongs_to_and_keeps_its_quote() {
    let judge = Scripted::new(&[DECISIONS, ITEMS]);
    let (extraction, _) = extract(COMMIT, &judge, &Recipe::default()).unwrap();

    let p = &extraction.prerequisites[0];
    assert_eq!(p.decision, 1);
    assert_eq!(p.kind, "existing-behaviour");
    assert!(!p.normative);
    assert!(
        p.quote.contains("working directory"),
        "the quote the corpus will check (D8)"
    );

    assert_eq!(extraction.alternatives[0].decision, 1);
    assert!(extraction.alternatives[0]
        .alternative
        .contains("home directory"));

    let c = &extraction.consequences[0];
    assert_eq!(c.decision, 1);
    assert!(c.verified, "verified only when the commit shows it");
    assert!(c.verification_quote.contains("root_beside"));
}

#[test]
fn what_the_model_returns_is_content_only_the_tool_assigns_the_rest() {
    // D5: the extraction carries no labels, sources or statuses — the corpus assigns those from the
    // commit it read. Building it proves the shapes meet.
    let judge = Scripted::new(&[DECISIONS, ITEMS]);
    let (extraction, _) = extract(COMMIT, &judge, &Recipe::default()).unwrap();
    let batch = cargo_smysl_corpus::build(
        &extraction,
        &cargo_smysl_corpus::CommitText {
            sha: "90ec2f781421002876548124e9fe02073503372c",
            message: COMMIT,
            files: vec![],
        },
        0,
    )
    .unwrap();
    assert!(
        batch.units.len() >= 4,
        "decision, prerequisite, alternative, consequence"
    );
    assert!(batch
        .labels
        .keys()
        .any(|l| l.as_str().starts_with("d/g90ec2f781421-")));
    // The quote that is in the commit is found; the one that is not caps its unit at speculative.
    assert!(batch.quotes.present + batch.quotes.loose > 0);
}

#[test]
fn a_decision_whose_items_fail_still_stands() {
    // One failed call is not a failed extraction: the decision was read, its items were not.
    let judge = Scripted::new(&[DECISIONS]);
    let (extraction, report) = extract(COMMIT, &judge, &Recipe::default()).unwrap();
    assert_eq!(extraction.decisions.len(), 1);
    assert!(extraction.prerequisites.is_empty());
    assert!(
        report.warnings.iter().any(|w| w.contains("no items")),
        "{:?}",
        report.warnings
    );
}

#[test]
fn a_first_pass_that_fails_is_an_error_not_an_empty_extraction() {
    let judge = Scripted::new(&["not json at all"]);
    assert!(
        extract(COMMIT, &judge, &Recipe::default()).is_err(),
        "an unreadable first answer must not read as a commit with no decisions"
    );
}

#[test]
fn a_long_commit_is_cut_and_says_so() {
    let judge = Scripted::new(&[DECISIONS, ITEMS]);
    let recipe = Recipe {
        max_input: 120,
        ..Recipe::default()
    };
    let (_, report) = extract(COMMIT, &judge, &recipe).unwrap();
    assert!(judge.asked.borrow()[0].1.len() < COMMIT.len() + 40);
    assert!(
        report.warnings.iter().any(|w| w.contains("the first 120")),
        "{:?}",
        report.warnings
    );
}

#[test]
fn a_commit_is_extracted_once_per_recipe() {
    let root = std::env::temp_dir().join(format!("cargo-smysl-extract-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cache = Cache::at(&root);
    let recipe = Recipe::default();
    let sha = "90ec2f781421002876548124e9fe02073503372c";

    assert!(cache.read(sha, &recipe).is_none(), "nothing yet");
    let judge = Scripted::new(&[DECISIONS, ITEMS]);
    let (extraction, _) = extract(COMMIT, &judge, &recipe).unwrap();
    cache.write(sha, &recipe, &extraction).unwrap();

    let again = cache.read(sha, &recipe).expect("the extraction is kept");
    assert_eq!(again.decisions.len(), extraction.decisions.len());
    assert_eq!(
        again.prerequisites[0].text,
        extraction.prerequisites[0].text
    );

    // Another recipe is another entry: an improvement is a new recipe, never a re-extraction merged
    // into the old one (D7).
    let other = Recipe {
        name: "tighter-prompt".into(),
        ..Recipe::default()
    };
    assert!(cache.read(sha, &other).is_none());
    assert_ne!(cache.path(sha, &recipe), cache.path(sha, &other));
    std::fs::remove_dir_all(&root).ok();
}

/// An answer the model ran out of room to finish keeps everything it managed to say.
#[test]
fn an_answer_cut_off_mid_item_is_read_up_to_its_last_whole_one() {
    // Two complete decisions, then a third the model never finished: the string is still open.
    let cut = r#"{"decisions": [
        {"decision": "Pin the dependency", "kind": "act", "rationale": "reproducible builds", "quote": "pin"},
        {"decision": "Do not vendor it", "kind": "decline", "rationale": "size", "quote": "vendor"},
        {"decision": "Rewrite the parser", "kind": "act", "rationale": "the old one cannot rep"#;
    let judge = Scripted::new(&[cut, r#"{"prerequisites": []}"#, r#"{"prerequisites": []}"#]);
    let (extraction, report) = extract("a commit", &judge, &Recipe::default()).unwrap();
    assert_eq!(
        extraction.decisions.len(),
        2,
        "the two finished decisions are kept, the unfinished one dropped"
    );
    assert_eq!(extraction.decisions[0].decision, "Pin the dependency");
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("stopped part way")),
        "and the run says so: {:?}",
        report.warnings
    );
}

/// A malformed answer that is not truncated is still an error: there is nothing to salvage.
#[test]
fn an_answer_that_is_not_json_at_all_is_an_error() {
    let judge = Scripted::new(&["I cannot help with that.", "I cannot help with that."]);
    assert!(extract("a commit", &judge, &Recipe::default()).is_err());
}

/// A commit too large for one call is read in parts, each carrying the message.
#[test]
fn a_large_commit_is_read_in_parts_and_each_part_carries_the_message() {
    let message = "Move the parser off the old lexer\n\nThe old one could not see raw strings.";
    let files: Vec<(String, String)> = (1..=4)
        .map(|n| (format!("src/f{n}.rs"), "x".repeat(3_000)))
        .collect();
    let input = cargo_smysl_extract::commit_input(message, &files);

    // A judge whose window admits about two files per part.
    let judge = Sized::new(
        7_000,
        &[
            // Pass one runs over every part first, and both parts report the same decision in the same
            // words: it is one decision. Pass two then asks about it once.
            r#"{"decisions":[{"decision":"Move to the new lexer","kind":"act","rationale":"raw strings","quote":"raw strings"}]}"#,
            r#"{"decisions":[{"decision":"Move to the new lexer","kind":"act","rationale":"raw strings","quote":"raw strings"}]}"#,
            r#"{"prerequisites":[]}"#,
        ],
    );
    let (extraction, report) = extract(&input, &judge, &Recipe::default()).unwrap();

    let asked = judge.asked.borrow().clone();
    let parts: Vec<&(String, String)> = asked
        .iter()
        .filter(|(system, _)| system.contains("report the decisions"))
        .collect();
    assert!(
        parts.len() >= 2,
        "it was read in parts: {} call(s)",
        parts.len()
    );
    for (_, user) in &parts {
        assert!(
            user.contains("could not see raw strings"),
            "every part carries the message, or a diff has no reasons in it"
        );
        assert!(
            user.len() <= 7_100,
            "and every part fits: {} chars",
            user.len()
        );
    }
    assert_eq!(
        extraction.decisions.len(),
        1,
        "the same decision from two parts is one decision"
    );
    assert!(
        report.warnings.iter().any(|w| w.contains("read in")),
        "and the split is reported: {:?}",
        report.warnings
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("more than one part")),
        "as is the duplicate: {:?}",
        report.warnings
    );
}
