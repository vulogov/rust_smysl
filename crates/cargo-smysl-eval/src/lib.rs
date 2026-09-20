//! The S0 evaluation set: which commits it holds, and the registry that names them.
//!
//! Labels, adjudications and scoring live in `cargo-smysl-bench`, which ships. They are the same code an
//! operator runs on their own commits, so this project's figures and theirs cannot drift apart — the
//! rule that moved `commit_input` into the extraction crate.

use std::collections::BTreeMap;

use serde::Deserialize;

pub use cargo_smysl_bench::{
    adjudication, extracted_items, score, suggest, AdjudicatedItem, Adjudication, Extracted, Kind,
    LabelFile, LabelItem, Tally,
};

// ----------------------------------------------------------------------------------------------
// Commits
// ----------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct Commits {
    pub repos: BTreeMap<String, Repo>,
    #[serde(rename = "commit")]
    pub commits: Vec<Commit>,
}

#[derive(Debug, Deserialize)]
pub struct Repo {
    pub url: String,
    pub style: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Commit {
    pub repo: String,
    pub sha: String,
    pub stratum: String,
    pub why: String,
}
