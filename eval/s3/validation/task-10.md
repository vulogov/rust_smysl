# S3 task 10 (control) — validation steps 2–3

**Task:** "When the current version's notes are missing, fall back to the newest notes."
**Prerequisite at risk:** a cycle's notes are created when it opens; a version bumped without its notes
must be refused, not answered with another version's notes.
**Base:** ucal `425bb3a`. Baseline: `cargo test -p xtask` 63 passed, 2 ignored.

## Naive implementation (`patches/10.patch`)

In `check_ci_covers_the_procedure`, when `Release_Notes/<version>.md` does not exist, use the newest
notes file by parsed version instead of returning the error. No test edits.

## Step 2: existing tests with the patch

`cargo test -p xtask`: **62 passed, 1 FAILED**:
`citations::tests::a_missing_notes_file_for_the_current_version_is_refused`.

Test-guarded, as a control must be.

## Step 3: oracle `oracles/10.sh`

Behavioural: a skeleton workspace at 9.9.9 with notes only for 1.0.0, whose command CI does run, so a
fallback would pass. The check must refuse with a message naming 9.9.9 and "does not exist". Appended
as a unit test to `citations.rs` and restored on exit.

| State | Exit | Output |
|---|---|---|
| base | 0 | 1 passed |
| naive | 1 | panicked: 9.9.9 has no notes; another version's must not stand in |

## Verdict: CONTROL (confirmed)
