//! Extraction: decisions, then prerequisites, rejected alternatives and consequences, from a
//! commit's message and diff (plan D5–D8, Phase 2).
//!
//! The model proposes content only; this crate hands units to `cargo-smysl-corpus`, which assigns
//! labels, sources and statuses. Extraction runs once per commit and recipe (D7).
//!
//! Item 7 is settled (D18): the client is our own and lives here, so `check` and `extract` reach a model
//! the same way. The default build speaks plain HTTP to a local provider and carries no TLS stack; the
//! `hosted` feature adds one for a paid provider.

pub mod judge;
pub mod pass;

pub use judge::{Charged, Judge, JudgeError, Provider, ProviderJudge, RawVerdict};
/// The decisions prompt, for a caller that must recognise the tool's own words quoted back.
pub fn pass_decisions_system() -> &'static str {
    pass::DECISIONS_SYSTEM
}

pub use pass::{
    check_quote, commit_input, extract, Cache, Checked, Recipe, Report, Source, SourceFile, Support,
};
