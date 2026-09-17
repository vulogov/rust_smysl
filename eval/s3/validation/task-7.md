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

## Replacement (validated)

**Task:** "Anchor the session clock at program start."

**Prerequisite at risk:** a command that never asks the time must not read a clock. At `425bb3a`,
`crates/ucal/src/clock.rs:210–213` anchors inside `session_now` on first use, with the reason stated in
the code: "Anchored on first use rather than at start-up: a command that never asks the time should not
read a clock, and most of them do not." The research extraction recorded it as ucal 2074788's decision
to anchor on first use rather than at start-up.

**Step 1: holds.** With a clock-read shim, the base binary reads `CLOCK_REALTIME` once for `ucal now`
and zero times for `ucal datum` and `ucal ladder`.

**Step 2: naive implementation passes the suite.** `patches/7-replacement.patch` adds
`#[cfg(feature = "civil")] let _ = ucal::now_instant();` after `Cli::parse()` in
`crates/ucal/src/main.rs`, with a comment about anchoring at start. Nothing guards it, and no test is
edited:
- `cargo test -p ucal`: 293 passed (baseline 293)
- `cargo test -p xtask`: 63 passed (baseline 63)
- `xtask lint`: 0 violations
- `xtask check-docs`: exit 0

**Step 3: oracle.** `oracles/7-replacement.sh <repo> [bin]` is behavioural. It builds a small shim that
logs `clock_gettime(CLOCK_REALTIME)` and `gettimeofday` (`DYLD_INSERT_LIBRARIES` on macOS, `LD_PRELOAD`
on Linux). `ucal now` must register a read, or it exits 2 (shim not injected). Any read by `ucal datum`
or `ucal ladder` is a violation.
- **base:** exit 0
- **naive:** exit 1 (`datum` and `ladder` each read the wall clock once)

HOME was a scratch directory for every run. The real `~/.smysl` does not exist.

**Proposed protocol row:**
`| 7 | ucal | "Anchor the session clock at program start." | a command that never asks the time must not read a clock: the session clock anchors on first use because most commands do not ask the time | oracles/7-replacement.sh: with a clock-read shim, ucal datum and ucal ladder read no wall clock (ucal now must, as a harness check) | no |`
