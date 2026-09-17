# smysl 1.4.0: acceptance from rust_smysl, before publish

**Checked:** 2026-09-17, against `../smysl` on `dev/1.4.0` at `c7bf5a3`. The working tree also had
uncommitted edits in the TUI, the Go/Node/Python READMEs and package versions, and two scripts. A
snapshot of that state was tested, with rust_smysl patched to it through `[patch.crates-io]`.
smysl's own suite (`make ci`) was not run here.

## Verdict

**Ready to publish, as far as rust_smysl is concerned.** R10, the one request that affects the tool,
passes its acceptance, and moving the pin from 1.3 to 1.4 changes nothing rust_smysl produces.
R11, R12, R13 and R14 are still open. None of them blocks rust_smysl, but R12 matters once test
evidence is imported (Phase 3).

## rust_smysl on 1.4.0

| Check | Result |
|---|---|
| `cargo test --workspace` | all pass, 0 ignored: `merging_a_store_into_itself_adds_no_records` (ignored on 1.3) now passes |
| `cargo clippy --workspace --all-targets -D warnings` | clean |
| `smysl-eval stage` for research-pro-v2, research-flash-v2 and research-deepseek-v2 | 0 staging errors; per-commit units, edges and quote tallies **identical** to 1.3 |
| `smysl-eval s3-context` (22 packed contexts) | **byte-identical** to 1.3, so S3 run2's contexts do not depend on the version |

## R10 acceptance (`crates/cargo-smysl-corpus/tests/acceptance_1_4.rs`)

All four pass on 1.4.0 and fail on 1.3.0. They are committed ignored until the pin moves.

| Test | Acceptance item |
|---|---|
| `r10_repeated_self_merge_appends_nothing` | `merge(A, A)` four times on a real staged batch: `added == 0`, record count unchanged |
| `r10_two_commits_merged_both_ways_then_again_are_stable` | A∪B and B∪A hold the same records; merging one into the other appends nothing |
| `r10_a_label_bound_to_a_different_uid_is_still_appended` | a rival binding is appended (`added == 1`), and the collision is detected from the store's own bindings |
| `r10_a_store_opened_from_a_file_merged_with_its_own_contents_appends_nothing` | `Store::open` on a CBOR file, merged with the same records: `added == 0` |

Coverage of every record type (threads, views, withdrawals, resolutions, unknown records) is smysl's
own `crates/smysl-graph/tests/merge_idempotence.rs`, not repeated here.

## R11–R15 through the 1.4.0 CLI

| # | Check | Result |
|---|---|---|
| R11 | `smysl import --format surface --key test r.csv` | **open**: writes CBOR, exit 0, and `fmt --check` on the output exits 2 |
| R12 | the same import, then `smysl check` | **open**: `SMY-E022: gist is 50 tokens, default allows 30`, exit 3; the same without `--key` |
| R13 | `ingest.path: "bogus"` in `.smysl/config.hjson`, then `smysl providers` | **open**: a clear message, but exit 1 (`ingest` with a bad provider exits 6) |
| R14 | a provider with `kind: nosuchkind`, then `smysl providers` | **open**: still "malformed provider response: provider kind `nosuchkind` is not compiled into this build" |
| R15 | json-ast `GIST_MAX_CHARS` against check's bound | **fixed**: `GIST_MAX_CHARS = 120`, with a test holding it at four bytes per L0 token |

A detail found with R13: a non-string `ingest.path` (`path: 42`) is reported as "`ingest.path` is ``",
naming an empty value rather than the number.

## When 1.4.0 is on crates.io

1. Set the workspace dependency to `smysl = { version = "1.4", … }` and update `Cargo.lock`.
2. Remove the `#[ignore]` from `tests/guarantees.rs` and `tests/acceptance_1_4.rs`.
3. Update the plan's §1 pin, and adopt the 1.4 lifecycle records for review (D15).
