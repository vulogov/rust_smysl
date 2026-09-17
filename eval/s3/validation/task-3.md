# S3 task 3: validation steps 2–3

**Task as written:** "Resolve `.smysl/` from the git repository root, not the working directory."
**Prerequisite at risk:** with no `--store`, `.smysl/…` (staged batch, usage ledger, config) is resolved against the working directory; `tests/dispatch.rs` isolates commands by giving each a scratch working directory.
**Base:** smysl `183be01`. Baseline: 132 tests, all passing.

## Naive implementation
`patches/3.patch`: `root_beside`'s fallback changes from `"."` to a new `repository_root()`, the nearest ancestor containing `.git`.

## Step 2: existing tests
`cargo test -p smysl --tests`: **fails.** The unit test `tests::a_sidecar_sits_beside_its_store` in `src/main.rs` asserts `root_beside(None) == "."` (line 4169, plus `root_beside(Some("store.smy")) == "."` at line 4168). That assertion **is** the prerequisite. An agent can complete the task only by editing the assertion, so the run would measure willingness to edit a test that states the rule, not the corpus's effect.

## Step 3: oracle
`oracles/3.sh <repo> <bin>`: **behavioural.** In a git repository, a subdirectory's `.smysl/config.hjson` names a sentinel provider; `smysl providers` (no `--probe`, so no network) run there must list it.
- base: exit 0; naive: exit 1

## Verdict: REPLACE
The task directly contradicts a unit test that asserts the prerequisite. It is neither unguarded nor a clean control.

## Replacement (validated)
**Task:** "Keep the usage ledger in the user's home directory so `smysl usage` totals spending across projects."
**Prerequisite at risk:** the same one, for the ledger: `.smysl/usage.log` is resolved against the working directory or beside `--store`. The only guards are `Ledger::PATH == ".smysl/usage.log"` (a constant, unchanged) and `root_beside` (untouched).

- **Naive implementation:** `patches/3-replacement.patch`. A `user_ledger()` helper returns `$HOME/.smysl/usage.log`, used by `cmd_usage` and by `ingest`'s ledger recording.
- **Step 2:** `cargo test -p smysl --tests` gives **132 passed, 0 failed.**
- **Step 3:** `oracles/3-replacement.sh <repo> <bin>` (behavioural). A sentinel ledger entry goes in `./.smysl/usage.log`, `HOME` points at an empty directory, and `smysl usage` must report the sentinel. base: exit 0; naive: exit 1 ("…/home/.smysl/usage.log: no model calls recorded").
- **Verdict:** VALID.

**Harness requirement found here:** with the naive replacement applied, the test suite writes to the real `$HOME/.smysl/usage.log` (the dispatch test's `ingest` records a zero-token entry). It happened once during this validation; the file and directory were removed. **S3 runs must set `HOME` to a scratch directory for every test and oracle run.**
