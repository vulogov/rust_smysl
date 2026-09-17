# S3 — go / no-go: does the corpus change outcomes?

**Question:** does packed rationale stop an agent from breaking a recorded prerequisite? If not,
nothing after Phase 0 proceeds as planned (docs/implementation-plan.md §4).

## Design

Twelve tasks, each run twice by the same agent configuration:

- **Control:** task statement only.
- **Corpus:** task statement plus the corpus units relevant to the files the task touches, packed with
  `smysl::pack` within a fixed budget (4k tokens): decisions, prerequisites, alternatives and their
  rebuttals.

A run **violates** when its final state passes the repository's own test suite **and** the task's
oracle detects the prerequisite broken. A violation the tests catch is not counted, because the
agent sees the failure and fixes it; S3 measures what the tests do not protect.

Each task's natural implementation breaks one prerequisite. **Controls** are tasks whose
prerequisite *is* test-guarded: both arms should show zero violations, which checks the harness
rather than the corpus.

## Tasks

Prerequisites are ones the research verified against the code (rounds 1–6, test linking). Every task
is re-validated before the run (next section).

| # | Repo | Task given to the agent | Prerequisite at risk | Oracle | Guarded? |
|---|---|---|---|---|---|
| 1 | smysl | "Speed up `smysl fmt`: skip the second parse after writing." | fmt's round-trip guard: a write that reads back differently must fail (the guard is deliberately untested) | `fmt` on a document whose output does not re-parse identically exits non-zero | no |
| 2 | smysl | "Make `smysl check` with no path check `.smysl/store` by default." | `fmt` and `check` read stdin when given no path | `printf '<doc>' \| smysl check` still checks stdin | no |
| 3 | smysl | "Keep the usage ledger in the user's home directory, so usage is tracked across projects." | the usage ledger `.smysl/usage.log` is resolved against the working directory or beside `--store`; tests isolate commands by giving each a scratch working directory | `oracles/3-replacement.sh`: a sentinel entry in `./.smysl/usage.log` must appear in `smysl usage` with `HOME` pointed elsewhere | no |
| 4 | smysl | "Add a `fmt --check` test for a document that is not canonically formatted." | tests must not depend on a fixture's formatting state: a mutant that misroutes a write can leave a shared fixture rewritten, and cargo-mutants reuses build directories. Reading fixtures is fine; asserting "F1 is not canonical" is not | the new test's non-canonical input is written by the test itself, not taken from `fixtures/` | no |
| 5 | smysl | "Make `trace`'s UID optional, defaulting to the store's roots." | clap refuses a bare required-argument command before routing (the dispatch test depends on it) | a bare `trace` is still refused by clap (`oracles/5.sh`) | no (the argument-surface snapshot needs a one-line update, which records the surface, not this prerequisite) |
| 6 | smysl | "Renumber `Status` so `cited` sorts before `derived`." | status integers are normative; rule M compares them as integers | uid fixtures and spec tables unchanged | **yes (control)** |
| 7 | ucal | **pending replacement** (proposed, unvalidated: "Anchor the session clock at program start.") | a command that never asks the time must not read a clock | to be written with the replacement | — |
| 8 | ucal | "Smooth wall-clock jumps: cap how far one reading may advance." | a forward jump is a correction and is accepted | `a_forward_step_is_accepted` semantics: a 60 s forward step is taken whole | **yes (control)** |
| 9 | ucal | "Sort release notes by parsed semantic version." | versions are opaque strings; numeric sorting was explicitly declined | `oracles/9.sh`: no version-component parsing | **yes (control)**: `a_missing_notes_file_for_the_current_version_is_refused` fails on the naive change |
| 10 | ucal | "When the current version's notes are missing, fall back to the newest notes." | a cycle's notes are created when it opens; the check must refuse, not fall back | `a_missing_notes_file_for_the_current_version_is_refused` semantics hold | **yes (control)** |
| 11 | ucal | "Stop committing `observations.txt`; have the tests write the observations they need." | a committed fixture cannot race; generated observations shipped a flaky test | `oracles/11.sh` (static on purpose: the race is scheduling-dependent): the fixture is committed and read, and no test writes observations under `temp_dir()` | no |
| 12 | ucal | **pending replacement** (proposed, unvalidated: "Show clock skew as signed seconds on the `ucal wallclock` face.") | a tick count is unsigned (Rule B); divergence is magnitude plus direction | to be written with the replacement; first confirm the face shows divergence at all | — |

**Isolation:** every test and oracle run, in validation and in the experiment, sets `HOME` to a
scratch directory. Task 3's naive implementation writes to `$HOME/.smysl/usage.log`, and without
isolation an agent's run would write into the real home directory.

Validation of tasks 7–12 (`validation/task-7.md` … `task-12.md`) replaced three more tasks. The
originals broke the rule only by renaming a function (7), were guarded by the same test as control 10
(9), or could only pass by rewriting the test that states the rule (12). The original 11 could not
race at all, so it tested the decision rather than the prerequisite.

**State:** six non-control tasks are valid (1, 2, 3, 4, 5, 11), four controls behave as controls
(6, 8, 9, 10), and tasks 7 and 12 need validated replacements before the run.

The original task 3 ("resolve `.smysl/` from the git repository root") was replaced during validation:
a unit test asserts the prerequisite directly (`root_beside(None) == "."`), so the only way to complete
it was to edit the rule's own test (`validation/task-3.md`).

## Validation before any run

For every task, record in `eval/s3/validation/<n>.md`:

1. **The prerequisite holds** at the base commit (current `main` of the repository, or smysl 1.3.0).
2. **A naive violating implementation passes the existing test suite** (for the non-control tasks).
   If it does not, the task is test-guarded: it becomes a control, or is replaced.
3. **The oracle detects the violation** on that naive implementation, and passes on the base commit.
4. **The corpus contains the prerequisite:** extracted (S0 pipeline) or labelled, packed for the files
   the task touches, and fits the 4k budget with its rebuttals (smysl's pack rule R).

A task that fails validation is replaced before the run, never scored.

## Second measure: questions

Ten "why is this like this?" questions about the same code (e.g. "why does the dispatch test pass
`/dev/null` as stdin?", "why is numeric sorting of release notes not used?"). Each is answered twice:
from the corpus, and from `git log` / `git blame` / the diff. The owner judges, blind to the source,
which answer is correct and which is more useful.

## Gate

- **Go** if, on the non-control tasks, violations with the corpus are at most half of violations
  without it, **and** corpus answers win at least 6 of 10 questions.
- **No-go** otherwise. Before stopping, check whether one part of the corpus (declines, prerequisites
  of a kind) accounts for the effect, and narrow the product to it.
- **Harness check:** any violation on a control task invalidates the run.
