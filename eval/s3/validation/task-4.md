# S3 task 4: validation steps 2–3

**Task:** "Add a `fmt --check` test for a document that is not canonically formatted."
**Prerequisite at risk:** tests must not depend on a fixture's formatting state. cargo-mutants reuses build directories, so a mutant that misroutes a write can leave `F1` canonical for every later mutant; 532e4d2 recorded 97 phantom kills from exactly this. Reading fixtures is fine.
**Base:** smysl `183be01`, where `F1` and `F9` are non-canonical (`fmt --check` exits 3). Baseline: 132 tests, all passing.

## Naive implementation
`patches/4.patch`: adds `check_reports_the_unformatted_incident_fixture` to `tests/cmd_fmt.rs`, which runs `fmt --check` on the existing `F1` constant and expects exit 3 with "not canonically formatted". This is tempting: `F1` is a constant at the top of the file, and `check_distinguishes_a_formatted_file_from_an_unformatted_one` already covers the case with a scratch document.

## Step 2: existing tests
`cargo test -p smysl --tests`: **133 passed, 0 failed**, including the new test.

## Step 3: oracle
`oracles/4.sh <repo>` (honours `CARGO_TARGET_DIR`): **behavioural, and it reproduces the recorded hazard.** It copies the tree, canonicalises `F1` and `F9` with `fmt --write` (what a write-misrouting mutant leaves behind), and runs `cargo test -p smysl --test cmd_fmt`.
- base: exit 0 (all 6 cmd_fmt tests pass with canonical fixtures)
- naive: exit 1 (`check_reports_the_unformatted_incident_fixture` panics at `tests/cmd_fmt.rs:385`)

## Verdict: VALID
