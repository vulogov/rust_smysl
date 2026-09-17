# S3 task 11 — validation steps 2–3

**Task (original):** "Generate the observations example at build time instead of committing it."
**Prerequisite at risk:** a committed fixture cannot race. Two test binaries once wrote the residuals
observations to one fixed path under `std::env::temp_dir()`, and the resulting flake passed every
local run and failed in CI on the cut commit (cc3aafe).
**Base:** ucal `425bb3a`. Baseline: `cargo test -p ucal --test json_surface --test manual_fields`
3 + 3 passed; `cargo test -p xtask` 63 passed.

## Original task: naive implementation (`patches/11.patch`)

Delete `Documentation/examples/observations.txt` (and ignore it); add `crates/ucal/build.rs` writing it
into `OUT_DIR`; both tests read `concat!(env!("OUT_DIR"), "/observations.txt")`; the xtask examples
generator writes the file into `Documentation/examples/` before running the manual's example.

**Step 2:** `json_surface` 3 passed, `manual_fields` 3 passed, `xtask` 63 passed.

**Step 3, oracle `oracles/11.sh`** (static, see below): base exit 0; naive exit 1 ("observations.txt is
missing from the working tree").

**Problem:** a build script is a single writer, so this implementation **cannot race**. It breaks the
decision (a committed fixture a reader can open) but not the prerequisite. A task whose natural
implementation leaves the prerequisite intact cannot measure whether the corpus protects it.

## Replacement: validated (`patches/11-replacement.patch`)

**Task:** "Stop committing `observations.txt`; have the tests write the observations they need."

Naive implementation: delete the committed file; in both `json_surface.rs` and `manual_fields.rs`, write
the four observations to `std::env::temp_dir().join("ucal-residuals-observations.txt")` and pass that
path to `cmd_ephem_residuals`, which is the pre-cc3aafe shape of the race.

**Step 2:** `cargo test -p ucal --test json_surface --test manual_fields`, run **3 times: all passed**
(3 + 3 each time). The race is scheduling-dependent and does not show in a local run, which is exactly
how it shipped.

**Step 3:** `oracles/11.sh` on the replacement: exit 1 (fixture missing; the check for a run-time
`temp_dir` write in either test file would also fire). Base: exit 0.

### Why the oracle is static

A behavioural run cannot expose the race reliably: the original passed every local run. So the oracle
checks the conditions that make the race impossible:
1. `Documentation/examples/observations.txt` exists, is tracked, and is not ignored;
2. both residuals tests read that committed path;
3. neither writes a file under `temp_dir()` at run time.

Comments are excluded, because both test files describe the old race in a doc comment (the first
version of the oracle fired on it at base).

## Verdict: REPLACE → use the validated replacement

Replace the protocol's task 11 with the replacement above. Keep `oracles/11.sh`, which serves both.
