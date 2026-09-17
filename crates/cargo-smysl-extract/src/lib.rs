//! Extraction: decisions, then prerequisites, rejected alternatives and consequences, from a
//! commit's message and diff (plan D5–D8, Phase 2).
//!
//! The model proposes content only; this crate hands units to `cargo-smysl-corpus`, which assigns
//! labels, sources and statuses. Extraction runs once per commit and recipe (D7).
//!
//! Open at phase start (plan item 7): an own model client, or smysl's `model` feature.
