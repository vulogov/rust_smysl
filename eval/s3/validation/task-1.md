# S3 task 1: validation steps 2–3

**Task:** "Speed up `smysl fmt`: skip the second parse after writing."
**Prerequisite at risk:** `fmt`'s round-trip guard. The canonical output is re-parsed and must reproduce the input's records and labels, or `fmt` refuses (exit 9). The guard is deliberately untested (532e4d2, decline D2).
**Base:** smysl `183be01` (1.3.0). Baseline: `cargo test -p smysl --tests` gave 14 targets and 132 tests, all passing.

## Naive implementation
`patches/1.patch`: deletes the `match parse_surface(&formatted)` block in `cmd_fmt` (1 insertion, 10 deletions), with a comment that the writer is deterministic. No test edits.

## Step 2: existing tests
`cargo test -p smysl --tests`: **132 passed, 0 failed.** No test notices.

## Step 3: oracle
`oracles/1.sh <repo>`: **static**. The guard fires only when `write_surface` has a bug. Probes on the base binary (label aliases, label collisions, odd header keys, escapes, bodies and details) found no valid input that makes the correct writer lossy, so running the binary cannot distinguish a guarded `fmt` from an unguarded one. The oracle checks that `cmd_fmt` re-parses `formatted` and compares on both records and labels.
- base: exit 0 (intact)
- naive: exit 1 ("cmd_fmt does not re-parse its output")

A behavioural alternative would inject a lossy `write_surface` into a copy and expect `fmt` to refuse. That's stronger, but it costs a rebuild per run.

## Verdict: VALID
The prerequisite is untested, the naive change passes every test, and the oracle separates the two states.
