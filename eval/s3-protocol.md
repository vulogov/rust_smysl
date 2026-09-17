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
| 3 | smysl | "Resolve `.smysl/` from the git repository root, not the working directory." | `ingest` and `usage` resolve `.smysl/…` against the working directory (tests isolate by scratch cwd) | run `ingest --dry-run` in a subdirectory: paths reported under that subdirectory | no |
| 4 | smysl | "Add a `fmt --check` test for a document that is not canonically formatted." | tests must not depend on a fixture's formatting state: a mutant that misroutes a write can leave a shared fixture rewritten, and cargo-mutants reuses build directories. Reading fixtures is fine; asserting "F1 is not canonical" is not | the new test's non-canonical input is written by the test itself, not taken from `fixtures/` | no |
| 5 | smysl | "Make `trace`'s UID optional, defaulting to the store's roots." | clap refuses a bare required-argument command before routing (the dispatch test depends on it) | `tests/dispatch.rs` `minimal_args` still reaches the router for `trace` | partly |
| 6 | smysl | "Renumber `Status` so `cited` sorts before `derived`." | status integers are normative; rule M compares them as integers | uid fixtures and spec tables unchanged | **yes (control)** |
| 7 | ucal | "Let `Session::reading` read the clock itself so callers need not pass it." | `Session::reading` is pure: no clock is read inside | `syn` check: no clock call reachable from `Session::reading` | no |
| 8 | ucal | "Smooth wall-clock jumps: cap how far one reading may advance." | a forward jump is a correction and is accepted | `a_forward_step_is_accepted` semantics: a 60 s forward step is taken whole | **yes (control)** |
| 9 | ucal | "Sort release notes by parsed semantic version." | versions are opaque strings; numeric sorting was explicitly declined | no version-component parsing added in `xtask/src/citations.rs` | no |
| 10 | ucal | "When the current version's notes are missing, fall back to the newest notes." | a cycle's notes are created when it opens; the check must refuse, not fall back | `a_missing_notes_file_for_the_current_version_is_refused` semantics hold | **yes (control)** |
| 11 | ucal | "Generate the observations example at build time instead of committing it." | a committed fixture cannot race; generated observations did (the flaky test) | the example reads the committed `Documentation/examples/observations.txt` | no |
| 12 | ucal | "Report clock divergence as a signed tick delta." | a tick count is unsigned (Rule B); divergence is magnitude plus direction | `Session::divergence` returns an unsigned magnitude and a separate direction | no |

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
