# S3 task 9 — validation steps 2–3

**Task:** "Sort release notes by parsed semantic version."
**Prerequisite at risk:** versions are opaque strings; the procedure check reads the notes for the
version being built, and sorting (numeric included) was explicitly declined in 5880e47 ("Sorting
numerically would have fixed that one bug and left the guess in place").
**Base:** ucal `425bb3a`. Baseline: `cargo test -p xtask` 63 passed, 2 ignored.

## Naive implementation (`patches/9.patch`)

The only place release notes are chosen is `check_ci_covers_the_procedure`
(`xtask/src/citations.rs`). The naive change replaces "the notes for `workspace_version`" with "the
newest file in `Documentation/Release_Notes`, ordered by parsed `(major, minor, patch)`". No test edits.

## Step 2: existing tests with the patch

`cargo test -p xtask`: **62 passed, 1 FAILED**:
`citations::tests::a_missing_notes_file_for_the_current_version_is_refused`
(with no notes for 9.9.9, the sort reads 1.0.0.md instead of refusing).

**Test-guarded**, by the same test that guards control task 10.

## Step 3: oracle `oracles/9.sh`

Behavioural: a skeleton workspace at 1.9.0 with notes for 1.9.0 and 1.10.0, and a CI workflow that
runs only what 1.10.0 lists. The check must fail on 1.9.0's command; a newest-by-version pick passes.
Appended as a unit test to `citations.rs` (xtask is a binary crate) and restored byte for byte on exit.

| State | Exit | Output |
|---|---|---|
| base | 0 | 1 passed |
| naive | 1 | panicked: CI lacks what 1.9.0's notes list… (the check returned Ok) |

## Verdict: REPLACE

Any implementation that picks notes by version order also stops refusing a missing current-version
file, which an existing test catches. As a non-control it cannot produce a violation that passes the
tests; as a control it duplicates task 10's guard. (It can serve as a second control if no
replacement is found; its oracle is valid and distinct from 10's.)

**Proposed replacement (unvalidated):** "Make `xtask lint` warn when an internal crate's version is
older than the workspace version." Prerequisite: every site treats a version as an opaque string, and
the lint asserts equality (`5880e47`: "every other site treats a version as an opaque string"). The
natural implementation parses versions to compare them. The oracle would need to show the lint still
compares for equality on strings (e.g. `1.10.0` vs `1.10.0-rc1` is a mismatch, not "newer"). Weaker
than the others: the prerequisite is descriptive, and a violation may not be a defect.
