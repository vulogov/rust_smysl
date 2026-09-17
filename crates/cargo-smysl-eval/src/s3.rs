//! S3 corpus arm: one store per repository from every extracted commit that is an ancestor of the task
//! base, and for each task and question a context packed within the registered budget.
//!
//! Selection is the product's own: BM25 hits on the task statement (or question) are the pack's focus,
//! so they must fit with their closure and live rebuttals (rule R); salience is seeded from units
//! sourced in the files the task touches, so what fills the rest of the budget is about that code.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Deserialize;
use smysl::{Bm25, Label, Lod, PackRequest, Query, Retriever as _, SalienceRequest, Store, Uid};

#[derive(Deserialize)]
pub struct Registry {
    pub system: String,
    pub budget: u64,
    pub repos: BTreeMap<String, RepoRun>,
    #[serde(rename = "task")]
    pub tasks: Vec<Task>,
    #[serde(rename = "question")]
    pub questions: Vec<Question>,
}

#[derive(Deserialize)]
pub struct RepoRun {
    pub base: String,
}

#[derive(Deserialize)]
pub struct Task {
    pub n: u32,
    pub repo: String,
    pub statement: String,
    pub files: Vec<String>,
}

#[derive(Deserialize)]
pub struct Question {
    pub n: u32,
    pub repo: String,
    pub text: String,
}

/// A repository's corpus: the merged store and its label for each unit.
pub struct Corpus {
    pub store: Store,
    pub labels: BTreeMap<Uid, Label>,
    pub commits: Vec<String>,
}

impl Corpus {
    pub fn new() -> Corpus {
        Corpus {
            store: Store::from_records(Vec::new()),
            labels: BTreeMap::new(),
            commits: Vec::new(),
        }
    }

    /// Stage one commit's batch against everything already in the corpus, and append it.
    pub fn add(&mut self, sha: &str, batch: cargo_smysl_corpus::Batch) -> Result<(), String> {
        let staged = cargo_smysl_corpus::stage(&self.store, batch, 0);
        if staged.has_errors() {
            let first: Vec<String> = staged
                .report
                .iter()
                .filter(|d| d.severity == smysl::Severity::Error)
                .take(3)
                .map(|d| d.to_string())
                .collect();
            return Err(format!("{sha}: staging errors: {}", first.join("; ")));
        }
        self.store
            .append(&staged.records())
            .map_err(|e| format!("{sha}: append: {e}"))?;
        self.labels
            .extend(staged.labels.iter().map(|(l, u)| (*u, l.clone())));
        self.commits.push(sha.to_string());
        Ok(())
    }

    /// Commits (as recorded in `commits`) holding the top BM25 hits for `query`, found through each hit's
    /// label, whose stem is `g<sha12>`.
    pub fn commits_retrieved(&self, query: &str, limit: usize) -> BTreeSet<String> {
        let hits = Bm25::index(&self.store).search(&Query::new(query.to_string(), limit));
        hits.iter()
            .filter_map(|h| {
                let name = self.name(&h.uid);
                self.commits
                    .iter()
                    .find(|c| name.contains(&format!("/g{}", &c[..c.len().min(7)])))
                    .cloned()
            })
            .collect()
    }

    pub(crate) fn name(&self, uid: &Uid) -> String {
        self.labels
            .get(uid)
            .map(|l| l.as_str().to_string())
            .unwrap_or_else(|| uid.short())
    }
}

/// What a packed context holds, for the summary and the validation record.
pub struct Packed {
    pub text: String,
    pub focus: usize,
    pub units: usize,
    /// Units packed with their body (L1 or more), not only their gist.
    pub detailed: usize,
    pub used: u64,
}

/// Pack for `query`, seeding salience from units sourced in `files`. The focus starts at six BM25 hits
/// and shrinks until the focus and its closure fit the budget.
pub fn pack(corpus: &Corpus, query: &str, files: &[String], budget: u64) -> Result<Packed, String> {
    let seed: Vec<Uid> = corpus
        .store
        .units()
        .filter(|(_, u)| {
            u.core.source.as_ref().is_some_and(|s| {
                files
                    .iter()
                    .any(|f| s.reference.starts_with(&format!("{f}@")))
            })
        })
        .map(|(uid, _)| *uid)
        .collect();
    let sal = smysl::salience(&corpus.store, &SalienceRequest::default().seeded(seed));
    let index = Bm25::index(&corpus.store);
    let mut last = String::new();
    for limit in (1..=6).rev() {
        let hits: Vec<Uid> = index
            .search(&Query::new(query.to_string(), limit))
            .into_iter()
            .map(|h| h.uid)
            .collect();
        if hits.is_empty() {
            return Err(format!("no BM25 hit for {query:?}"));
        }
        let req = PackRequest::budget(budget).focusing(hits.clone());
        match smysl::pack(&corpus.store, &sal, &req) {
            Ok(p) => {
                return Ok(Packed {
                    text: render(corpus, &p.selection),
                    focus: hits.len(),
                    units: p.len(),
                    detailed: p.selection.values().filter(|l| **l >= Lod::L1).count(),
                    used: p.used(),
                })
            }
            Err(e) => last = e.to_string(),
        }
    }
    Err(format!("nothing fits the budget: {last}"))
}

/// Plain text for an agent: each selected unit with its kind, status, source commit, text at its packed
/// level of detail, and its edges to other selected units.
pub(crate) fn render(corpus: &Corpus, selection: &BTreeMap<Uid, Lod>) -> String {
    let selected: BTreeSet<&Uid> = selection.keys().collect();
    let mut edges: BTreeMap<Uid, Vec<String>> = BTreeMap::new();
    for r in corpus.store.relations() {
        if selected.contains(&r.from) && selected.contains(&r.to) {
            edges
                .entry(r.from)
                .or_default()
                .push(format!("{} {}", r.kind, corpus.name(&r.to)));
        }
    }
    // Stable, readable order: by label, which groups a commit's decision with its items.
    let mut order: Vec<(&Uid, &Lod)> = selection.iter().collect();
    order.sort_by_key(|(u, _)| corpus.name(u));
    let mut out = String::new();
    for (uid, level) in order {
        let Some(unit) = corpus.store.get(uid) else {
            continue;
        };
        let source = unit
            .core
            .source
            .as_ref()
            .map(|s| format!(", source {}", s.reference))
            .unwrap_or_default();
        out.push_str(&format!(
            "[{}] {} ({}{})\n  {}\n",
            corpus.name(uid),
            unit.core.schema,
            unit.core.status,
            source,
            unit.core.gist
        ));
        if *level >= Lod::L1 {
            if let Some(b) = &unit.core.body {
                for line in b.lines() {
                    out.push_str(&format!("  {line}\n"));
                }
            }
        }
        for e in edges.get(uid).into_iter().flatten() {
            out.push_str(&format!("  -> {e}\n"));
        }
        out.push('\n');
    }
    out
}

pub fn read_registry(path: &Path) -> Result<Registry, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}
