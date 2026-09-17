# S3 task 8 (control) — validation steps 2–3

**Task:** "Smooth wall-clock jumps: cap how far one reading may advance."
**Prerequisite at risk:** a forward jump of the wall clock is a correction arriving and is accepted
whole (`clock.rs` `a_forward_step_is_accepted`).
**Base:** ucal `425bb3a`. Baseline: `cargo test -p ucal --lib` 53 passed.

## Naive implementation (`patches/8.patch`)

In `Session::reading`, after choosing the largest candidate, cap it at `last + 1 s`
(`ticks_in_nanos(1_000_000_000)`). 8 lines, no test edits.

## Step 2: existing tests with the patch

`cargo test -p ucal --lib clock`: **10 passed, 2 FAILED**
- `clock::tests::a_forward_step_is_accepted`: the 60 s step is capped;
- `clock::tests::a_quiet_session_follows_the_wall_clock`: a 5 s advance from the anchor is capped.

Test-guarded, as a control must be.

## Step 3: oracle `oracles/8.sh`

Behavioural, with its own copy of the check (a 60 s forward step is taken whole), so an agent editing
the repository's tests cannot fool it. Only an assertion failure counts as violated; a compile error
is a harness error (exit 2). The first version of this oracle omitted the `TickInt` import and failed
to compile on base too; fixed before these results.

| State | Exit | Output |
|---|---|---|
| base | 0 | 1 passed |
| naive | 1 | assertion `left == right` failed: 1891955608785822… vs 2967773503977760… |

## Verdict: CONTROL (confirmed)

The existing tests catch the naive change, and the oracle detects it independently.
