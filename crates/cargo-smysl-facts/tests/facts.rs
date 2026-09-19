//! What the extractor must hold (plan D9, D10), on source written for the purpose and on this
//! workspace's own code.

use cargo_smysl_facts::{facts, EventKind, Fact};

const SOURCE: &str = r#"
//! A file.
#![cfg(feature = "civil")]

/// What the ledger is for.
pub const LEDGER: &str = ".smysl/usage.log";
const LIMITS: &[u32] = &[1, 2, 3];

pub struct Session {
    /// When it was anchored.
    pub anchor: Option<u64>,
}

impl Session {
    /// Reads the clock only on first use.
    pub fn now(&mut self, wall: u64) -> u64 {
        let anchored = self.anchor.unwrap_or(wall);
        if wall > anchored {
            log("clock moved forward");
            self.advance(wall)
        } else {
            anchored
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_forward_step_is_accepted() {
        assert_eq!(super::Session::default().now(10), 10);
    }
}
"#;

fn function<'a>(items: &'a [Fact], name: &str) -> &'a cargo_smysl_facts::item::Function {
    items
        .iter()
        .find_map(|f| match f {
            Fact::Function(x) if x.name == name => Some(x),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no function {name} in {items:#?}"))
}

#[test]
fn a_function_records_what_it_does_and_where_it_sits() {
    let items = facts("src/clock.rs", SOURCE).unwrap();
    let now = function(&items, "now");
    assert_eq!(now.owner.as_deref(), Some("Session"));
    assert_eq!(now.visibility, "pub");
    assert!(
        now.doc.contains("first use"),
        "doc comments are kept: {:?}",
        now.doc
    );
    assert!(now.params.contains("wall"), "{}", now.params);
    assert!(now.returns.contains("u64"), "{}", now.returns);

    // The call inside the `if` carries the branch it sits in: a claim about "when the clock moves
    // forward" can be checked against the context, not the whole body.
    let call = now
        .events
        .iter()
        .find(|e| e.kind == EventKind::Call && e.detail.iter().any(|(_, v)| v == "log"))
        .expect("the call is recorded");
    assert!(
        call.ctx.iter().any(|c| c.starts_with("if wall > anchored")),
        "control context: {:?}",
        call.ctx
    );
    // A method call, a binding and the file's cfg are all there.
    assert!(now.events.iter().any(|e| e.kind == EventKind::Method));
    assert!(now.events.iter().any(|e| e.kind == EventKind::Let));
    // `syn` renders attribute tokens with spacing of its own; the fact is the cfg, not its layout.
    assert_eq!(now.file_cfg.len(), 1);
    assert!(
        now.file_cfg[0].contains("feature") && now.file_cfg[0].contains("civil"),
        "{:?}",
        now.file_cfg
    );
}

#[test]
fn author_prose_is_tagged_so_no_verdict_can_rest_on_it() {
    let items = facts("src/clock.rs", SOURCE).unwrap();
    // A string literal in a body is prose (D10).
    let now = function(&items, "now");
    let literal = now
        .events
        .iter()
        .find(|e| e.kind == EventKind::Str)
        .expect("the literal is extracted");
    assert!(
        literal.prose,
        "a string literal never verifies: {literal:?}"
    );
    assert!(
        now.events.iter().filter(|e| !e.prose).count() > 0,
        "and the rest of the events are not prose"
    );

    // So is a string-valued constant; a numeric table is not.
    let consts: Vec<&cargo_smysl_facts::Constant> = items
        .iter()
        .filter_map(|f| match f {
            Fact::Const(c) => Some(c),
            _ => None,
        })
        .collect();
    let ledger = consts.iter().find(|c| c.name == "LEDGER").unwrap();
    assert!(ledger.prose, "a string constant is author text");
    let limits = consts.iter().find(|c| c.name == "LIMITS").unwrap();
    assert!(!limits.prose);
    assert_eq!(
        limits.elements.as_ref().unwrap().len(),
        3,
        "a table keeps its elements"
    );
}

#[test]
fn a_test_is_told_from_the_code_it_exercises() {
    let items = facts("src/clock.rs", SOURCE).unwrap();
    let t = function(&items, "a_forward_step_is_accepted");
    assert!(t.is_test(), "{:?}", t.cfg);
    assert!(
        t.file_cfg.iter().any(|c| c.contains("test")),
        "the enclosing #[cfg(test)] module marks it: {:?}",
        t.file_cfg
    );
    assert!(!function(&items, "now").is_test());
    // A macro's arguments are visited, so the assertion's calls are not invisible.
    assert!(t.events.iter().any(|e| e.kind == EventKind::Macro));
    assert!(t.events.iter().any(|e| e.kind == EventKind::Method));
}

#[test]
fn a_struct_keeps_its_fields_and_a_file_that_does_not_parse_is_an_error() {
    let items = facts("src/clock.rs", SOURCE).unwrap();
    let s = items
        .iter()
        .find_map(|f| match f {
            Fact::Struct(s) if s.name == "Session" => Some(s),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        s.fields,
        vec![("anchor".to_string(), "Option < u64 >".to_string())]
    );
    assert!(
        facts("bad.rs", "fn (").is_err(),
        "absence of facts is never a fact"
    );
}

#[test]
fn the_same_source_gives_byte_identical_facts() {
    // Phase 2's "done when": facts regenerate byte-identically, which is what makes a cache safe.
    let once = serde_json::to_string(&facts("src/clock.rs", SOURCE).unwrap()).unwrap();
    let twice = serde_json::to_string(&facts("src/clock.rs", SOURCE).unwrap()).unwrap();
    assert_eq!(once, twice);

    // And on real code: this crate's own source.
    let own = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/item.rs")).unwrap();
    let a = serde_json::to_string(&facts("src/item.rs", &own).unwrap()).unwrap();
    let b = serde_json::to_string(&facts("src/item.rs", &own).unwrap()).unwrap();
    assert_eq!(a, b);
    assert!(a.len() > 10_000, "the extractor found something to say");
}

#[test]
fn moving_a_function_does_not_change_its_body_hash() {
    let a = facts("a.rs", "fn f() { g(1); }\n").unwrap();
    let b = facts("a.rs", "\n\n// a comment\nfn f() { g(1); }\n").unwrap();
    let (a, b) = (function(&a, "f"), function(&b, "f"));
    assert_ne!(a.line, b.line, "it moved");
    assert_eq!(a.body_hash, b.body_hash, "but it is the same function");

    let changed = facts("a.rs", "fn f() { g(2); }\n").unwrap();
    assert_ne!(a.body_hash, function(&changed, "f").body_hash);
}

#[test]
fn the_cache_returns_the_same_facts_and_regenerates_what_it_cannot_read() {
    use cargo_smysl_facts::Cache;

    let root = std::env::temp_dir().join(format!("cargo-smysl-facts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cache = Cache::at(&root);

    let first = cache.facts_of("src/clock.rs", SOURCE).unwrap();
    assert!(cache.entry(SOURCE).exists(), "the entry is written");
    let second = cache.facts_of("src/clock.rs", SOURCE).unwrap();
    assert_eq!(first, second, "a hit is the same answer as a miss");
    assert_eq!(first, facts("src/clock.rs", SOURCE).unwrap());

    // Different bytes, different entry: a stale fact can never be read as a fresh one.
    let edited = SOURCE.replace("wall > anchored", "wall >= anchored");
    assert_ne!(cache.entry(&edited), cache.entry(SOURCE));

    // A corrupted entry is regenerated rather than trusted.
    std::fs::write(cache.entry(SOURCE), "{ not json").unwrap();
    assert_eq!(cache.facts_of("src/clock.rs", SOURCE).unwrap(), first);

    cache.clear().unwrap();
    assert!(!cache.entry(SOURCE).exists());
    std::fs::remove_dir_all(&root).ok();
}

/// D9: what bears on a change is stated, not guessed — the touched items, the named ones, and one hop.
mod scope {
    use cargo_smysl_facts::{facts, render, select, Around, Fact, Reason};

    const CLOCK: &str = r#"
pub fn session_now(wall: u64) -> u64 {
    anchor_once(wall)
}

pub fn anchor_once(wall: u64) -> u64 {
    store(wall)
}

pub fn store(v: u64) -> u64 { v }

pub fn unrelated_helper() -> u64 { 0 }
"#;

    const OTHER: &str = r#"
pub fn caller_elsewhere() -> u64 {
    super::session_now(7)
}

pub fn far_away() -> u64 { 1 }
"#;

    fn corpus() -> Vec<Fact> {
        let mut all = facts("src/clock.rs", CLOCK).unwrap();
        all.extend(facts("src/other.rs", OTHER).unwrap());
        all
    }

    fn names(picked: &[cargo_smysl_facts::Selected<'_>]) -> Vec<String> {
        picked
            .iter()
            .map(|s| match s.fact {
                Fact::Function(f) => f.name.clone(),
                _ => String::new(),
            })
            .collect()
    }

    #[test]
    fn a_touched_file_brings_its_items_and_one_hop_outward() {
        let all = corpus();
        let picked = select(&all, &Around::files(["src/clock.rs".into()]));
        let found = names(&picked);
        assert!(found.contains(&"session_now".to_string()), "{found:?}");
        assert!(
            found.contains(&"unrelated_helper".to_string()),
            "a touched file comes whole"
        );
        // One hop outward: what calls into the touched file, from a file the change did not touch.
        assert!(found.contains(&"caller_elsewhere".to_string()), "{found:?}");
        assert!(
            !found.contains(&"far_away".to_string()),
            "two hops is not one: {found:?}"
        );
        assert_eq!(
            picked
                .iter()
                .find(|s| matches!(s.fact, Fact::Function(f) if f.name == "caller_elsewhere"))
                .unwrap()
                .reason,
            Reason::CalledBy
        );
    }

    #[test]
    fn a_named_item_brings_itself_and_what_it_calls() {
        let all = corpus();
        let picked = select(
            &all,
            &Around {
                files: vec![],
                names: vec!["anchor_once".into()],
                hops: 1,
            },
        );
        let found = names(&picked);
        assert!(
            found.contains(&"anchor_once".to_string()),
            "the named item: {found:?}"
        );
        assert!(
            found.contains(&"store".to_string()),
            "what it calls: {found:?}"
        );
        assert!(
            found.contains(&"session_now".to_string()),
            "and what calls it: {found:?}"
        );
        assert!(!found.contains(&"far_away".to_string()), "{found:?}");
    }

    #[test]
    fn no_hops_is_the_touched_items_alone_and_selection_is_deterministic() {
        let all = corpus();
        let around = Around {
            files: vec!["src/clock.rs".into()],
            names: vec![],
            hops: 0,
        };
        let picked = names(&select(&all, &around));
        assert!(
            !picked.contains(&"caller_elsewhere".to_string()),
            "{picked:?}"
        );
        assert_eq!(
            picked,
            names(&select(&all, &around)),
            "same input, same order"
        );
    }

    fn now_line(all: &[Fact]) -> usize {
        all.iter()
            .find_map(|f| match f {
                Fact::Function(x) if x.name == "now" => Some(x.line),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn a_rendered_fact_says_what_the_code_does_and_marks_what_it_only_claims() {
        let all = facts("src/clock.rs", super::SOURCE).unwrap();
        let now = all
            .iter()
            .find(|f| matches!(f, Fact::Function(x) if x.name == "now"))
            .unwrap();
        let text = render(now);
        assert!(text.starts_with("fn Session::now"), "{text}");
        assert!(
            text.contains("src/clock.rs:") && text.contains(&format!(":{}", now_line(&all))),
            "where it is: {text}"
        );
        assert!(
            text.contains("under if wall > anchored"),
            "the branch a call sits in: {text}"
        );
        assert!(
            text.contains("doc (prose):"),
            "author text is marked: {text}"
        );
        assert!(
            text.contains("string (prose)"),
            "and so are literals: {text}"
        );

        let ledger = all
            .iter()
            .find(|f| matches!(f, Fact::Const(c) if c.name == "LEDGER"))
            .unwrap();
        assert!(
            render(ledger).contains("never verifies"),
            "{}",
            render(ledger)
        );
    }
}

/// D9: which `cfg` CI actually builds, so a claim is not checked against code nothing compiles.
mod ci {
    use std::collections::BTreeSet;

    use cargo_smysl_facts::{builds, ci::builds_in, coverage, Coverage};

    const WORKFLOW: &str = r#"
jobs:
  test:
    steps:
      - run: cargo test --workspace --locked
  lint:
    steps:
      - run: cargo clippy --workspace --all-features --all-targets -- -D warnings
  matrix:
    strategy:
      matrix:
        features:
          - "--no-default-features"
          - "--all-features"
          - "--no-default-features --features cli"
    steps:
      - run: cargo build --workspace ${{ matrix.features }}
"#;

    #[test]
    fn the_builds_a_workflow_runs_are_read_from_its_cargo_lines() {
        let found = builds_in(WORKFLOW);
        assert_eq!(found.len(), 5, "{found:#?}");
        assert!(found
            .iter()
            .any(|b| b.command == "test" && b.default_features));
        assert!(found
            .iter()
            .any(|b| b.command == "clippy" && b.all_features));
        // A cargo line that interpolates the matrix stands for each of its entries.
        let builds: Vec<_> = found.iter().filter(|b| b.command == "build").collect();
        assert_eq!(builds.len(), 3);
        assert!(builds
            .iter()
            .any(|b| !b.default_features && b.features.is_empty()));
        assert!(builds.iter().any(|b| b.all_features));
        assert!(builds
            .iter()
            .any(|b| !b.default_features && b.features.contains("cli")));
    }

    #[test]
    fn a_cfg_is_covered_always_sometimes_or_never() {
        let found = builds_in(WORKFLOW);
        let defaults: BTreeSet<String> = ["cli".to_string()].into_iter().collect();
        assert_eq!(
            coverage(&[], &found, &defaults),
            Coverage::Always,
            "ungated code"
        );
        // `test` is compiled by the one `cargo test` job.
        assert!(matches!(
            coverage(&["cfg(test)".into()], &found, &defaults),
            Coverage::Some { .. }
        ));
        // A feature no job names, with one job at --no-default-features, is compiled by some.
        assert!(matches!(
            coverage(&["cfg(feature = \"cli\")".into()], &found, &defaults),
            Coverage::Some { .. }
        ));
        // Nothing builds `not(any())`-style impossibilities.
        assert_eq!(
            coverage(&["cfg(all(test, not(test)))".into()], &found, &defaults),
            Coverage::Never
        );
        assert_eq!(
            coverage(&["cfg(feature = \"x\")".into()], &[], &defaults),
            Coverage::Unknown,
            "no builds found is not the same as never built"
        );
        // A feature no job names and the manifest does not default to is built by the --all-features
        // jobs only.
        assert!(matches!(
            coverage(&["cfg(feature = \"hosted\")".into()], &found, &defaults),
            Coverage::Some { .. }
        ));
    }

    #[test]
    fn this_repository_s_own_workflows_are_read() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../.github/workflows");
        let found = builds(dir);
        assert!(found.len() >= 4, "{found:#?}");
        assert!(found.iter().any(|b| b.is_test()), "the test job");
        assert!(
            found.iter().any(|b| b.command == "install"),
            "the install job, which is the objective"
        );
    }
}
