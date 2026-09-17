# smysl 1.4.0: acceptance from rust_smysl, before publish

**Run 2:** 2026-09-17, against `../smysl` on `dev/1.4.0` at `c7bf5a3` plus uncommitted work: R12 in
`smysl-ingest/src/import.rs`, the `ingest.path` type message in `smysl-provider/src/config.rs`, the TUI
contention status, version checks for the Go, Node and Python implementations, and the architecture RFC
(14 files, diff sha256 `955a7b19…`). A snapshot of that state was tested, with rust_smysl patched to it
through `[patch.crates-io]`. smysl's own suite (`make ci`) was not run here.

**Run 1**, earlier the same day on `c7bf5a3` with the TUI and version-check work only: R10 passed, and
R11–R14 were open. Run 2 repeats every check and adds R12.

## Verdict

**Ready to publish, as far as rust_smysl is concerned.** R10 and R12, the two requests that affect the
tool, pass their acceptance. Moving the pin from 1.3 to 1.4 changes nothing rust_smysl produces. R11,
R13 and R14 are open on purpose: smysl's changelog keeps them as S2 tasks. None of them blocks
rust_smysl.

## rust_smysl on 1.4.0

| Check | Result |
|---|---|
| `cargo test --workspace` | all pass, nothing ignored: `merging_a_store_into_itself_adds_no_records` and the five acceptance tests below, all ignored on 1.3, pass |
| `cargo clippy --workspace --all-targets -D warnings` | clean |
| `smysl-eval stage` for research-pro-v2, research-flash-v2 and research-deepseek-v2 | 0 staging errors; per-commit units, edges and quote tallies **identical** to 1.3 |
| `smysl-eval s3-context` (22 packed contexts) | **byte-identical** to 1.3, so S3 run2's contexts do not depend on the version |

## R10 and R12 acceptance (`crates/cargo-smysl-corpus/tests/acceptance_1_4.rs`)

All five pass on 1.4.0 and fail on 1.3.0. They are committed ignored until the pin moves.

| Test | Acceptance item |
|---|---|
| `r10_repeated_self_merge_appends_nothing` | `merge(A, A)` four times on a real staged batch: `added == 0`, record count unchanged |
| `r10_two_commits_merged_both_ways_then_again_are_stable` | A∪B and B∪A hold the same records; merging one into the other appends nothing |
| `r10_a_label_bound_to_a_different_uid_is_still_appended` | a rival binding is appended (`added == 1`), and the collision is detected from the store's own bindings |
| `r10_a_store_opened_from_a_file_merged_with_its_own_contents_appends_nothing` | `Store::open` on a CBOR file, merged with the same records: `added == 0` |
| `r12_an_imported_reading_with_a_long_key_checks_clean_and_keeps_every_cell` | `from_csv` with a 100-character key, 27 columns and a 300-byte cell, with `--key test` and without: no check errors, and every column name and cell in the payload |

Coverage of every record type (threads, views, withdrawals, resolutions, unknown records) is smysl's
own `crates/smysl-graph/tests/merge_idempotence.rs`, not repeated here.

## R11–R15 through the 1.4.0 CLI

| # | Check | Run 1 | Run 2 |
|---|---|---|---|
| R11 | `smysl import --format surface --key test r.csv` | open: writes CBOR | **open** (kept for S2): still CBOR |
| R12 | the same import (and without `--key`), then `smysl check` | open: `SMY-E022`, exit 3 | **fixed**: "4 records, 2 units, 0 diagnostic(s)", exit 0, both ways |
| R13 | `ingest.path: "bogus"`, then `smysl providers` | open: exit 1 | **open** (kept for S2): exit 1 |
| R14 | a provider with `kind: nosuchkind`, then `smysl providers` | open | **open** (kept for S2): still "malformed provider response", then "no providers configured" |
| R15 | json-ast `GIST_MAX_CHARS` against check's bound | fixed | **fixed**: 120 |

`ingest.path: 42`, reported in run 1 as "`ingest.path` is ``", now reads "`ingest.path` is an integer".

## When 1.4.0 is on crates.io

1. Set the workspace dependency to `smysl = { version = "1.4", … }` and update `Cargo.lock`.
2. Remove the `#[ignore]` from `tests/guarantees.rs` and `tests/acceptance_1_4.rs`.
   R12 changes the uid of an imported row whose gist 1.3 overflowed, which never checked, so no stored
   evidence is affected.
3. Update the plan's §1 pin, and adopt the 1.4 lifecycle records for review (D15).
