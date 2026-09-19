//! Verdicts: per-prerequisite fact retrieval, matching, and the status policy (plan D11–D12,
//! Phase 3).
//!
//! No single run raises a status. SUPPORTED needs full coverage by structural facts and two
//! independent runs agreeing, or a person; everything else attaches evidence or goes to review.

pub mod check;

pub use check::{check, Change, Finding, Outcome, Settings, DEFAULT_SYSTEM};
// The model client lives in `cargo-smysl-extract` (D18): one client, two callers.
pub use cargo_smysl_extract::{Charged, Judge, JudgeError, Provider, ProviderJudge, RawVerdict};
