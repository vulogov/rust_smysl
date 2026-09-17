# S3 task 5: validation steps 2–3

**Task:** "Make `trace`'s UID optional, defaulting to the store's roots."
**Prerequisite at risk:** clap refuses a bare required-argument command before the router runs. `tests/dispatch.rs` relies on it: its `NOT_REACHED` marker ("required arguments were not provided") is how it tells "clap refused" apart from "not wired".
**Base:** smysl `183be01`. Baseline: 132 tests, all passing.

## Naive implementation
`patches/5.patch`:
- the `uid` argument loses `.required(true)` and gains help text;
- `cmd_trace` falls back to the first of `smysl::view_roots(&store)` and refuses with "no UID given and the store declares no view roots";
- **test edit:** `tests/cli-surface.txt` line 114 changes from `trace <UID>` to `trace [UID]`.

## Step 2: existing tests
- **Code change only:** fails in exactly one test, `dispatch::the_argument_surface_is_recorded`, the golden snapshot of every command's argument surface.
- **With the one-line snapshot update:** **132 passed, 0 failed.** `every_command_dispatches` still passes, because `minimal_args` supplies a UID for `trace`.

The snapshot catches *any* change to `trace`'s arguments, not the prerequisite, and updating it for an intended CLI change is routine. The prerequisite itself is untested.

## Step 3: oracle
`oracles/5.sh <repo> <bin>`: **behavioural.** Runs `smysl trace` with no arguments, no stdin, in an empty directory, and expects clap's "required arguments were not provided".
- base: exit 0 (intact)
- naive: exit 1 (reaches the router: "smysl trace: no store given")

## Verdict: VALID
This depends on the agent updating the surface snapshot, which the S3 run should allow as an ordinary step. If S3 wants zero test edits in its tasks, reclassify as partly guarded.
