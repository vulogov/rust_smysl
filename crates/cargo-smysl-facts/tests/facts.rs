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
