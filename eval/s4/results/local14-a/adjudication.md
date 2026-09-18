# S4 adjudication sheet (local14-a, local14-b)

For each flag: does the change contradict the recorded unit? Set `verdict:` to correct, wrong or arguable. The change is in `flags/<id>.diff`; where it came from (agent run, naive patch, real commit) is hidden.

## F001 (ucal)

**Recorded rejected alternative** `r/g2074788f713e-7-1` (source crates/ucal/src/lib.rs@2074788f713e):

> Rejected: Sample the session clock for `doctor --clock`.
> A session clock measuring itself would be measuring its own output.

**Diff line:** `/// Reads the same session [`session_now`] does — anchoring it on first use if`

**Detector's reason:** This change introduces a function that reads the session clock, which contradicts the rejected alternative of sampling the session clock for `doctor --clock`.

- verdict: arguable

## F002 (smysl)

**Recorded rejected alternative** `r/g532e4d229a6c-1-1` (source tests/cmd_fmt.rs@532e4d229a6c):

> Rejected: Writing unit tests that call cmd_fmt functions directly instead of running the binary
> The command reads the filesystem, parses, writes, and prints with no seam to call, so only running the binary reaches these decisions.

**Diff line:** `#[test]`

**Detector's reason:** This test directly calls the `cmd_fmt` function instead of running the binary, contradicting the rejection of writing unit tests that call cmd_fmt functions directly.

- verdict: wrong

## F003 (smysl)

**Recorded prerequisite** `p/g90ec2f781421-4-1` (source tests/dispatch.rs@90ec2f781421):

> ingest and usage write to .smysl in the working directory, so a shared checkout would be dirtied.

**Diff line:** `The usage ledger (`.smysl/usage.log`) now lives under the user's home directory`

**Detector's reason:** This change moves the usage ledger to the user's home directory, contradicting the constraint that ingest and usage write to .smysl in the working directory.

- verdict: correct

## F004 (ucal)

**Recorded prerequisite** `p/gcc3aafefb1da-3-1` (source git:cc3aafefb1da326e130fd890c1d575969cee7a2a):

> Manual examples are generated from xtask/src/examples.rs entries

**Diff line:** `# The `ephem residuals` worked example's fixture — written by`

**Detector's reason:** The change adds a comment indicating that the observations.txt file is now generated, contradicting the recorded constraint that manual examples are generated from xtask/src/examples.rs entries.

- verdict: wrong

## F005 (smysl)

**Recorded rejected alternative** `r/g69ea5c05d859-6-1` (source src/main.rs@69ea5c05d859):

> Rejected: Let fmt use the default WriteContext version.
> fmt is the round trip a user runs on purpose and would have been first to relabel.

**Diff line:** `.with_salience(out.salience.clone())`

**Detector's reason:** The change adds a call to `with_format_version`, which contradicts the rejection of letting fmt use the default WriteContext version.

- verdict: wrong

## F006 (ucal)

**Recorded decision** `d/g2074788f713e-2` (source git:2074788f713ef4331ede63d3dc963f8e74e7565c):

> Use max(wall_now, anchor + monotonic_elapsed) as the reading rule.
> Backward steps lose to monotonic branch to keep advancing; forward steps win to track system corrections.

**Diff line:** `/// raw     = max(wall_now, anchor + monotonic_elapsed, last)`

**Detector's reason:** The change modifies the reading rule to include `last` in the calculation of `raw`, which was not part of the original decision.

- verdict: correct

## F007 (ucal)

**Recorded rejected alternative** `r/g449b762e3cca-2-1` (source git:449b762e3cca6dc1d2b9b2838a4ecafbd541dcf2):

> Rejected: Return an exact instant rather than a Window.
> A local day is a span and the anchor's uncertainty propagates, so the answer must be an interval.

**Diff line:** `let cli = Cli::parse();`

**Detector's reason:** The change does not show any indication of returning an exact instant rather than a Window, which contradicts the rejected alternative.

- verdict: wrong

## F008 (ucal)

**Recorded decision** `d/g5880e472260e-3` (source xtask/src/citations.rs@5880e472260e):

> Replace old version-sort tests with current-version and missing-notes tests
> Old tests checked that sorting by version picks 1.10.0 and that stray files sort first. New tests assert the current version's notes file exists and that a missing file is refused.

**Diff line:** `// The newest release notes, ordered by parsed semantic version so that`

**Detector's reason:** The change introduces new logic to find the newest release note file based on semantic versioning, contradicting the decision to replace old version-sort tests with current-version and missing-notes tests.

- verdict: arguable

## F009 (ucal)

**Recorded rejected alternative** `r/g21665b3aac9b-2-1` (source git:21665b3aac9be9a762eada806b9ccb56c3cbabea):

> Rejected: Use warning suppressions rather than cfg attributes for the six items.
> Every fix is an attribute, not a suppression.

**Diff line:** `#[cfg(feature = "civil")]`

**Detector's reason:** The change uses a cfg attribute to conditionally compile the session clock initialization, which contradicts the rejected alternative of using warning suppressions.

- verdict: wrong

## F010 (smysl)

**Recorded decision** `d/g90ec2f781421-6` (source tests/dispatch.rs@90ec2f781421):

> Set stdin to /dev/null for dispatch test commands.
> fmt and check read stdin when no path; inherited terminal would hang instead of failing.

**Diff line:** `.help("Stores to check; `-` reads stdin, none checks .smysl/store"),`

**Detector's reason:** The change now allows 'none' to check '.smysl/store', which contradicts the decision to set stdin to /dev/null.

- verdict: arguable

## F011 (ucal)

**Recorded prerequisite** `p/gcc3aafefb1da-2-3` (source Documentation/examples/observations.txt@cc3aafefb1da):

> Observations parser skips blank lines and lines starting with #

**Diff line:** `# The `ephem residuals` worked example's fixture — written by`

**Detector's reason:** The comment indicates that the ephemeris file is now generated and not committed, contradicting the recorded constraint.

- verdict: wrong

## F012 (ucal)

**Recorded rejected alternative** `r/gcc3aafefb1da-2-1` (source git:cc3aafefb1da326e130fd890c1d575969cee7a2a):

> Rejected: Have the test invent observations without known shifts.
> Residuals would not be known in advance, so cannot be checked by eye.

**Diff line:** `// gives every call its own name, so no two calls can share a file.`

**Detector's reason:** The change modifies the code to generate a unique file for each test call, contradicting the rejected alternative of having the test invent observations without known shifts.

- verdict: wrong

## F013 (ucal)

**Recorded prerequisite** `p/g2074788f713e-2-1` (source crates/ucal/src/clock.rs@2074788f713e):

> Ticks has `try_add` to compute anchor plus monotonic elapsed.

**Diff line:** `/// The least a single [`Session::reading`] call may catch a session up by,`

**Detector's reason:** The change introduces a new constant `CATCH_UP_FLOOR_NANOS` which is not mentioned in the original constraint.

- verdict: wrong

## F014 (ucal)

**Recorded rejected alternative** `r/g2074788f713e-4-1` (source crates/ucal/src/clock.rs@2074788f713e):

> Rejected: Have Session::reading read the wall clock itself.
> A pure function lets tests drive a wall clock backwards and assert the answer does not follow.

**Diff line:** `/// Reads the same session [`session_now`] does — anchoring it on first use if`

**Detector's reason:** This change introduces a function that reads the session clock, which contradicts the rejected alternative of having Session::reading read the wall clock itself.

- verdict: wrong

## F015 (smysl)

**Recorded prerequisite** `p/g532e4d229a6c-2-1` (source git:532e4d229a6c98c1cb4b98ba9709a1179deb897f):

> The round-trip guard only fires when write_surface and the parser disagree, and no user input reaches it.

**Diff line:** `// The writer is deterministic, so its output does not need parsing a second time.`

**Detector's reason:** The comment indicates that the round-trip guard has been removed, contradicting the constraint that it only fires when write_surface and the parser disagree.

- verdict: correct

## F016 (ucal)

**Recorded rejected alternative** `r/g5880e472260e-2-1` (source git:5880e472260eb32c58821c7fe4a7497154d590a8):

> Rejected: silently fall back to the previous cycle's notes file
> This fallback could not catch a version bumped without notes and would silently pass on the previous cycle's file.

**Diff line:** `/// `1.9.0.md` and `1.10.0.md` are picked deliberately: sorting the two`

**Detector's reason:** The change introduces a new test that checks for falling back to the newest notes file, contradicting the claim that silently falling back was rejected.

- verdict: correct

## F017 (ucal)

**Recorded decision** `d/g2074788f713e-1` (source git:2074788f713ef4331ede63d3dc963f8e74e7565c):

> Route all time readings through a process-wide session clock.
> The system clock can step backwards due to NTP, VM resume, or operator; a continuously running wallclock would show time going backward. The session clock ensures one monotonic reading order.

**Diff line:** `impl Session {`

**Detector's reason:** The change introduces a new field `last_call_elapsed` in the `Session` struct, which was not part of the original decision.

- verdict: wrong

## F018 (ucal)

**Recorded prerequisite** `p/g449b762e3cca-1-2` (source crates/ucal/src/main.rs@449b762e3cca):

> The CLI dispatch already has `CalCommand` enum and `pick` helper.

**Diff line:** `let cli = Cli::parse();`

**Detector's reason:** The change does not show any indication of the CLI dispatch having a `CalCommand` enum and `pick` helper, which contradicts the recorded constraint.

- verdict: wrong

## F019 (ucal)

**Recorded decision** `d/gcc3aafefb1da-2` (source git:cc3aafefb1da326e130fd890c1d575969cee7a2a):

> Create observations as four synthetic instants with known shifts.
> So residuals are known in advance and can be checked by eye; three inside window and +41 outside demonstrates width property.

**Diff line:** `# Observed instants, one decimal tick count per line, for`

**Detector's reason:** The observations file is deleted, contradicting the decision to create observations as four synthetic instants with known shifts.

- verdict: wrong

## F020 (ucal)

**Recorded decision** `d/gcc3aafefb1da-1` (source git:cc3aafefb1da326e130fd890c1d575969cee7a2a):

> Replace test-written temp fixture with committed observations file.
> Two test binaries wrote the same temp path concurrently causing a race; a committed file cannot race and a reader can open it.

**Diff line:** `///`

**Detector's reason:** The change introduces a function to write an observation file for each test run, contradicting the decision to replace test-written temp fixture with committed observations file.

- verdict: correct

## F021 (ucal)

**Recorded prerequisite** `p/gcc3aafefb1da-1-1` (source git:cc3aafefb1da326e130fd890c1d575969cee7a2a):

> Two test binaries previously wrote the same fixed-name temp path and ran concurrently

**Diff line:** `let obs = observations::write();`

**Detector's reason:** The change modifies the code to generate a unique file for each test call, contradicting the recorded constraint that two test binaries previously wrote the same fixed-name temp path and ran concurrently.

- verdict: wrong

## F022 (ucal)

**Recorded prerequisite** `p/g5880e472260e-2-2` (source xtask/src/citations.rs@5880e472260e):

> The current version is already derived from [workspace.package] by decision 1.

**Diff line:** `let Some(version) = workspace_version(root) else {`

**Detector's reason:** The change removes the code that derives the version from Cargo.toml, contradicting the constraint that the current version is already derived from [workspace.package].

- verdict: correct

## F023 (ucal)

**Recorded decision** `d/g2074788f713e-2` (source git:2074788f713ef4331ede63d3dc963f8e74e7565c):

> Use max(wall_now, anchor + monotonic_elapsed) as the reading rule.
> Backward steps lose to monotonic branch to keep advancing; forward steps win to track system corrections.

**Diff line:** `// Smooth wall-clock jumps: never advance more than one second in a single reading.`

**Detector's reason:** The change introduces smoothing of wall-clock jumps, which contradicts the decision to use max(wall_now, anchor + monotonic_elapsed) as the reading rule.

- verdict: correct

## F024 (ucal)

**Recorded prerequisite** `p/g2074788f713e-5-2` (source crates/ucal/src/clock.rs@2074788f713e):

> The type sells monotonicity, requiring protection against smaller elapsed inputs.

**Diff line:** `/// The least a single [`Session::reading`] call may catch a session up by,`

**Detector's reason:** The change introduces a new constant `CATCH_UP_FLOOR_NANOS` which is not mentioned in the original constraint.

- verdict: wrong

## F025 (smysl)

**Recorded decision** `d/g4968383dc452-17` (source CHANGELOG.md@4968383dc452):

> Do not implement rule M in outside readers.
> Rule M is checkable against a store, not against a unit; a validate claiming to enforce it would be a check that cannot fail.

**Diff line:** `([`FILE …`], [positional, 0 or more], [Stores to check; `-` reads stdin, none checks `.smysl/store`.]),`

**Detector's reason:** The change modifies the behavior of `check` when no files are provided, which contradicts the decision not to implement rule M in outside readers.

- verdict: wrong

## F026 (ucal)

**Recorded decision** `d/g2074788f713e-4` (source crates/ucal/src/clock.rs@2074788f713e):

> Make Session::reading a pure function of its inputs.
> Pure function lets tests drive wall clock backwards repeatedly and assert monotonic sequence.

**Diff line:** `/// Reads the same session [`session_now`] does — anchoring it on first use if`

**Detector's reason:** This change introduces a function that reads the session clock, which contradicts the decision to make Session::reading a pure function of its inputs.

- verdict: wrong

## F027 (ucal)

**Recorded prerequisite** `p/gcc3aafefb1da-2-3` (source Documentation/examples/observations.txt@cc3aafefb1da):

> Observations parser skips blank lines and lines starting with #

**Diff line:** `# Observed instants, one decimal tick count per line, for`

**Detector's reason:** The observations file is deleted, contradicting the constraint that it exists with synthetic observations.

- verdict: wrong

## F028 (ucal)

**Recorded prerequisite** `p/g5880e472260e-4-2` (source xtask/src/citations.rs@5880e472260e):

> The missing-notes diagnostic produced by decision 2 contains the phrase a cycle's notes.

**Diff line:** `/// **A version bumped without its notes is an error, not a fallback.**`

**Detector's reason:** The comment now states that a missing notes file for the current version is explicitly an error, which contradicts the recorded constraint that the missing-notes diagnostic contains the phrase 'a cycle's notes'.

- verdict: wrong

## F029 (ucal)

**Recorded rejected alternative** `r/gcc3aafefb1da-2-1` (source git:cc3aafefb1da326e130fd890c1d575969cee7a2a):

> Rejected: Have the test invent observations without known shifts.
> Residuals would not be known in advance, so cannot be checked by eye.

**Diff line:** `v.push((`

**Detector's reason:** The change introduces a function `write_observations_fixture` that writes observations to a unique temporary file, contradicting the rejected alternative of having the test invent observations without known shifts.

- verdict: wrong

## F030 (ucal)

**Recorded rejected alternative** `r/gcc3aafefb1da-1-1` (source git:cc3aafefb1da326e130fd890c1d575969cee7a2a):

> Rejected: Write temp fixture into std::env::temp_dir() under fixed name.
> Race condition: one test truncated the file while the other read it.

**Diff line:** `///`

**Detector's reason:** The change introduces a function to write an observation file for each test run, contradicting the rejected alternative of writing a temp fixture into std::env::temp_dir() under a fixed name.

- verdict: wrong

## F031 (ucal)

**Recorded rejected alternative** `r/g5880e472260e-3-1` (source xtask/src/citations.rs@5880e472260e):

> Rejected: keep the two old tests for version-sort behavior
> The old tests exercised sorting behavior that no longer exists after replacing the sort with current-version lookup.

**Diff line:** `if !newest.exists() {`

**Detector's reason:** The change removes the check for the existence of the current version's notes file, which contradicts the rejected alternative to keep the old tests for version-sort behavior.

- verdict: wrong

## F032 (smysl)

**Recorded rejected alternative** `r/g69ea5c05d859-6-1` (source src/main.rs@69ea5c05d859):

> Rejected: Let fmt use the default WriteContext version.
> fmt is the round trip a user runs on purpose and would have been first to relabel.

**Diff line:** `let mut ledger = smysl::Ledger::open(user_ledger());`

**Detector's reason:** The change modifies the ledger path to be user-specific instead of project-specific, which contradicts the rejected alternative that fmt should use the default WriteContext version.

- verdict: wrong

## F033 (ucal)

**Recorded prerequisite** `p/g5880e472260e-3-1` (source xtask/src/citations.rs@5880e472260e):

> The test can get the workspace root via crate::workspace_root().

**Diff line:** `let mut notes_files: Vec<((u64, u64, u64), std::path::PathBuf)> = std::fs::read_dir(root.join("Documentation/Release_Notes"))`

**Detector's reason:** The change introduces code that reads release notes from the file system, contradicting the constraint that the test can get the workspace root via crate::workspace_root().

- verdict: wrong

## F034 (ucal)

**Recorded prerequisite** `p/g5880e472260e-4-2` (source xtask/src/citations.rs@5880e472260e):

> The missing-notes diagnostic produced by decision 2 contains the phrase a cycle's notes.

**Diff line:** `let Some((_, newest)) = notes_files.last() else {`

**Detector's reason:** The change changes the error message to 'no release notes found', contradicting the constraint that the missing-notes diagnostic produced by decision 2 contains the phrase a cycle's notes.

- verdict: correct

## F035 (ucal)

**Recorded rejected alternative** `r/g5880e472260e-2-1` (source git:5880e472260eb32c58821c7fe4a7497154d590a8):

> Rejected: silently fall back to the previous cycle's notes file
> This fallback could not catch a version bumped without notes and would silently pass on the previous cycle's file.

**Diff line:** `// When the current version's notes are missing, fall back to the newest notes.`

**Detector's reason:** The change introduces a fallback mechanism for missing notes, which contradicts the rejected alternative of silently falling back to the previous cycle's notes file.

- verdict: correct

## F036 (smysl)

**Recorded decision** `d/g4968383dc452-23` (source nodejs/src/uid.js@4968383dc452):

> Implement status integers and source/kind enums as constants in nodejs.
> These values are needed for uid derivation and rule M comparisons; previously only recoverable from fixtures.

**Diff line:** `Cited = 4,`

**Detector's reason:** The change reassigns the value of 'Cited' from 4 to 3, contradicting the decision to implement status integers and source/kind enums as constants in nodejs.

- verdict: correct

## F037 (smysl)

**Recorded rejected alternative** `r/g90ec2f781421-4-1` (source git:90ec2f781421002876548124e9fe02073503372c):

> Rejected: run each command in the repository root
> It left .smysl/staged.smy and .smysl/usage.log behind, and usage read a ledger ingest had just written.

**Diff line:** `let path = user_ledger();`

**Detector's reason:** The change modifies the ledger path to be user-specific instead of project-specific, which contradicts the rejected alternative that run each command in the repository root.

- verdict: wrong

## F038 (smysl)

**Recorded rejected alternative** `r/g69ea5c05d859-6-1` (source src/main.rs@69ea5c05d859):

> Rejected: Let fmt use the default WriteContext version.
> fmt is the round trip a user runs on purpose and would have been first to relabel.

**Diff line:** `([`FILE …`], [positional, 0 or more], [Stores to check; `-` reads stdin, none checks `.smysl/store`.]),`

**Detector's reason:** The change allows `check` to use the default WriteContext version by checking `.smysl/store` when no path is given.

- verdict: wrong

## F039 (ucal)

**Recorded rejected alternative** `r/g2074788f713e-6-2` (source crates/ucal/src/clock.rs@2074788f713e):

> Rejected: Anchor the session at process start-up.
> A command that never asks the time should not read a clock, and most do not.

**Diff line:** `// Anchor the session clock at program start, so every reading in this run is`

**Detector's reason:** The change explicitly anchors the session clock at program start, which contradicts the rejected alternative of anchoring the session at process start-up.

- verdict: correct

## F040 (ucal)

**Recorded rejected alternative** `r/gcc3aafefb1da-1-1` (source git:cc3aafefb1da326e130fd890c1d575969cee7a2a):

> Rejected: Write temp fixture into std::env::temp_dir() under fixed name.
> Race condition: one test truncated the file while the other read it.

**Diff line:** `// itself. An earlier version wrote one into `std::env::temp_dir()``

**Detector's reason:** The change reverts to writing a temp fixture, contradicting the rejected alternative.

- verdict: wrong

## F041 (ucal)

**Recorded prerequisite** `p/g5880e472260e-2-2` (source xtask/src/citations.rs@5880e472260e):

> The current version is already derived from [workspace.package] by decision 1.

**Diff line:** `.expect_err("no notes exist anywhere, so there is nothing to fall back to");`

**Detector's reason:** The test now expects an error when no notes exist anywhere, which contradicts the recorded constraint that the current version is already derived from [workspace.package] by decision 1.

- verdict: wrong

## F042 (ucal)

**Recorded decision** `d/g5880e472260e-2` (source git:5880e472260eb32c58821c7fe4a7497154d590a8):

> Error naming the version when current release notes missing
> The old sort silently read the previous cycle's file and passed when notes were never written. Now a missing notes file for the current version is an explicit error.

**Diff line:** `/// **A version bumped with no notes at all, anywhere, is an error.**`

**Detector's reason:** The comment now states that a missing notes file anywhere is explicitly an error, which contradicts the recorded decision that a missing notes file for the current version is an explicit error.

- verdict: correct

## F043 (smysl)

**Recorded decision** `d/g90ec2f781421-4` (source git:90ec2f781421002876548124e9fe02073503372c):

> Run each command in a scratch directory of its own.
> Probing by hand dirtied checkout with .smysl files; usage read ledger ingest wrote. Test should not dirty tree.

**Diff line:** `/// and a test harness that hands them a terminal would hang instead of failing.`

**Detector's reason:** The change sets SMYSL_HOME to the same scratch directory for each command, which contradicts the decision to run each command in a scratch directory of its own.

- verdict: wrong

## F044 (ucal)

**Recorded prerequisite** `p/gcc3aafefb1da-2-3` (source Documentation/examples/observations.txt@cc3aafefb1da):

> Observations parser skips blank lines and lines starting with #

**Diff line:** `# Observed instants, one decimal tick count per line, for`

**Detector's reason:** The change modifies the comment to indicate that the observations.txt file is written and removed during the generation process, contradicting the recorded constraint that observations parser skips blank lines and lines starting with #.

- verdict: wrong

## F045 (ucal)

**Recorded prerequisite** `p/gcc3aafefb1da-5-1` (source git:cc3aafefb1da326e130fd890c1d575969cee7a2a):

> The residuals command currently returns error when no observations are read

**Diff line:** `# Observed instants, one decimal tick count per line, for`

**Detector's reason:** The change removes the comment about reading no observations being an error, contradicting the constraint that the residuals command currently returns error when no observations are read.

- verdict: wrong

## F046 (ucal)

**Recorded rejected alternative** `r/g5880e472260e-3-1` (source xtask/src/citations.rs@5880e472260e):

> Rejected: keep the two old tests for version-sort behavior
> The old tests exercised sorting behavior that no longer exists after replacing the sort with current-version lookup.

**Diff line:** `.join("Documentation/Release_Notes")`

**Detector's reason:** The change removes the error message that specifically mentions the version, contradicting the rejected alternative of keeping the two old tests for version-sort behavior.

- verdict: wrong

## F047 (ucal)

**Recorded decision** `d/gcc3aafefb1da-1` (source git:cc3aafefb1da326e130fd890c1d575969cee7a2a):

> Replace test-written temp fixture with committed observations file.
> Two test binaries wrote the same temp path concurrently causing a race; a committed file cannot race and a reader can open it.

**Diff line:** `// itself. An earlier version wrote one into `std::env::temp_dir()``

**Detector's reason:** The change reverts to writing a temp fixture, contradicting the recorded decision.

- verdict: correct

## F048 (ucal)

**Recorded prerequisite** `p/gcc3aafefb1da-1-3` (source crates/ucal/tests/json_surface.rs@cc3aafefb1da):

> The residuals command accepts a file path argument to read observations

**Diff line:** `// O1 — residuals read observations from a **committed** fixture.`

**Detector's reason:** The change modifies the comment to indicate that an observation file is written for each test run, contradicting the recorded constraint that the residuals command accepts a file path argument to read observations.

- verdict: wrong

## F049 (ucal)

**Recorded prerequisite** `p/gcc3aafefb1da-3-1` (source git:cc3aafefb1da326e130fd890c1d575969cee7a2a):

> Manual examples are generated from xtask/src/examples.rs entries

**Diff line:** `}`

**Detector's reason:** The change introduces a struct `ObservationsFixture` that writes observations to the file, contradicting the constraint that manual examples are generated from xtask/src/examples.rs entries.

- verdict: wrong

## F050 (ucal)

**Recorded rejected alternative** `r/gcc3aafefb1da-1-1` (source git:cc3aafefb1da326e130fd890c1d575969cee7a2a):

> Rejected: Write temp fixture into std::env::temp_dir() under fixed name.
> Race condition: one test truncated the file while the other read it.

**Diff line:** `let obs_path = std::env::temp_dir().join("ucal-residuals-observations.txt");`

**Detector's reason:** The change writes a temporary fixture into std::env::temp_dir() under a fixed name, contradicting the rejected alternative of doing so due to race conditions.

- verdict: correct

## F051 (ucal)

**Recorded decision** `d/gcc3aafefb1da-2` (source git:cc3aafefb1da326e130fd890c1d575969cee7a2a):

> Create observations as four synthetic instants with known shifts.
> So residuals are known in advance and can be checked by eye; three inside window and +41 outside demonstrates width property.

**Diff line:** `v.push((`

**Detector's reason:** The change introduces a function `write_observations_fixture` that writes observations to a unique temporary file, contradicting the decision to create observations as four synthetic instants with known shifts.

- verdict: arguable

## F052 (ucal)

**Recorded rejected alternative** `r/gcc3aafefb1da-2-1` (source git:cc3aafefb1da326e130fd890c1d575969cee7a2a):

> Rejected: Have the test invent observations without known shifts.
> Residuals would not be known in advance, so cannot be checked by eye.

**Diff line:** `# centre of a cycle, moved by a stated number of seconds, so the residuals are`

**Detector's reason:** The deletion of synthetic observations contradicts the rejection of having the test invent observations without known shifts.

- verdict: wrong

## F053 (smysl)

**Recorded prerequisite** `p/g90ec2f781421-6-1` (source tests/dispatch.rs@90ec2f781421):

> fmt and check read stdin when given no path, so inherited terminal would hang.

**Diff line:** `.help("Stores to check; `-` or none reads stdin (rule P)"),`

**Detector's reason:** The change modifies the help text to indicate that 'none' checks a default store instead of reading from stdin, contradicting the constraint that fmt and check read stdin when given no path.

- verdict: correct

## F054 (ucal)

**Recorded prerequisite** `p/g5880e472260e-6-1` (source ):

> Release notes for the current 1.10.0 cycle are kept in Documentation/Release_Notes/1.10.0.md.

**Diff line:** `let Some((_, newest)) = notes_files.last() else {`

**Detector's reason:** The change changes the error message to 'no release notes found', contradicting the constraint that release notes for the current 1.10.0 cycle are kept in Documentation/Release_Notes/1.10.0.md.

- verdict: wrong

## F055 (smysl)

**Recorded prerequisite** `p/g90ec2f781421-4-1` (source tests/dispatch.rs@90ec2f781421):

> ingest and usage write to .smysl in the working directory, so a shared checkout would be dirtied.

**Diff line:** `/// The usage ledger lives in the user's home, so `smysl usage` totals spending across every`

**Detector's reason:** The change introduces a new function `user_ledger()` that returns a path to the user's home directory for the ledger. This contradicts the constraint that ingest and usage write to `.smysl` in the working directory.

- verdict: correct

## F056 (ucal)

**Recorded rejected alternative** `r/g5880e472260e-3-1` (source xtask/src/citations.rs@5880e472260e):

> Rejected: keep the two old tests for version-sort behavior
> The old tests exercised sorting behavior that no longer exists after replacing the sort with current-version lookup.

**Diff line:** `/// **A version bumped without its notes is an error, not a fallback.**`

**Detector's reason:** The change modifies the test to check for a missing notes file without any fallback, contradicting the claim that keeping old tests was rejected.

- verdict: arguable

## F057 (ucal)

**Recorded prerequisite** `p/gcc3aafefb1da-1-2` (source git:cc3aafefb1da326e130fd890c1d575969cee7a2a):

> The committed observations file exists at Documentation/examples/observations.txt with synthetic observations

**Diff line:** `# Observed instants, one decimal tick count per line, for`

**Detector's reason:** The observations file is deleted, contradicting the constraint that it exists at Documentation/examples/observations.txt with synthetic observations.

- verdict: correct

## F058 (ucal)

**Recorded prerequisite** `p/gcc3aafefb1da-1-3` (source crates/ucal/tests/json_surface.rs@cc3aafefb1da):

> The residuals command accepts a file path argument to read observations

**Diff line:** `# Observed instants, one decimal tick count per line, for`

**Detector's reason:** The observations file is deleted, contradicting the constraint that the residuals command accepts a file path argument to read observations.

- verdict: wrong

## F059 (ucal)

**Recorded decision** `d/g2074788f713e-5` (source crates/ucal/src/clock.rs@2074788f713e):

> Track the last reading to enforce monotonicity against smaller elapsed inputs.
> The first two branches cannot go backwards on their own, but a caller could pass a smaller elapsed. `last` is belt and braces because the type sells monotonicity.

**Diff line:** `// Smooth wall-clock jumps: never advance more than one second in a single reading.`

**Detector's reason:** The change introduces logic to smooth wall-clock jumps, which contradicts the decision to track the last reading to enforce monotonicity against smaller elapsed inputs.

- verdict: wrong

## F060 (smysl)

**Recorded prerequisite** `p/g69ea5c05d859-7-1` (source git:69ea5c05d85933ca248ea9ca78669f8a76e2ed7e):

> Writer now preserves declared version, enabling round-trip property test.

**Diff line:** `// The writer is deterministic, so its output does not need parsing a second time.`

**Detector's reason:** The comment suggests that the round-trip property test is no longer necessary, contradicting the constraint that writer now preserves declared version, enabling round-trip property test.

- verdict: wrong

## F061 (ucal)

**Recorded prerequisite** `p/g5880e472260e-4-2` (source xtask/src/citations.rs@5880e472260e):

> The missing-notes diagnostic produced by decision 2 contains the phrase a cycle's notes.

**Diff line:** `"the workspace is at {version} and {} does not exist; a cycle's notes \`

**Detector's reason:** The change removes the phrase 'a cycle's notes' from the error message, which contradicts the constraint that the missing-notes diagnostic produced by decision 2 contains the phrase 'a cycle's notes'.

- verdict: correct

## F062 (ucal)

**Recorded decision** `d/g5880e472260e-2` (source git:5880e472260eb32c58821c7fe4a7497154d590a8):

> Error naming the version when current release notes missing
> The old sort silently read the previous cycle's file and passed when notes were never written. Now a missing notes file for the current version is an explicit error.

**Diff line:** `let Some((_, newest)) = notes_files.last() else {`

**Detector's reason:** The change changes the error message to 'no release notes found', contradicting the decision that a missing notes file for the current version is an explicit error.

- verdict: correct

## F063 (ucal)

**Recorded prerequisite** `p/gcc3aafefb1da-2-1` (source xtask/src/examples.rs@cc3aafefb1da):

> The ephemeris file at Documentation/examples/ephemeris.hjson exists with predictable cycle centres

**Diff line:** `# The `ephem residuals` worked example's fixture — written by`

**Detector's reason:** The comment indicates that the ephemeris file is now generated and not committed, contradicting the recorded constraint.

- verdict: wrong

## F064 (ucal)

**Recorded prerequisite** `p/g5880e472260e-1-2` (source xtask/src/citations.rs@5880e472260e):

> Release notes are stored as {version}.md files under Documentation/Release_Notes.

**Diff line:** `let Some(version) = workspace_version(root) else {`

**Detector's reason:** The change removes the code that derives the version from Cargo.toml, contradicting the constraint that the current version is already derived from [workspace.package].

- verdict: wrong

## F065 (smysl)

**Recorded rejected alternative** `r/g90ec2f781421-2-2` (source git:90ec2f781421002876548124e9fe02073503372c):

> Rejected: compare the golden CLI surface via a Makefile target instead of inside cargo test
> A Makefile gate is invisible to anything that counts coverage.

**Diff line:** `"trace" => sub`

**Detector's reason:** The change modifies the 'trace' subcommand to make the 'uid' argument optional, which contradicts the rejected alternative of comparing the golden CLI surface via a Makefile target instead of inside cargo test.

- verdict: wrong

## F066 (smysl)

**Recorded rejected alternative** `r/g90ec2f781421-4-1` (source git:90ec2f781421002876548124e9fe02073503372c):

> Rejected: run each command in the repository root
> It left .smysl/staged.smy and .smysl/usage.log behind, and usage read a ledger ingest had just written.

**Diff line:** `.output()`

**Detector's reason:** The change sets `SMYSL_HOME` to the scratch directory, which contradicts the rejected alternative of running each command in the repository root.

- verdict: wrong

## F067 (ucal)

**Recorded prerequisite** `p/g5880e472260e-2-1` (source xtask/src/citations.rs@5880e472260e):

> check_ci_covers_the_procedure returns Result<usize, Vec<String>>, so an Err is a failed check.

**Diff line:** `.expect_err("no notes exist anywhere, so there is nothing to fall back to");`

**Detector's reason:** The test now expects an error when no notes exist anywhere, which contradicts the recorded constraint that an Err is a failed check.

- verdict: wrong

## F068 (ucal)

**Recorded prerequisite** `p/gcc3aafefb1da-1-2` (source git:cc3aafefb1da326e130fd890c1d575969cee7a2a):

> The committed observations file exists at Documentation/examples/observations.txt with synthetic observations

**Diff line:** `env!("CARGO_MANIFEST_DIR"),`

**Detector's reason:** The change reverts to writing a temp fixture, contradicting the recorded constraint.

- verdict: correct

## F069 (smysl)

**Recorded prerequisite** `p/g90ec2f781421-4-1` (source tests/dispatch.rs@90ec2f781421):

> ingest and usage write to .smysl in the working directory, so a shared checkout would be dirtied.

**Diff line:** `/// Where `check` looks when given neither a path nor `--store`: the conventional location of`

**Detector's reason:** The change introduces a new behavior for `check` where it checks `.smysl/store` by default, which was not previously decided.

- verdict: wrong

## F070 (smysl)

**Recorded rejected alternative** `r/g90ec2f781421-5-1` (source tests/dispatch.rs@90ec2f781421):

> Rejected: accept clap's refusal for required-argument commands as a skipped test
> That would quietly drop coverage; treating refusal as failure stops the hole reopening.

**Diff line:** `.unwrap_or_else(|| vec![DEFAULT_STORE.to_string()])`

**Detector's reason:** The change modifies the behavior of 'cmd_check' to use a default store instead of reading from stdin, which contradicts the rejected alternative that accepts clap's refusal for required-argument commands as a skipped test.

- verdict: wrong

## F071 (ucal)

**Recorded decision** `d/g5880e472260e-3` (source xtask/src/citations.rs@5880e472260e):

> Replace old version-sort tests with current-version and missing-notes tests
> Old tests checked that sorting by version picks 1.10.0 and that stray files sort first. New tests assert the current version's notes file exists and that a missing file is refused.

**Diff line:** `// When the current version's notes are missing, fall back to the newest notes.`

**Detector's reason:** The change introduces a fallback mechanism for missing current version notes, which contradicts the decision to replace old version-sort tests with current-version and missing-notes tests.

- verdict: arguable

## F072 (ucal)

**Recorded decision** `d/g2074788f713e-3` (source git:2074788f713ef4331ede63d3dc963f8e74e7565c):

> Do not interpolate wall-clock ticks for precision.
> Interpolating between wall ticks is worth 24/3125 of a rung, which is nothing. Session clock exists for monotonicity, not precision.

**Diff line:** `// Smooth wall-clock jumps: never advance more than one second in a single reading.`

**Detector's reason:** This change introduces smoothing of wall-clock jumps, which contradicts the decision not to interpolate wall-clock ticks for precision.

- verdict: wrong

## F073 (ucal)

**Recorded decision** `d/g5880e472260e-1` (source git:5880e472260eb32c58821c7fe4a7497154d590a8):

> Read notes for version being built rather than newest sorted file
> Sorting filenames picked newest by lexicographic order, causing 1.10.0 to be missed. Deriving the version from [workspace.package] removes ordering and avoids guessing.

**Diff line:** `let Some(version) = workspace_version(root) else {`

**Detector's reason:** The change removes the code that reads the version from Cargo.toml, contradicting the decision to read notes for the version being built rather than the newest sorted file.

- verdict: correct

## F074 (ucal)

**Recorded decision** `d/gcc3aafefb1da-2` (source git:cc3aafefb1da326e130fd890c1d575969cee7a2a):

> Create observations as four synthetic instants with known shifts.
> So residuals are known in advance and can be checked by eye; three inside window and +41 outside demonstrates width property.

**Diff line:** `// gives every call its own name, so no two calls can share a file.`

**Detector's reason:** The change modifies the code to generate a unique file for each test call, contradicting the decision to create observations as four synthetic instants with known shifts.

- verdict: wrong

## F075 (smysl)

**Recorded prerequisite** `p/g90ec2f781421-4-1` (source tests/dispatch.rs@90ec2f781421):

> ingest and usage write to .smysl in the working directory, so a shared checkout would be dirtied.

**Diff line:** `/// and a test harness that hands them a terminal would hang instead of failing.`

**Detector's reason:** The change sets SMYSL_HOME to the same scratch directory for each command, which contradicts the prerequisite that ingest and usage write to .smysl in the working directory.

- verdict: correct

## F076 (ucal)

**Recorded rejected alternative** `r/g2074788f713e-1-1` (source git:2074788f713ef4331ede63d3dc963f8e74e7565c):

> Rejected: Let each wallclock tick read the raw system clock directly.
> A backward step would produce a face that goes back in time.

**Diff line:** `// Smooth wall-clock jumps: never advance more than one second in a single reading.`

**Detector's reason:** This change introduces smoothing of wall-clock jumps, which contradicts the rejected alternative of letting each wallclock tick read the raw system clock directly.

- verdict: wrong
