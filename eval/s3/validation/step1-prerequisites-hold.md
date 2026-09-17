# S3 validation, step 1: does each prerequisite still hold at the base commit?

Checked 2026-09-17 against smysl `origin/main` `183be01` (1.3.0 merged) and ucal `origin/main` `425bb3a`.
Steps 2–4 (naive violation passes tests, oracle detects it, corpus contains and packs it) are still to
do for every task.

| # | Holds? | Evidence at the base commit |
|---|---|---|
| 1 | yes | `src/main.rs:954`: `Ok(again) if again.labels == out.labels && again.records == out.records` is the fmt round-trip guard |
| 2 | yes | `src/main.rs:845`: `vec!["-"]` when `files.is_empty()` (fmt); `:1029` `unwrap_or_else(\|\| vec!["-"])` (check); `read_bytes` reads stdin on `path == "-"` (`:1252–1253`) |
| 3 | yes | `crates/smysl-ingest/src/stage.rs:34` `PATH = ".smysl/staged.smy"`; `crates/smysl-provider/src/usage.rs:290` `Ledger::PATH = ".smysl/usage.log"`, both relative |
| 4 | yes, **reworded** | `tests/cmd_fmt.rs` still reads `F1` and `F9` (`bundle`, `fmt --output`), but the doc comment at `:66–69` records that depending on their formatting state failed spuriously under cargo-mutants. The prerequisite is about depending on fixture state, not about reading fixtures; the task was reworded to match |
| 5 | yes | `src/main.rs:249` `Arg::new("uid").required(true)` for `trace`; `tests/dispatch.rs:92` supplies a UID for `trace` in `minimal_args` |
| 6 | yes (control) | `crates/smysl-core/src/types/epistemics.rs:20,25`: `Unfounded = 0` … `Measured = 5` |
| 7 | yes | `crates/ucal/src/clock.rs:140–144`: `reading(&mut self, wall_now, monotonic_elapsed)`, with the clock passed in, not read |
| 8 | yes (control) | `crates/ucal/src/clock.rs:508` `a_forward_step_is_accepted` |
| 9 | yes | no version-component parsing in `xtask/src/citations.rs`; the only `split('.')` (`:464`) splits JSON field paths; `workspace_version` (`:644`) returns the string |
| 10 | yes (control) | `xtask/src/citations.rs:259` `a_missing_notes_file_for_the_current_version_is_refused` |
| 11 | yes | `Documentation/examples/observations.txt` is committed (blob `ccc12a8`) |
| 12 | yes | `crates/ucal/src/clock.rs:167`: `divergence(…) -> (Ticks, bool)`, magnitude plus direction |
