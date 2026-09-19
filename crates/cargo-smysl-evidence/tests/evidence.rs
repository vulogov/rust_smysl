//! Test candidates (D14) and readings (D13), on this workspace's own code and on real libtest output.

use cargo_smysl_evidence::{candidates, import, readings, tests, Plan, Reading};
use cargo_smysl_facts::facts;

const SOURCE: &str = r#"
pub fn root_beside(store: Option<&str>) -> String {
    store.map(|_| ".".to_string()).unwrap_or_else(|| ".".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ledger sits beside the store, not in the user's home directory.
    #[test]
    fn a_sidecar_sits_beside_its_store() {
        assert_eq!(root_beside(Some("store.smy")), ".");
        assert_eq!(root_beside(None), ".");
    }

    #[test]
    fn a_forward_step_of_the_clock_is_accepted() {
        let session = Session::default();
        assert!(session.advance(60).is_ok());
    }

    #[test]
    fn unrelated_parsing_of_a_manifest() {
        assert!(parse_manifest("[package]").is_ok());
    }
}
"#;

fn corpus() -> Vec<cargo_smysl_facts::Fact> {
    facts("src/main.rs", SOURCE).unwrap()
}

#[test]
fn the_tests_are_told_from_the_code() {
    let all = corpus();
    let found: Vec<String> = tests(&all).iter().map(|t| t.name.clone()).collect();
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(
        !found.contains(&"root_beside".to_string()),
        "the code is not a test"
    );
}

#[test]
fn a_claim_shortlists_the_tests_that_speak_of_it() {
    let all = corpus();
    let picked = candidates(
        &all,
        "the usage ledger is resolved beside the store, not in the home directory",
        &[],
        3,
    );
    assert!(!picked.is_empty(), "something is shortlisted");
    assert_eq!(
        picked[0].label, "tests::a_sidecar_sits_beside_its_store",
        "the test that names the same things comes first: {picked:#?}"
    );
    assert!(
        picked[0]
            .shared
            .iter()
            .any(|t| t == "store" || t == "beside"),
        "and it says which terms they share: {:?}",
        picked[0].shared
    );
    assert!(picked.len() <= 3, "a shortlist, not everything");
}

#[test]
fn a_test_in_a_touched_file_is_lifted_and_the_ranking_is_stable() {
    let all = corpus();
    let claim = "the session clock accepts a forward step";
    let plain = candidates(&all, claim, &[], 5);
    let touched = candidates(&all, claim, &["src/main.rs".to_string()], 5);
    assert_eq!(plain[0].label, touched[0].label);
    assert!(touched[0].score > plain[0].score, "the bonus applies");
    assert!(touched.iter().all(|c| c.touched));
    assert_eq!(
        plain,
        candidates(&all, claim, &[], 5),
        "same input, same order"
    );
}

#[test]
fn a_claim_with_nothing_in_common_shortlists_nothing() {
    let all = corpus();
    assert!(candidates(&all, "quantum chromodynamics in the renderer", &[], 5).is_empty());
    assert!(
        candidates(&all, "", &[], 5).is_empty(),
        "an empty claim is not a search"
    );
}

/// Real libtest output, including the shapes that are not tests.
const OUTPUT: &str = "
running 3 tests
test tests::a_sidecar_sits_beside_its_store ... ok
test tests::a_forward_step_of_the_clock_is_accepted ... FAILED
test tests::unrelated_parsing_of_a_manifest ... ignored, needs a fixture

failures:
    tests::a_forward_step_of_the_clock_is_accepted

test result: FAILED. 1 passed; 1 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.42s
";

#[test]
fn what_the_tests_did_is_read_from_what_cargo_printed() {
    let found = readings(OUTPUT, "90ec2f7", "cargo 1.94.1");
    assert_eq!(found.len(), 3, "{found:#?}");
    assert_eq!(found[0].outcome, "passed");
    assert_eq!(found[1].outcome, "failed");
    assert_eq!(
        found[2].outcome, "ignored",
        "an ignored test has a reading, not silence"
    );
    assert!(
        found.iter().all(|r| r.run_seconds == 0.42),
        "the suite's time"
    );
    assert!(found.iter().all(|r| r.commit == "90ec2f7"));
    assert!(
        !found.iter().any(|r| r.test.contains(' ')),
        "summary lines are not tests"
    );
}

#[test]
fn a_reading_becomes_a_measured_unit_that_traces_to_the_importer() {
    let found = readings(OUTPUT, "90ec2f7", "cargo 1.94.1");
    let imported = import(&found, "cargo test").unwrap();
    assert_eq!(imported.units.len(), 3, "{:?}", imported.diagnostics);
    assert!(
        imported
            .units
            .iter()
            .all(|u| u.status == smysl::Status::Measured),
        "a reading is measured, and the attestation is the licence"
    );
    assert_eq!(imported.attestations.len(), 3);
    assert!(
        imported
            .attestations
            .iter()
            .all(|a| a.agent.as_str() == "tool:smysl-import"),
        "every reading traces to tool:smysl-import"
    );
    // smysl 1.4's R12: an imported reading checks clean, gist bound included.
    let store = smysl::Store::from_records(imported.records());
    let errors: Vec<String> = smysl::check(&store, smysl::CheckOptions::default())
        .iter()
        .filter(|d| d.severity == smysl::Severity::Error)
        .map(|d| d.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn the_plan_is_a_command_a_person_could_paste() {
    let plan = Plan {
        package: Some("cargo-smysl-facts".into()),
        target: Some("facts".into()),
        tests: vec!["scope::a_named_item_brings_itself_and_what_it_calls".into()],
        features: vec!["hosted".into()],
    };
    let args = plan.args().join(" ");
    assert_eq!(
        args,
        "test --locked --no-fail-fast -p cargo-smysl-facts --test facts --features hosted \
         -- --exact scope::a_named_item_brings_itself_and_what_it_calls"
    );
    assert!(
        Plan::default().args().join(" ") == "test --locked --no-fail-fast",
        "the plainest run is the whole workspace"
    );
}

#[test]
fn a_reading_carries_the_machine_it_was_made_on() {
    let r = Reading {
        test: "a::b".into(),
        outcome: "passed".into(),
        run_seconds: 0.1,
        commit: "90ec2f7".into(),
        toolchain: "cargo 1.94.1".into(),
    };
    assert!(r.passed());
    let csv = cargo_smysl_evidence::run::to_csv(&[r]);
    assert!(csv
        .lines()
        .next()
        .unwrap()
        .starts_with("test,commit,outcome"));
    assert!(
        csv.contains("cargo 1.94.1"),
        "the toolchain is part of the reading"
    );
}

/// D13: what a link may say, and what it may not.
mod links {
    use cargo_smysl_evidence::link::{edges, Kind, Link};
    use cargo_smysl_evidence::Reading;
    use smysl::{AgentId, Record, RelKind, Uid};

    fn reading(outcome: &str) -> Reading {
        Reading {
            test: "tests::a".into(),
            outcome: outcome.into(),
            run_seconds: 0.1,
            commit: "90ec2f7".into(),
            toolchain: "cargo 1.94.1".into(),
        }
    }

    fn uids() -> (Uid, Uid) {
        let a = smysl::hash_bytes(b"reading");
        let b = smysl::hash_bytes(b"claim");
        (Uid::from_bytes(a), Uid::from_bytes(b))
    }

    #[test]
    fn a_passing_test_that_verifies_a_claim_backs_it_and_says_who_proposed_that() {
        let (r, c) = uids();
        let model = AgentId::new("model:qwen2.5-coder").unwrap();
        let records = edges(
            &[Link {
                reading: r,
                claim: c,
                kind: Kind::Verifies,
            }],
            &|_| Some(reading("passed")),
            &model,
        )
        .unwrap();
        let edge = records
            .iter()
            .find_map(|x| match x {
                Record::Relation(rel) => Some(rel),
                _ => None,
            })
            .unwrap();
        assert_eq!(edge.kind, RelKind::Backs);
        let attestation = records
            .iter()
            .find_map(|x| match x {
                Record::Attestation(a) => Some(a),
                _ => None,
            })
            .expect("the proposal is attributed");
        assert_eq!(attestation.agent.as_str(), "model:qwen2.5-coder");
    }

    #[test]
    fn a_failing_test_never_backs_the_claim_it_verifies() {
        let (r, c) = uids();
        let model = AgentId::new("model:qwen2.5-coder").unwrap();
        let records = edges(
            &[Link {
                reading: r,
                claim: c,
                kind: Kind::Verifies,
            }],
            &|_| Some(reading("failed")),
            &model,
        )
        .unwrap();
        let edge = records
            .iter()
            .find_map(|x| match x {
                Record::Relation(rel) => Some(rel),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            edge.kind,
            RelKind::Rebuts,
            "a failure is a disagreement, not support"
        );
    }

    #[test]
    fn exercising_is_navigation_and_unrelated_is_nothing() {
        let (r, c) = uids();
        let model = AgentId::new("model:qwen2.5-coder").unwrap();
        let records = edges(
            &[Link {
                reading: r,
                claim: c,
                kind: Kind::Exercises,
            }],
            &|_| Some(reading("passed")),
            &model,
        )
        .unwrap();
        let edge = records
            .iter()
            .find_map(|x| match x {
                Record::Relation(rel) => Some(rel),
                _ => None,
            })
            .unwrap();
        assert_eq!(edge.kind.to_string(), "x.code/exercises");

        let none = edges(
            &[Link {
                reading: r,
                claim: c,
                kind: Kind::Unrelated,
            }],
            &|_| Some(reading("passed")),
            &model,
        )
        .unwrap();
        assert!(none.is_empty(), "no edge, no record");
    }

    #[test]
    fn a_reading_that_was_never_taken_is_not_evidence_either_way() {
        let (r, c) = uids();
        let model = AgentId::new("model:qwen2.5-coder").unwrap();
        let records = edges(
            &[Link {
                reading: r,
                claim: c,
                kind: Kind::Verifies,
            }],
            &|_| None,
            &model,
        )
        .unwrap();
        assert!(records.is_empty());
    }

    #[test]
    fn the_kinds_are_read_forgivingly_and_default_to_unrelated() {
        assert_eq!(Kind::parse("Verifies"), Kind::Verifies);
        assert_eq!(Kind::parse(" exercises "), Kind::Exercises);
        assert_eq!(Kind::parse("probably related?"), Kind::Unrelated);
    }
}

/// S1: the cheap check that ships — a test that cannot fail.
mod vacuity {
    use cargo_smysl_evidence::vacuous;
    use cargo_smysl_facts::{facts, Fact};

    fn test_named(source: &str, name: &str) -> cargo_smysl_facts::item::Function {
        facts("t.rs", source)
            .unwrap()
            .into_iter()
            .find_map(|f| match f {
                Fact::Function(x) if x.name == name => Some(x),
                _ => None,
            })
            .unwrap()
    }

    const SOURCE: &str = r#"
#[cfg(test)]
mod tests {
    #[test]
    fn compares_a_thing_with_itself() {
        assert_eq!(session.anchor, session.anchor);
    }

    #[test]
    fn asserts_a_literal_truth() {
        assert!(true);
    }

    #[test]
    fn has_no_assertion_at_all() {
        let session = Session::default();
        session.advance(60);
    }

    #[test]
    fn actually_checks_something() {
        assert_eq!(root_beside(Some("store.smy")), ".");
    }
}
"#;

    #[test]
    fn an_assertion_comparing_a_thing_with_itself_cannot_fail() {
        let reasons = vacuous(&test_named(SOURCE, "compares_a_thing_with_itself"));
        assert_eq!(reasons.len(), 1, "{reasons:?}");
        assert!(
            reasons[0].contains("compares a thing with itself"),
            "{reasons:?}"
        );
    }

    #[test]
    fn so_can_a_literal_truth_and_a_test_with_no_assertion() {
        assert!(
            vacuous(&test_named(SOURCE, "asserts_a_literal_truth"))[0].contains("literal truth")
        );
        assert!(vacuous(&test_named(SOURCE, "has_no_assertion_at_all"))[0].contains("no assertion"));
    }

    #[test]
    fn a_real_assertion_is_reported_as_nothing_proved() {
        assert!(
            vacuous(&test_named(SOURCE, "actually_checks_something")).is_empty(),
            "silence here means 'nothing proved', not 'this test is good'"
        );
    }
}
