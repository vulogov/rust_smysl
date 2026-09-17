# S3 questions: blind judging sheet
For each question, two answers, A and B, in random order: one written from the recorded corpus only, the other from git history and code only. Judge each on its own merits against your knowledge of the code.
Fill in each `correct` line with the answers that are correct (A, B, both, neither) and each `more useful` line with A, B or tie. Hashes and unit ids were replaced by [id] in both.

## 1. (smysl) Why does the dispatch test pass `/dev/null` as stdin to every command?

**A.** fmt and check read stdin when no path is given, so if stdin inherited the terminal, the dispatch test would hang waiting for input instead of failing. Setting stdin to /dev/null ensures those commands fail immediately rather than blocking the test.

**B.** The comment on the `dispatch` helper explains it directly: stdin is redirected to `/dev/null` rather than left inherited because two of the commands, `fmt` and `check`, read from stdin when no path is given. If the test harness were run interactively and let those commands inherit a real terminal as stdin, they would block waiting for input and the test would hang rather than fail. Using `/dev/null` guarantees an immediate EOF, so those commands fail fast (as expected, since they're being invoked without their required input) instead of hanging the whole test run.

- correct: 
- more useful: 

## 2. (smysl) Why does each dispatch-test command run in its own scratch directory?

**A.** Each command ran against a shared checkout when tested manually, and this dirtied the tree with `.smysl` files that a real command had written — for example, a usage command ended up reading a ledger that an ingest command had just written into. Running each command in its own scratch directory keeps the test from depending on or altering shared state, so results don't leak between commands and the working checkout stays clean.

**B.** Each command in `tests/dispatch.rs` gets its own scratch directory because running all twenty-two commands from the same working directory (the repo root, during manual probing) left behind state — `.smysl/staged.smy` and `.smysl/usage.log` — that later commands then read. The `usage` invocation, for instance, ended up reading a ledger that the `ingest` invocation had just written, so the commands were no longer independent of each other's side effects. Isolating every command in its own scratch directory prevents that cross-contamination: each invocation runs against a clean state, so a failure or false pass can't be caused by leftovers from a previous command in the same test run.

- correct: 
- more useful: 

## 3. (smysl) Why is the list of 22 subcommands hardcoded in `tests/dispatch.rs` rather than read from the binary?

**A.** A list derived from the binary would shrink silently when the binary shrinks, and regenerating a golden file from that shrunk output would record the absence of a command as intentional rather than as a regression. Hardcoding the 22 names means the test can only pass if the hardcoded list still matches the actual commands, so a dropped command shows up as a mismatch instead of being silently accepted.

**B.** The list is hardcoded on purpose, so the test's expectations don't come from the same binary being tested. If the names were instead parsed out of the binary's own `--help` output, deleting a command's registration would make it disappear from both the help text and the derived list at once—so the loop would simply stop testing that command and still report success. That defeats the whole point of the test, which exists specifically to catch a command silently losing its wiring or its arguments. A separate test then checks that the hardcoded list still matches what `--help` reports, in both directions, so the list can't quietly drift from the binary either. The one cost is that adding a new subcommand requires a human to update the array by hand, which is considered acceptable since the alternative is a test that can't fail.

- correct: 
- more useful: 

## 4. (smysl) Why is `fmt`'s round-trip guard not covered by a test?

**A.** The guard only fires when `write_surface` produces output that the parser reads back differently, and no user input can reach that path. A test cannot distinguish a guard that actually checks this from one that does nothing, since there's no way to trigger the mismatch it's meant to catch. It is instead recorded as a known survivor, with a pointer to the tests that cover the underlying property it protects. What would break otherwise: writing a test anyway would produce a check that can never fail, giving false confidence rather than real coverage.

**B.** The guard checks that re-parsing the just-formatted text still yields the same labels and records, but it only ever fires when `write_surface` itself is buggy — no valid input a user can supply drives the writer into producing text that parses back differently, so no test document can make the guard actually reject something. Two mutation-testing survivors mark this (replacing the check with `true`, and loosening the `&&` to `||`): they're unreachable, not untested, since nothing exercises the failing branch. The properties the guard exists to protect are still exercised indirectly: a versioning test asserts documents round-trip with their declared format version, and a surface test asserts records survive formatting. If a future change to the writer ever broke that invariant, this guard is what would make `fmt` refuse to write the damage to disk rather than silently corrupt a file.

- correct: 
- more useful: 

## 5. (smysl) Why do the `cmd_fmt` tests write their own documents instead of using the corpus fixtures?

**A.** I found the answer directly in both the code comments and the commit history.

The tests build their own documents because several tests need a document with a specific property — not canonically formatted, or containing comments — and that's a property of the file's content, not something the repository is guaranteed to preserve. The first version of these tests relied on corpus fixtures (F1 for "not canonical," F9 for "has comments") having those properties, and that broke under mutation testing: `cargo-mutants` reuses build directories, so a mutant that misroutes a write could leave a fixture rewritten on disk, and every subsequent mutant tested in that directory would then see an already-canonical F1. The tests failed spuriously because of this stale state, and those spurious failures were miscounted as genuine mutant kills — inflating the measured coverage by 97 mutants that hadn't actually been caught. So the rule adopted was: a test must not depend on the state of the tree, just as it must not depend on the state of the machine. Fixtures are still used as stable inputs (e.g., for `--strict` controls), but any test needing a specific formatting property writes its own scratch file for that purpose.

**B.** The first version depended on repository fixtures, but cargo-mutants reuses build directories, so a mutant that misroutes a write could leave a fixture rewritten, causing spurious failures that were incorrectly counted as caught mutants. Each test writing its own document with the needed property removes this dependency on fixture state. The rule drawn from this is that a test must not depend on the state of the tree, just as it must not depend on the state of the machine.

- correct: 
- more useful: 

## 6. (smysl) Why does the `COMMANDS.iter().find` `==`/`!=` mutant not have a test?

**A.** The mutant is treated as an equivalent mutant, not a real gap. Flipping `==` to `!=` in that lookup makes it select the wrong `Cmd` entry from the table, but the only field ever read off that `Cmd` is consumed in the `_ =>` fallback arm, which no actual input reaches. Since no observable behavior differs regardless of which `Cmd` is picked, no test could ever distinguish the mutated code from the original — so writing one would be chasing a mutation that can't manifest as a failure. It was left documented in place instead, following the same practice used earlier for the `worse` function's `>=` mutant.

**B.** Flipping the comparison to `!=` selects the wrong `Cmd`, but the only field read from that result is in the `_` arm, which is unreachable. Since no reachable code path exposes the difference between the two programs, a test cannot distinguish the correct version from the mutant, so one wasn't written and the mutation was instead recorded as equivalent.

- correct: 
- more useful: 

## 7. (ucal) Why does `Session::reading` take the wall clock and elapsed time as arguments instead of reading the clock?

**A.** Making it a pure function lets tests drive the wall clock backwards repeatedly and assert that the reading sequence stays monotonic. If it read the clock itself, the answer would not follow from its inputs in a way tests could check, and the rejected alternative—having it read the wall clock directly—was turned down for exactly this reason: a pure function lets tests assert the answer does not follow a backward step in the wall clock.

**B.** The doc comment directly above `reading` states the reason: it's kept "Pure: no clock is read here, which is what lets a test drive a wall clock backwards and check that the answer does not follow it."

`Session::reading` takes the wall-clock reading and monotonic-elapsed time as arguments rather than reading them itself so that the function stays pure. That's what makes it possible to test the core guarantee of the type — that a reading never moves backwards even if the system wall clock jumps backwards — by feeding it a fabricated, backward-stepping wall time and checking the output still only moves forward. If `reading` read the clocks internally, a test could not control or fake a backward step in the wall clock, so this key monotonicity property (a system-clock step back must lose to the monotonic branch, while a forward step wins) would be untestable.

- correct: 
- more useful: 

## 8. (ucal) Why does the session clock accept a large forward jump but never go backwards?

**A.** The reading rule takes the max of the raw wall clock and the anchor plus monotonic elapsed time. A backward step loses to the monotonic branch, so the clock keeps advancing at the oscillator's rate instead of jumping back. A forward jump wins because it is treated as a correction arriving from the system, and refusing it would mean preferring this process's own projection over the system's corrected time. If backward jumps were allowed through, the clock would stop being monotonic.

**B.** The session clock's reading is `max(wall_now, anchor + monotonic_elapsed)`. The system clock isn't trustworthy on its own: it can be disciplined backwards by NTP corrections, a resumed VM, or an operator resetting the date, and a long-running process (unlike a one-shot command) would actually observe that regression. A backward step loses to the monotonic branch, which keeps advancing at the oscillator's rate instead of freezing, so the reading never retreats — a live face never appears to go back in time. A forward step wins instead, because a forward jump is treated as a correction arriving, and rejecting it would mean preferring the process's own opinion of the time over the system's. If backward jumps were honored, monotonicity — the one property this clock exists to provide — would be broken.

- correct: 
- more useful: 

## 9. (ucal) Why are release notes not found by sorting file names by version?

**A.** Sorting filenames lexicographically picked the newest by string order rather than numeric version order, so 1.10.0 sorted before other versions and its release notes were missed. Deriving the version from `[workspace.package]` instead removes the ordering guess entirely. Without this fix, a version like 1.10.0 could be silently skipped in favor of an earlier-numbered but lexicographically "larger" file.

**B.** Sorting release-notes filenames as strings breaks on two-digit minor versions: a plain lexicographic sort puts `1.10.0.md` between `1.1.0.md` and `1.2.0.md`, so "the newest file" resolved to `1.9.0.md` instead of `1.10.0.md`. That made a CI-procedure check silently validate against the *previous* release's notes while reporting success. Even sorting numerically wasn't adopted as the fix, because sorting by version can't detect the adjacent failure where a version is bumped but its notes file is never written — the sort would just quietly fall back to the prior cycle's file and pass. Instead, the code now derives the version directly from the workspace package and reads that specific file, failing loudly if it's missing, removing any ordering guess entirely.

- correct: 
- more useful: 

## 10. (ucal) Why is the observations example a committed file?

**A.** Two test binaries were writing the same temporary fixture path concurrently, causing a race. Replacing that with a committed observations file removes the race, since a fixed file can't be written by two processes at once and is simply opened for reading. Without it, the test would remain flaky.

**B.** The observations file was previously generated at runtime by test binaries writing a residuals fixture into the shared temp directory under a fixed name. Two tests did this concurrently, so one truncated the file while the other read it, producing a flaky failure in CI (though it passed locally) because reading zero observations is treated as an error rather than a vacuous match. The fix was to commit the observations as a static file instead: four synthetic instants representing predicted cycle centers offset by known amounts, so the expected residuals are known in advance and can be checked by eye, and there is nothing left for concurrent processes to race over. It also lets the fixture double as a documented, demonstrable example generated into the manual.

- correct: 
- more useful: 
