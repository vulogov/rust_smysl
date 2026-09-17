//! Change-rationale units on smysl: labels, the `x.code` extension schema, and the edge policy
//! (plan D2–D5, Phase 1).
//!
//! Model-free: everything here is deterministic, so the corpus can be built, checked and queried
//! without a model or a network.

use smysl::{IdError, Label};

/// Where the corpus lives, relative to the workspace root.
pub const CORPUS_DIR: &str = ".smysl";

/// The extension schema this tool declares.
pub const CODE_SCHEMA_ID: &str = "x.code/v1";
/// A decision touches a code anchor.
pub const REL_TOUCHES: &str = "x.code/touches";
/// A test run exercises the code a prerequisite is about, without asserting the prerequisite.
pub const REL_EXERCISES: &str = "x.code/exercises";

/// The surface declaration every corpus document carries, so `x.code/*` edges check clean (W013).
pub fn schema_declaration() -> String {
    format!(
        "@schema {CODE_SCHEMA_ID} {{ version: 1, relations: [{REL_TOUCHES}, {REL_EXERCISES}] }}"
    )
}

/// Labels for one extraction of one commit (plan D5).
///
/// Labels are not identity in smysl, but two extractions binding the same label to different
/// units are a label-collision contention on merge. So every label names the commit, and a run
/// beyond the first names the run as well.
#[derive(Debug, Clone)]
pub struct Labels {
    stem: String,
}

impl Labels {
    /// `commit` is a hex revision (shortened to 12); `run` is 0 for the first extraction.
    pub fn new(commit: &str, run: u32) -> Result<Labels, IdError> {
        let short: String = commit
            .chars()
            .take(12)
            .collect::<String>()
            .to_ascii_lowercase();
        let stem = if run == 0 {
            format!("g{short}")
        } else {
            format!("g{short}r{run}")
        };
        // Validate once through smysl, so every label built from the stem is well formed.
        Label::new(format!("d/{stem}-1"))?;
        Ok(Labels { stem })
    }

    fn make(&self, kind: &str, parts: &[u32]) -> Label {
        let tail: Vec<String> = parts.iter().map(u32::to_string).collect();
        Label::new(format!("{kind}/{}-{}", self.stem, tail.join("-")))
            .expect("stem validated in Labels::new")
    }

    pub fn decision(&self, d: u32) -> Label {
        self.make("d", &[d])
    }
    pub fn prerequisite(&self, d: u32, n: u32) -> Label {
        self.make("p", &[d, n])
    }
    pub fn alternative(&self, d: u32, n: u32) -> Label {
        self.make("r", &[d, n])
    }
    pub fn consequence(&self, d: u32, n: u32) -> Label {
        self.make("q", &[d, n])
    }
    pub fn evidence(&self, d: u32, n: u32) -> Label {
        self.make("e", &[d, n])
    }
    pub fn anchor(&self, n: u32) -> Label {
        self.make("a", &[n])
    }
    pub fn reading(&self, n: u32) -> Label {
        self.make("t", &[n])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smysl::{
        check, dependents_via, parse_surface, resolve_label, CheckOptions, EdgeSet, Store,
    };

    #[test]
    fn labels_are_valid_smysl_labels_and_name_the_commit() {
        let l = Labels::new("90EC2F781421002876548124", 0).unwrap();
        assert_eq!(l.decision(1).to_string(), "d/g90ec2f781421-1");
        assert_eq!(l.prerequisite(1, 2).to_string(), "p/g90ec2f781421-1-2");
        let again = Labels::new("90ec2f781421", 2).unwrap();
        assert_ne!(
            l.decision(1),
            again.decision(1),
            "a second run must not collide with the first"
        );
    }

    /// Plan D3/D4 against the real library: a prerequisite linked by `conditions` reaches its
    /// decision through `EdgeSet::premises()`, rewording it leaves the decision's uid alone, and the
    /// `x.code` extension checks clean once declared.
    fn document(prerequisite_gist: &str) -> String {
        let l = Labels::new("90ec2f7", 0).unwrap();
        format!(
            "@doc smysl/1.0 {{ id: v/g90ec2f7, intent: change-rationale, granularity: {{ profile: fine }} }}\n\n{}\n\n\
             @constraint {p} {{ status: cited, source: {{ kind: doc, ref: \"git:90ec2f7\" }} }}\n~ {prerequisite_gist}\n\n\
             @decision {d} {{ status: cited, source: {{ kind: doc, ref: \"git:90ec2f7\" }} }}\n~ Add a test that runs every command.\n\n\
             @artifact-ref {a} {{ status: cited, source: {{ kind: file, ref: \"tests/dispatch.rs@90ec2f7\" }} }}\n~ tests/dispatch.rs\n\n\
             @rel {p} --conditions--> {d}\n@rel {d} --{REL_TOUCHES}--> {a}\n",
            schema_declaration(),
            p = l.prerequisite(1, 1),
            d = l.decision(1),
            a = l.anchor(1),
        )
    }

    fn store(src: &str) -> Store {
        Store::from_records(parse_surface(src).expect("parses").records)
    }

    #[test]
    fn the_data_model_holds_against_smysl() {
        let l = Labels::new("90ec2f7", 0).unwrap();
        let s = store(&document(
            "cli() registers every command in the COMMANDS table.",
        ));
        let report = check(&s, CheckOptions::default());
        assert!(report.is_clean(), "{:?}", report.iter().collect::<Vec<_>>());

        let p = resolve_label(&s, &l.prerequisite(1, 1)).unwrap();
        let d = resolve_label(&s, &l.decision(1)).unwrap();
        assert!(
            dependents_via(&s, p, &EdgeSet::premises()).contains(&d),
            "conditions must reach the decision"
        );

        let reworded = store(&document(
            "Every command in COMMANDS is registered by cli().",
        ));
        assert_eq!(
            resolve_label(&reworded, &l.decision(1)).unwrap(),
            d,
            "rewording a prerequisite must not move the decision"
        );
        assert_ne!(resolve_label(&reworded, &l.prerequisite(1, 1)).unwrap(), p);
    }
}
