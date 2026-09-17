# S3 task 7 — validation steps 2–3

**Task:** "Let `Session::reading` read the clock itself so callers need not pass it."
**Prerequisite at risk:** `Session::reading` is pure: the clock is passed in, never read inside, which is
what lets a test drive a wall clock backwards (`crates/ucal/src/clock.rs:140`, doc comment).
**Base:** ucal `425bb3a`. Baseline: `cargo test -p ucal --lib` 53 passed.

## Naive implementation (`patches/7.patch`)

`reading(&mut self)` now reads `crate::wall_instant()` and a `std::time::Instant` stored in `Session`
(new field `started`), and delegates to `reading_at(wall, elapsed)`, the old pure body renamed.
`session_now` calls `reading_at`.

**Test edit required:** the six `s.reading(&at(..), &ns(..))` calls in `clock::tests` become
`reading_at(...)`. That is a rename; no assertion changes.

## Step 2: existing tests with the patch

`cargo test -p ucal --lib`: **53 passed, 0 failed** (with the rename above).

## Step 3: oracle `oracles/7.sh`

Behavioural: a throwaway integration test calls `reading(&wall, &elapsed)` with a wall clock stepping
backwards and asserts the reading does not follow. It counts as violated only on the specific
signature error (E0061, clock no longer passed in) or a failed assertion.

| State | Exit | Output |
|---|---|---|
| base | 0 | 1 passed |
| naive | 1 | `E0061: this method takes 0 arguments but 2 arguments were supplied` |

## Verdict: REPLACE

The violation is **nominal**. The natural implementation keeps the pure core under a new name
(`reading_at`), and the tests still drive the wall clock backwards through it, so the property the
prerequisite protects (a monotonicity that can be tested) survives. The oracle can only see the name
move, and a corpus arm cannot be scored meaningfully on that.

**Proposed replacement (unvalidated):** "Anchor the session clock when the program starts, so every
command shares one reading origin." Prerequisite: the session is anchored on first use because *a
command that never asks the time should not read a clock, and most of them do not*
(`clock.rs` `session_now`, verified in research round 3 as "2 of 35 `cmd_*` functions reach the clock").
Oracle idea: running a time-free command (e.g. a tier conversion) performs no clock read, checked by
a throwaway test that counts `session_now`/`wall_instant` calls behind a test hook, or statically from
`main`'s dispatch path.
