//! Test evidence: link prerequisites to tests, run them at the commit, import the results as
//! measured units, and gate `backs` edges (plan D13–D14, Phase 3, spike S1).
//!
//! Self-contained: tests run through `$CARGO`; mutation gating, if S1 keeps it, is built in rather
//! than delegated to an external cargo-mutants binary.
