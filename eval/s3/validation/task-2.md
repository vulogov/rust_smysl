# S3 task 2: validation steps 2–3

**Task:** "Make `smysl check` with no path check `.smysl/store` by default."
**Prerequisite at risk:** `fmt` and `check` read stdin when given no path (`tests/dispatch.rs` passes `/dev/null` as stdin because of it; the manual pipes into `check -`).
**Base:** smysl `183be01`. Baseline: 132 tests, all passing.

## Naive implementation
`patches/2.patch`: in `cmd_check`, the no-path, no-`--store` default changes from `vec!["-"]` to `vec![".smysl/store"]`. No test edits.

## Step 2: existing tests
`cargo test -p smysl --tests`: **132 passed, 0 failed.** The manual transcripts use `smysl check -` explicitly, and `dispatch` only looks for the not-registered, not-wired and not-reached markers.

## Step 3: oracle
`oracles/2.sh <repo> <bin>`: **behavioural.** Pipes a valid one-unit document into `smysl check` with no path, in an empty directory, and expects it to be checked ("1 unit").
- base: exit 0 (intact)
- naive: exit 1 ("`.smysl/store: No such file or directory`")

## Verdict: VALID
