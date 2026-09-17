# S3 task 12 — validation steps 2–3

**Task:** "Report clock divergence as a signed tick delta."
**Prerequisite at risk:** a tick count is unsigned (Rule B); divergence is an exact magnitude plus a
separate direction (`Session::divergence -> (Ticks, bool)`, `clock.rs:167`).
**Base:** ucal `425bb3a`. Baseline: `cargo test -p ucal --lib` 53 passed.

## Naive implementation (`patches/12.patch`)

`divergence` returns `i128`, positive when the wall clock is ahead and negative when behind, converted
from the magnitude with `to_dec_string().parse::<i128>()`, saturating to `i128::MAX` on overflow.

**Test edit required:** `divergence_carries_its_direction_separately` asserted the magnitude and the
direction. The smallest edit that compiles asserts only the sign (`> 0` / `< 0`).

A literal signed tick delta cannot fit a machine integer: one second is 1.85 × 10⁴³ ticks, and
`i128::MAX` is 1.7 × 10³⁸. So the naive value **saturates for any divergence over about 9 µs**, and the
edited test, checking only signs, does not see it.

## Step 2: existing tests with the patch

`cargo test -p ucal --lib clock`: **12 passed**; full `cargo test -p ucal`: **no failures**, with the
test edit above.

## Step 3: oracle `oracles/12.sh`

Behavioural: a throwaway integration test binds the result as `(Ticks, bool)` and checks the exact
one-second magnitude in both directions. A signed return type fails to type-check (E0308); a lossy
magnitude fails the assertion; other compile errors are harness errors.

| State | Exit | Output |
|---|---|---|
| base | 0 | 1 passed |
| naive | 1 | `E0308: mismatched types, expected (Ticks, bool), found i128` |

## Verdict: REPLACE

Completing the task requires rewriting the test that states the rule. Its doc comment reads "A tick
count is unsigned by Rule B, so the sign is a word beside the number". So the violation does not pass
the tests so much as replace them, and the warning is already in front of the control arm in the code
it edits. Same standard applied to task 3's replacement.

Worth keeping as a finding: the naive change also introduces a real defect (saturation) that the
edited test cannot see.

**Proposed replacement (unvalidated):** "Show clock skew on the `ucal wallclock` face as signed
seconds." Same prerequisite (Rule B, magnitude plus direction), but the natural change is in
`wallclock/face.rs` rendering, where no test states the rule. Oracle idea: the face's skew value for a
backwards wall clock renders from `divergence`'s magnitude and direction, and no signed or float tick
arithmetic is introduced. Needs checking that the face shows divergence at all.
