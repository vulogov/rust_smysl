# S4 adjudication sheet (held-a, held-b)

For each flag: does the change contradict the recorded unit? Set `verdict:` to correct, wrong or arguable. The change is in `flags/<id>.diff`; where it came from (agent run, naive patch, real commit) is hidden.

**Verdicts proposed by Claude (2026-09-19) at the owner's request, not by the owner.** Blind to provenance; calibrated against the owner's development verdicts in `local14-a`. Notes on every correct and arguable flag.

## F001 (smysl)

**Recorded decision** `d/g69ea5c05d859-14` (source git:69ea5c05d85933ca248ea9ca78669f8a76e2ed7e):

> Defer flipping the writer default to smysl/1.0 until 0.14 is published.
> §8.2 requires readers to refuse a version absent from their list. Flipping the writer before 0.14 is in the field would make every other implementation reject the output.

**Diff line:** `### The lifecycle of edges and disagreements — specification draft and library`

**Detector's reason:** The change introduces new features for version 1.4.0, contradicting the decision to defer flipping the writer default to smysl/1.0 until 0.14 is published.

- verdict: wrong

## F002 (smysl)

**Recorded rejected alternative** `r/ga18e230ae345-6-1` (source CHANGELOG.md@a18e230ae345):

> Rejected: Leave the changelog entry as 'Nothing yet.'
> To record the removal and its justification for the 0.8.0 release notes.

**Diff line:** `Nothing yet.`

**Detector's reason:** The change updates the changelog entry to describe a specific change, which contradicts the rejection of leaving the changelog entry as 'Nothing yet.'

- verdict: wrong

## F003 (smysl)

**Recorded decision** `d/g87345653cdf3-10` (source tests/doc_output.rs@87345653cdf3):

> Assert on transcript count rather than just MISMATCHED presence in doc output test
> The first version asserted only that MISMATCHED appeared, which ran 0 transcripts satisfied; the count assertion ensures a gate that cannot confuse nothing wrong with nothing checked.

**Diff line:** `assert_eq!(out.code, CHECK_ERRORS, "{}", out.stderr);`

**Detector's reason:** This line asserts only that the exit code is `CHECK_ERRORS`, which does not ensure a gate that cannot confuse nothing wrong with nothing checked, contradicting the decision.

- verdict: wrong

## F004 (smysl)

**Recorded rejected alternative** `r/ga18e230ae345-6-1` (source CHANGELOG.md@a18e230ae345):

> Rejected: Leave the changelog entry as 'Nothing yet.'
> To record the removal and its justification for the 0.8.0 release notes.

**Diff line:** `## Unreleased — 0.9.0`

**Detector's reason:** The changelog entry is no longer 'Nothing yet.' as it now contains release notes for version 0.9.0.

- verdict: wrong

## F005 (smysl)

**Recorded prerequisite** `p/g0bc1aeb96d6b-1-2` (source git:0bc1aeb96d6b6bd4140a24886064f1d60a42829e):

> CI sets `-D warnings`, which refuses an unused function

**Diff line:** `#   And in 0.13, S2 and S4 of the road to 1.0: `runtime`, `stream` and the five concrete`

**Detector's reason:** The change updates the count from 988 to 716, contradicting the constraint that CI sets `-D warnings`, which refuses an unused function.

- verdict: wrong

## F006 (ucal)

**Recorded rejected alternative** `r/g2074788f713e-4-1` (source crates/ucal/src/clock.rs@2074788f713e):

> Rejected: Have Session::reading read the wall clock itself.
> A pure function lets tests drive a wall clock backwards and assert the answer does not follow.

**Diff line:** `**The session clock's drift is reported too**, as `clock_measured.session_drift``

**Detector's reason:** This change introduces reporting of the session clock's drift, which contradicts the decision to reject having Session::reading read the wall clock itself.

- verdict: wrong

## F007 (ucal)

**Recorded decision** `d/g21665b3aac9b-6` (source git:21665b3aac9be9a762eada806b9ccb56c3cbabea):

> Gate parse_form and codec::Form behind all(civil, std)
> They belong to run_now, which needs both civil and std features.

**Diff line:** `fn parse_version(v: &str) -> Option<(u64, u64, u64)> {`

**Detector's reason:** The change removes the `parse_version` function that returns a tuple of three integers representing the major, minor, and patch version numbers, contradicting the decision to gate `parse_form` and `codec::Form` behind all(civil, std) features.

- verdict: wrong

## F008 (smysl)

**Recorded decision** `d/ga18e230ae345-1` (source git:a18e230ae34564f0a3d93538cf1a557b68a3f5aa):

> Remove the local-improvement pass (§18.3 step 3).
> It was making packs worse; across 28,000 generated packs it changed 26 and 22 were worse by the value function it exists to maximise.

**Diff line:** `| `provider::stream` | `ollama_live.rs` drives a real streaming response. `StreamMsg` stays contract via the root `pub use`; only the module path went. |`

**Detector's reason:** The change hides the entire `stream` module, contradicting the decision to remove the local-improvement pass.

- verdict: wrong

## F009 (smysl)

**Recorded prerequisite** `p/ga18e230ae345-6-1` (source CHANGELOG.md@a18e230ae345):

> The changelog has an Unreleased — 0.8.0 section where the removal can be recorded.

**Diff line:** `from the registry gives `smysl 0.11.0`. `BASELINE` moved from 0.9.0 to 0.11.0 and`

**Detector's reason:** The change updates the BASELINE to 0.11.0, which contradicts the constraint that the changelog has an Unreleased — 0.8.0 section where the removal can be recorded.

- verdict: wrong

## F010 (ucal)

**Recorded decision** `d/g2074788f713e-1` (source git:2074788f713ef4331ede63d3dc963f8e74e7565c):

> Route all time readings through a process-wide session clock.
> The system clock can step backwards due to NTP, VM resume, or operator; a continuously running wallclock would show time going backward. The session clock ensures one monotonic reading order.

**Diff line:** `/// So a reading never goes backwards, never advances faster than the cap, and`

**Detector's reason:** The change introduces a new behavior for forward steps that contradicts the decision.

- verdict: wrong

## F011 (ucal)

**Recorded rejected alternative** `r/g21665b3aac9b-6-1` (source git:21665b3aac9be9a762eada806b9ccb56c3cbabea):

> Rejected: Gate parse_form and codec::Form behind a single feature or leave them ungated.
> They belong to run_now, which needs both civil and std, so all(civil, std) is required.

**Diff line:** `fn parse_version(v: &str) -> Option<(u64, u64, u64)> {`

**Detector's reason:** The change removes the `parse_version` function that returns a tuple of three integers representing the major, minor, and patch version numbers, contradicting the rejection of gating `parse_form` and `codec::Form` behind a single feature or leaving them ungated.

- verdict: wrong

## F012 (smysl)

**Recorded prerequisite** `p/g4968383dc452-6-1` (source git:4968383dc452dba3c9ac9ab2c6abae4b3612bba5):

> Missing facts were recoverable only by decoding core_bytes_hex in uid fixtures.

**Diff line:** `(`SMY-W056`, registry 53). Not a `retracts` edge pointing at a rid, because a 1.3 checker would`

**Detector's reason:** The change introduces a new diagnostic code, contradicting the prerequisite that missing facts were recoverable only by decoding core_bytes_hex in uid fixtures.

- verdict: wrong

## F013 (smysl)

**Recorded decision** `d/g4968383dc452-16` (source git:4968383dc452dba3c9ac9ab2c6abae4b3612bba5):

> Hold beta.smy repair rather than guess missing stanza.
> The missing stanza cannot be reconstructed from the page, and guessing one into a published chapter is not a repair.

**Diff line:** ``beta.smy` in chapter 29 unlocks seven commands, `draft.smy` in chapter 4 unlocks one. Both`

**Detector's reason:** The change indicates that missing stanzas were held back rather than guessed, contradicting the decision.

- verdict: wrong

## F014 (ucal)

**Recorded decision** `d/g5880e472260e-4` (source git:5880e472260eb32c58821c7fe4a7497154d590a8):

> Make missing-notes test strict about the diagnostic
> Asserting only that some error came back and the message named the version passed with the check disabled because read_to_string also fails and names the path. The test now checks for the specific missing-notes diagnostic.

**Diff line:** `/// guess closes.`

**Detector's reason:** The change now allows a missing notes file for the current version to fall back to the newest notes, contradicting the decision that such a case should be an error.

- verdict: wrong

## F015 (ucal)

**Recorded decision** `d/g31e52cc10b1f-4` (source git:31e52cc10b1f6079a130e3bc27479739d7e393eb):

> Do not measure accuracy.
> Operation is offline per §8.4, so no reference exists; a rate error from a short baseline reports quantisation as drift.

**Diff line:** `it. §8.4 makes operation offline, so there is no reference — and a rate error`

**Detector's reason:** The change introduces a new feature `session_drift` that measures the difference between two clocks, which contradicts the decision to do not measure accuracy.

- verdict: arguable
  - note: reports a session drift over a one-command baseline, but says outright that it is not accuracy and not a rate

## F016 (smysl)

**Recorded decision** `d/g69ea5c05d859-12` (source git:69ea5c05d85933ca248ea9ca78669f8a76e2ed7e):

> Update format spec §8.5 to record the version-trap defect as fixed in 0.14.
> The trap was invisible until it bit; the spec should now state the fix so the historical warning is replaced with the actual current behavior.

**Diff line:** `Nothing shipped yet. What this cycle starts from, carried from 1.3.0 (details there):`

**Detector's reason:** The change indicates that nothing has been shipped yet for version 1.4.0, contradicting the decision that the version-trap defect is fixed in 0.14.

- verdict: wrong

## F017 (smysl)

**Recorded decision** `d/g4968383dc452-17` (source CHANGELOG.md@4968383dc452):

> Do not implement rule M in outside readers.
> Rule M is checkable against a store, not against a unit; a validate claiming to enforce it would be a check that cannot fail.

**Diff line:** `nothing on the wire.`

**Detector's reason:** The change introduces new features like relation identity, withdrawal, who asserted an edge, live rebuttal, and resolution, contradicting the decision not to implement rule M in outside readers.

- verdict: wrong

## F018 (smysl)

**Recorded decision** `d/ga18e230ae345-4` (source git:a18e230ae34564f0a3d93538cf1a557b68a3f5aa):

> Decline to update the golden file.
> The golden file is byte-identical after the removal; no selections moved, so there is nothing to record.

**Diff line:** `**Result, with S2:** `smysl-provider` 988 → **716**, `smysl-ingest` 751 → **541**. **482 items`

**Detector's reason:** The change updates the golden file count, contradicting the decision to decline updating the golden file.

- verdict: wrong

## F019 (smysl)

**Recorded decision** `d/g4968383dc452-17` (source CHANGELOG.md@4968383dc452):

> Do not implement rule M in outside readers.
> Rule M is checkable against a store, not against a unit; a validate claiming to enforce it would be a check that cannot fail.

**Diff line:** `- **Rule M.** A `derived` unit grounded on `cited` evidence now exceeds its cap (`SMY-E030`),`

**Detector's reason:** The change contradicts the decision not to implement rule M in outside readers by implementing it.

- verdict: wrong

## F020 (smysl)

**Recorded decision** `d/g532e4d229a6c-3` (source tests/cmd_fmt.rs@532e4d229a6c):

> Make each test write its own document with the needed property instead of using repository fixtures.
> The first version depended on repository fixtures like F1 not canonical and F9 having comments. cargo-mutants reuses build directories, so a mutant that misroutes a write can leave a fixture rewritten, causing spurious failures counted as catches. The rule is that a test must not depend on the state of the tree, just as it must not depend on the state of the machine.

**Diff line:** `assert_eq!(canonical.code, SUCCESS, "{}", canonical.stderr);`

**Detector's reason:** This line calls `cmd_fmt` functions directly instead of writing its own document, contradicting the decision.

- verdict: wrong

## F021 (smysl)

**Recorded decision** `d/g69ea5c05d859-6` (source git:69ea5c05d85933ca248ea9ca78669f8a76e2ed7e):

> Pass the parsed format_version from ParseOutcome to WriteContext in `smysl fmt`.
> `fmt` is the round trip a user runs on purpose and would have been the first place the relabelling bit. Using the declared version preserves the document's self-description.

**Diff line:** `configuration allowing more. `IngestOptions::max_output` defaults to `0`, meaning the provider's;`

**Detector's reason:** The change introduces a new internal requirement for workspace version 1.3.0, contradicting the decision to pass the parsed format_version from ParseOutcome to WriteContext in `smysl fmt`.

- verdict: wrong

## F022 (ucal)

**Recorded rejected alternative** `r/g2074788f713e-4-1` (source crates/ucal/src/clock.rs@2074788f713e):

> Rejected: Have Session::reading read the wall clock itself.
> A pure function lets tests drive a wall clock backwards and assert the answer does not follow.

**Diff line:** `// the time should not read a clock, and most of them do not.`

**Detector's reason:** This line contradicts the rejected alternative 'Have Session::reading read the wall clock itself' by removing the comment that suggests anchoring on first use.

- verdict: wrong

## F023 (smysl)

**Recorded prerequisite** `p/g4968383dc452-20-1` (source Documentation/READINESS.md@4968383dc452):

> cargo-mutants auto-set timeout based on baseline, causing timeouts on workspace suite.

**Diff line:** `BASELINE  := 1.3.0`

**Detector's reason:** The change updates BASELINE to 1.3.0, which contradicts the constraint that cargo-mutants auto-set timeout based on baseline, causing timeouts on workspace suite.

- verdict: wrong

## F024 (smysl)

**Recorded decision** `d/g4968383dc452-16` (source git:4968383dc452dba3c9ac9ab2c6abae4b3612bba5):

> Hold beta.smy repair rather than guess missing stanza.
> The missing stanza cannot be reconstructed from the page, and guessing one into a published chapter is not a repair.

**Diff line:** `- run: python3 scripts/verify-spec-tables.py`

**Detector's reason:** The change introduces a new job 'dep-versions' that checks internal crate requirements against the workspace version, contradicting the decision to hold beta.smy repair rather than guess missing stanza.

- verdict: wrong

## F025 (ucal)

**Recorded decision** `d/g2074788f713e-8` (source git:2074788f713ef4331ede63d3dc963f8e74e7565c):

> Gate wall_instant (and session clock) under the civil feature.
> Converting a clock reading is a leap-table lookup, because Unix time is a UTC label. The feature workflow caught the missing gate when building with u512,std.

**Diff line:** `it. §8.4 makes operation offline, so there is no reference — and a rate error`

**Detector's reason:** The change introduces a new feature `session_drift` that measures the difference between two clocks, which contradicts the decision to gate wall_instant under the civil feature.

- verdict: wrong

## F026 (ucal)

**Recorded prerequisite** `p/g2074788f713e-2-2` (source crates/ucal/src/clock.rs@2074788f713e):

> `std::time::Instant::elapsed()` gives monotonic duration from the session base.

**Diff line:** `let nanos = base.elapsed().as_nanos();`

**Detector's reason:** The change introduces a new function `since` that calculates the elapsed time using `base.elapsed().as_nanos()`, which contradicts the recorded constraint that `std::time::Instant::elapsed()` gives monotonic duration from the session base.

- verdict: wrong

## F027 (ucal)

**Recorded rejected alternative** `r/g2074788f713e-5-1` (source crates/ucal/src/clock.rs@2074788f713e):

> Rejected: Trust callers to supply non-decreasing elapsed; omit `last`.
> A caller could pass a smaller elapsed, making the first two branches go backwards.

**Diff line:** `reading = max(wall_now, anchor + monotonic_elapsed)`

**Detector's reason:** The change removes the use of `last` to enforce monotonicity against smaller elapsed inputs.

- verdict: wrong

## F028 (ucal)

**Recorded prerequisite** `p/g5880e472260e-2-1` (source xtask/src/citations.rs@5880e472260e):

> check_ci_covers_the_procedure returns Result<usize, Vec<String>>, so an Err is a failed check.

**Diff line:** `/// **A version bumped without its notes is an error, not a fallback.**`

**Detector's reason:** The comment now states that a missing notes file falls back to the newest notes, contradicting the previous statement that it should be an error.

- verdict: wrong

## F029 (smysl)

**Recorded rejected alternative** `r/g69ea5c05d859-8-1` (source crates/smysl-core/src/lib.rs@69ea5c05d859):

> Rejected: Assert a literal list and keep using smysl/1.0 as an unknown version.
> The old test would fail on new versions or silently pass on unlisted additions.

**Diff line:** `already computed, now stated, with vectors in `fixtures/wire/relation-id/`.`

**Detector's reason:** The change adds a new section stating that the old unit test asserted 'smysl/1.0' as an unknown version, which contradicts the recorded claim that this was rejected.

- verdict: wrong

## F030 (smysl)

**Recorded decision** `d/g69ea5c05d859-14` (source git:69ea5c05d85933ca248ea9ca78669f8a76e2ed7e):

> Defer flipping the writer default to smysl/1.0 until 0.14 is published.
> §8.2 requires readers to refuse a version absent from their list. Flipping the writer before 0.14 is in the field would make every other implementation reject the output.

**Diff line:** `BASELINE  := 0.15.0`

**Detector's reason:** The change updates the BASELINE from 0.15.0 to 1.0.0, contradicting the recorded decision to defer flipping the writer default to smysl/1.0 until 0.14 is published.

- verdict: wrong

## F031 (smysl)

**Recorded decision** `d/g69ea5c05d859-6` (source git:69ea5c05d85933ca248ea9ca78669f8a76e2ed7e):

> Pass the parsed format_version from ParseOutcome to WriteContext in `smysl fmt`.
> `fmt` is the round trip a user runs on purpose and would have been the first place the relabelling bit. Using the declared version preserves the document's self-description.

**Diff line:** `if formatted != src {`

**Detector's reason:** The change introduces a condition to skip the verification reparse when the formatted output is byte-for-byte identical to the source, which contradicts the decision to pass the parsed format_version from ParseOutcome to WriteContext in `smysl fmt` for preserving the document's self-description.

- verdict: wrong

## F032 (smysl)

**Recorded prerequisite** `p/g90ec2f781421-6-1` (source tests/dispatch.rs@90ec2f781421):

> fmt and check read stdin when given no path, so inherited terminal would hang.

**Diff line:** `.num_args(0..)`

**Detector's reason:** The change modifies the help text to indicate that `none` checks `.smysl/store` instead of reading standard input, which contradicts the constraint.

- verdict: correct
  - note: check with no path now reads .smysl/store, so the prerequisite no longer holds for check (as dev local14-a F053)

## F033 (ucal)

**Recorded decision** `d/g2074788f713e-1` (source git:2074788f713ef4331ede63d3dc963f8e74e7565c):

> Route all time readings through a process-wide session clock.
> The system clock can step backwards due to NTP, VM resume, or operator; a continuously running wallclock would show time going backward. The session clock ensures one monotonic reading order.

**Diff line:** `let nanos = base.elapsed().as_nanos();`

**Detector's reason:** The change introduces a new function `since` that calculates the elapsed time using `base.elapsed().as_nanos()`, which contradicts the recorded decision to route all time readings through a process-wide session clock.

- verdict: wrong

## F034 (ucal)

**Recorded prerequisite** `p/g2074788f713e-2-2` (source crates/ucal/src/clock.rs@2074788f713e):

> `std::time::Instant::elapsed()` gives monotonic duration from the session base.

**Diff line:** `///`

**Detector's reason:** This line contradicts the constraint that 'std::time::Instant::elapsed() gives monotonic duration from the session base.' by changing the anchoring mechanism.

- verdict: wrong

## F035 (ucal)

**Recorded prerequisite** `p/g2074788f713e-2-2` (source crates/ucal/src/clock.rs@2074788f713e):

> `std::time::Instant::elapsed()` gives monotonic duration from the session base.

**Diff line:** `let mut g = match session().lock() {`

**Detector's reason:** This line contradicts the constraint 'std::time::Instant::elapsed() gives monotonic duration from the session base' by changing the locking mechanism.

- verdict: wrong

## F036 (smysl)

**Recorded prerequisite** `p/ga18e230ae345-6-1` (source CHANGELOG.md@a18e230ae345):

> The changelog has an Unreleased — 0.8.0 section where the removal can be recorded.

**Diff line:** `## Unreleased — 0.9.0`

**Detector's reason:** The changelog entry is no longer 'Nothing yet.' as it now contains release notes for version 0.9.0.

- verdict: wrong

## F037 (smysl)

**Recorded decision** `d/g4968383dc452-3` (source git:4968383dc452dba3c9ac9ab2c6abae4b3612bba5):

> Implement uid derivation in nodejs reproducing all sixteen fixture uids.
> Reaching C-Produce requires deriving uids; reproducing the fixtures proves the implementation matches the reference bytes.

**Diff line:** `- **Rule T.** The `computed` ceiling (`derived`) now reaches `cited`, and the `document` and`

**Detector's reason:** The change introduces a behavior change in how statuses are compared, contradicting the decision to implement uid derivation in nodejs reproducing all sixteen fixture uids.

- verdict: wrong

## F038 (ucal)

**Recorded decision** `d/g2074788f713e-4` (source crates/ucal/src/clock.rs@2074788f713e):

> Make Session::reading a pure function of its inputs.
> Pure function lets tests drive wall clock backwards repeatedly and assert monotonic sequence.

**Diff line:** `let nanos = base.elapsed().as_nanos();`

**Detector's reason:** The change introduces a new function `since` that calculates the elapsed time using `base.elapsed().as_nanos()`, which contradicts the recorded decision to make `Session::reading` a pure function of its inputs.

- verdict: wrong

## F039 (smysl)

**Recorded decision** `d/g4968383dc452-19` (source CHANGELOG.md@4968383dc452):

> Measure mutation survivors on store/mod.rs under both per-package and workspace tests.
> To compare weak per-package claim with stronger workspace claim and quantify artefact rate.

**Diff line:** `nothing on the wire.`

**Detector's reason:** The change introduces new features like relation identity, withdrawal, who asserted an edge, live rebuttal, and resolution, contradicting the decision to measure mutation survivors on store/mod.rs under both per-package and workspace tests.

- verdict: wrong

## F040 (smysl)

**Recorded decision** `d/g4968383dc452-12` (source CHANGELOG.md@4968383dc452):

> Correct README claims that tables were asserted against document.
> The tables were hand-typed copies, not assertions against the document. Spec-tables gate now parses the document.

**Diff line:** `it contained one string. A gate that had reported on the manual since 0.3 could not see a`

**Detector's reason:** The change states that tests did not actually assert the spec's tables against the document, contradicting the decision.

- verdict: wrong

## F041 (ucal)

**Recorded rejected alternative** `r/g2074788f713e-4-1` (source crates/ucal/src/clock.rs@2074788f713e):

> Rejected: Have Session::reading read the wall clock itself.
> A pure function lets tests drive a wall clock backwards and assert the answer does not follow.

**Diff line:** `reading = max(wall_now, anchor + monotonic_elapsed)`

**Detector's reason:** The change removes the use of `last` to enforce monotonicity against smaller elapsed inputs.

- verdict: wrong

## F042 (ucal)

**Recorded rejected alternative** `r/g2074788f713e-6-2` (source crates/ucal/src/clock.rs@2074788f713e):

> Rejected: Anchor the session at process start-up.
> A command that never asks the time should not read a clock, and most do not.

**Diff line:** `The anchor is taken **when the program starts**, first thing in `main`, and not`

**Detector's reason:** This line contradicts the rejected alternative 'Anchor the session at process start-up.' by stating that the anchor is taken when the program starts.

- verdict: correct
  - note: anchors the session first thing in main: the rejected alternative, done

## F043 (ucal)

**Recorded rejected alternative** `r/g5880e472260e-2-1` (source git:5880e472260eb32c58821c7fe4a7497154d590a8):

> Rejected: silently fall back to the previous cycle's notes file
> This fallback could not catch a version bumped without notes and would silently pass on the previous cycle's file.

**Diff line:** `/// **A version bumped without its notes is an error, not a fallback.**`

**Detector's reason:** The change now allows a missing notes file for the current version to fall back to the newest notes, contradicting the rejection of silently falling back to the previous cycle's notes file.

- verdict: correct
  - note: falls back to the newest notes when the current version's are missing

## F044 (smysl)

**Recorded rejected alternative** `r/g4968383dc452-23-1` (source git:4968383dc452dba3c9ac9ab2c6abae4b3612bba5):

> Rejected: Leave status and source/kind values recoverable only from fixtures.
> These were needed for uid derivation and rule M comparisons.

**Diff line:** ``Status` is renumbered: `cited` is 3 and `derived` is 4 (was 3 and 4 the other way round);`

**Detector's reason:** The change renumbers the `Status` values, contradicting the rejection of leaving status and source/kind values recoverable only from fixtures.

- verdict: wrong

## F045 (smysl)

**Recorded decision** `d/g4968383dc452-8` (source CHANGELOG.md@4968383dc452):

> Mark spec-derived constants with SPEC: in nodejs src/uid.js.
> To record that these values were previously guessed from fixtures, making the method explicit.

**Diff line:** ``Status` is renumbered: `cited` is 3 and `derived` is 4 (was 3 and 4 the other way round);`

**Detector's reason:** The change renumbers the `Status` values, contradicting the decision to mark spec-derived constants with SPEC in nodejs src/uid.js.

- verdict: wrong

## F046 (smysl)

**Recorded decision** `d/g4968383dc452-15` (source Documentation/READINESS.md@4968383dc452):

> Revise gate 7 blocked command explanation, no longer 'decision about book'.
> The block was due to filename-keyed fixtures vs position-keyed chapters, not a book decision.

**Diff line:** `Nothing shipped yet. What this cycle starts from, carried from 1.3.0 (details there):`

**Detector's reason:** The change indicates that nothing has been shipped yet for version 1.4.0, contradicting the decision to revise gate 7 blocked command explanation.

- verdict: wrong

## F047 (smysl)

**Recorded decision** `d/g4968383dc452-1` (source nodejs/README.md@4968383dc452):

> Upgrade nodejs conformance target from C-Read to C-Produce.
> To provide a fourth independent derivation of uids and a third witness for §2.3 beyond the Rust, strengthening the evidence that the specification is sufficient.

**Diff line:** `Nothing shipped yet. What this cycle starts from, carried from 1.3.0 (details there):`

**Detector's reason:** The change indicates that nothing has been shipped yet for version 1.4.0, contradicting the decision to upgrade nodejs conformance target from C-Read to C-Produce.

- verdict: wrong

## F048 (smysl)

**Recorded decision** `d/g4968383dc452-23` (source nodejs/src/uid.js@4968383dc452):

> Implement status integers and source/kind enums as constants in nodejs.
> These values are needed for uid derivation and rule M comparisons; previously only recoverable from fixtures.

**Diff line:** ``Status` is renumbered: `cited` is 3 and `derived` is 4 (was 3 and 4 the other way round);`

**Detector's reason:** The change renumbers the `Status` values, contradicting the decision to implement status integers and source/kind enums as constants in nodejs.

- verdict: correct
  - note: renumbers the normative Status integers the nodejs constants encode; follows the owner's dev verdict (local14-a F036). Nodejs moves in step here, so a stricter reading is arguable

## F049 (ucal)

**Recorded rejected alternative** `r/g5880e472260e-4-1` (source xtask/src/citations.rs@5880e472260e):

> Rejected: assert only that some error came back and the message named the version
> The loose assertion passed even with the check disabled because read_to_string also fails and names the path containing the version.

**Diff line:** `/// **A version bumped without its notes is an error, not a fallback.**`

**Detector's reason:** The comment now states that a missing notes file for the current version falls back to the newest notes, contradicting the previous rejection.

- verdict: wrong

## F050 (smysl)

**Recorded prerequisite** `p/g69ea5c05d859-8-1` (source crates/smysl-core/src/lib.rs@69ea5c05d859):

> Old unit test asserted smysl/1.0 as unknown version.

**Diff line:** `already computed, now stated, with vectors in `fixtures/wire/relation-id/`.`

**Detector's reason:** The change adds a new section stating that the old unit test asserted 'smysl/1.0' as an unknown version, which contradicts the recorded claim that this was rejected.

- verdict: wrong

## F051 (ucal)

**Recorded rejected alternative** `r/g5880e472260e-4-1` (source xtask/src/citations.rs@5880e472260e):

> Rejected: assert only that some error came back and the message named the version
> The loose assertion passed even with the check disabled because read_to_string also fails and names the path containing the version.

**Diff line:** `use std::cmp::Ordering;`

**Detector's reason:** The change introduces a new function `sort_newest_first` that sorts rows by parsed version in descending order, contradicting the rejection of asserting only that some error came back and the message named the version.

- verdict: wrong

## F052 (ucal)

**Recorded prerequisite** `p/g31e52cc10b1f-4-2` (source git:31e52cc10b1f6079a130e3bc27479739d7e393eb):

> A rate error from a short baseline reports quantisation as drift.

**Diff line:** `it. §8.4 makes operation offline, so there is no reference — and a rate error`

**Detector's reason:** The change introduces a new feature `session_drift` that measures the difference between two clocks, which contradicts the constraint that a rate error from a short baseline reports quantization as drift.

- verdict: wrong

## F053 (ucal)

**Recorded rejected alternative** `r/g5880e472260e-1-1` (source git:5880e472260eb32c58821c7fe4a7497154d590a8):

> Rejected: sort filenames numerically / by version to pick the newest
> Sorting numerically would fix only the one exposed bug and would leave the guess in place.

**Diff line:** `use std::cmp::Ordering;`

**Detector's reason:** The change introduces a new function `sort_newest_first` that sorts rows by parsed version in descending order, contradicting the rejection of sorting filenames numerically or by version to pick the newest.

- verdict: wrong

## F054 (smysl)

**Recorded rejected alternative** `r/g90ec2f781421-9-1` (source tests/dispatch.rs@90ec2f781421):

> Rejected: assert exit code in every_command_dispatches
> Most commands fail without a store; how they fail is the rest of the suite's job.

**Diff line:** `.unwrap_or_else(|| vec!["-".to_string()])`

**Detector's reason:** The change modifies the behavior of `smysl check` to use a default store instead of reading from stdin when no path is provided, contradicting the rejected alternative of asserting exit code in every_command_dispatches.

- verdict: wrong

## F055 (smysl)

**Recorded decision** `d/g4968383dc452-16` (source git:4968383dc452dba3c9ac9ab2c6abae4b3612bba5):

> Hold beta.smy repair rather than guess missing stanza.
> The missing stanza cannot be reconstructed from the page, and guessing one into a published chapter is not a repair.

**Diff line:** `Nothing shipped yet. What this cycle starts from, carried from 1.3.0 (details there):`

**Detector's reason:** The change indicates that nothing has been shipped yet for version 1.4.0, contradicting the decision to hold beta.smy repair rather than guess missing stanza.

- verdict: wrong

## F056 (smysl)

**Recorded decision** `d/ga18e230ae345-6` (source CHANGELOG.md@a18e230ae345):

> Document the removal in the changelog.
> To record the removal and its justification for the 0.8.0 release notes.

**Diff line:** `- **Publishing 0.10.0 and 0.11.0.** Both are tagged and merged and neither is on crates.io, so`

**Detector's reason:** The changelog entry now states that 0.11.0 is published, contradicting the decision to document the removal in the changelog.

- verdict: wrong

## F057 (smysl)

**Recorded decision** `d/g4968383dc452-17` (source CHANGELOG.md@4968383dc452):

> Do not implement rule M in outside readers.
> Rule M is checkable against a store, not against a unit; a validate claiming to enforce it would be a check that cannot fail.

**Diff line:** `- run: python3 scripts/verify-spec-tables.py`

**Detector's reason:** The change introduces a new job 'dep-versions' that checks internal crate requirements against the workspace version, contradicting the decision to do not implement rule M in outside readers.

- verdict: wrong

## F058 (ucal)

**Recorded rejected alternative** `r/g2074788f713e-6-2` (source crates/ucal/src/clock.rs@2074788f713e):

> Rejected: Anchor the session at process start-up.
> A command that never asks the time should not read a clock, and most do not.

**Diff line:** `The clock is anchored at program start, before arguments are parsed, so the`

**Detector's reason:** This line contradicts the rejected alternative 'Anchor the session at process start-up' by explicitly stating that the clock is anchored at program start.

- verdict: correct
  - note: anchors the session at program start, before argument parsing

## F059 (smysl)

**Recorded rejected alternative** `r/g4968383dc452-16-1` (source CHANGELOG.md@4968383dc452):

> Rejected: Guess the missing stanza into the published chapter.
> The missing stanza cannot be reconstructed from the page, and guessing is not repair.

**Diff line:** ``beta.smy` in chapter 29 unlocks seven commands, `draft.smy` in chapter 4 unlocks one. Both`

**Detector's reason:** The change indicates that missing stanzas were held back rather than guessed, contradicting the rejected alternative.

- verdict: wrong

## F060 (smysl)

**Recorded prerequisite** `p/g0bc1aeb96d6b-2-1` (source git:0bc1aeb96d6b6bd4140a24886064f1d60a42829e):

> `cargo check -p smysl-provider --no-default-features` fails on `jitter` being unused the same way on the …
> `cargo check -p smysl-provider --no-default-features` fails on `jitter` being unused the same way on the previous commit

**Diff line:** `# Published, not tagged. `cargo-semver-checks` fetches the baseline from crates.io, so this`

**Detector's reason:** The BASELINE is now set to 0.11.0, contradicting the constraint that `cargo check -p smysl-provider --no-default-features` fails on `jitter` being unused.

- verdict: wrong

## F061 (smysl)

**Recorded decision** `d/ga18e230ae345-5` (source crates/smysl-pack/src/solve.rs@a18e230ae345):

> Add a comment explaining the removal in place of the loop.
> To document why the pass was removed, including the measurements that justified it.

**Diff line:** `**S6 — land it as a break.**`

**Detector's reason:** The change adds a comment explaining the removal, contradicting the decision to add a comment explaining the removal.

- verdict: wrong

## F062 (ucal)

**Recorded decision** `d/g5880e472260e-1` (source git:5880e472260eb32c58821c7fe4a7497154d590a8):

> Read notes for version being built rather than newest sorted file
> Sorting filenames picked newest by lexicographic order, causing 1.10.0 to be missed. Deriving the version from [workspace.package] removes ordering and avoids guessing.

**Diff line:** `use std::cmp::Ordering;`

**Detector's reason:** The change introduces a new function `sort_newest_first` that sorts rows by parsed version in descending order, contradicting the decision to read notes for the version being built rather than the newest sorted file.

- verdict: wrong

## F063 (ucal)

**Recorded decision** `d/g2074788f713e-2` (source git:2074788f713ef4331ede63d3dc963f8e74e7565c):

> Use max(wall_now, anchor + monotonic_elapsed) as the reading rule.
> Backward steps lose to monotonic branch to keep advancing; forward steps win to track system corrections.

**Diff line:** `reading = max(wall_now, anchor + monotonic_elapsed)`

**Detector's reason:** The change removes the use of `last` to enforce monotonicity against smaller elapsed inputs.

- verdict: correct
  - note: replaces max(wall_now, anchor + elapsed) with a slewed rule; a forward step no longer wins in one reading

## F064 (ucal)

**Recorded rejected alternative** `r/g2074788f713e-2-2` (source git:2074788f713ef4331ede63d3dc963f8e74e7565c):

> Rejected: Refuse a forward jump because it disagrees with the monotonic projection.
> A forward jump is a correction arriving; refusing would prefer this process's opinion to the system's.

**Diff line:** `refusing it would prefer this process's opinion of the time to the system's.`

**Detector's reason:** The change introduces a new behavior for forward steps that contradicts the rejected alternative.

- verdict: arguable
  - note: does not refuse the forward jump but slews it, so readings lag the system clock while it is absorbed

## F065 (ucal)

**Recorded decision** `d/g5880e472260e-1` (source git:5880e472260eb32c58821c7fe4a7497154d590a8):

> Read notes for version being built rather than newest sorted file
> Sorting filenames picked newest by lexicographic order, causing 1.10.0 to be missed. Deriving the version from [workspace.package] removes ordering and avoids guessing.

**Diff line:** `pub fn check_ci_covers_the_procedure(root: &Path) -> Result<usize, Vec<String>> {`

**Detector's reason:** The comment now states that the notes for the version being built are used only when they exist, which contradicts the recorded decision to read notes for the version being built rather than the newest sorted file.

- verdict: correct
  - note: falls back to the newest notes (by version) when the current version's are missing

## F066 (ucal)

**Recorded decision** `d/g2074788f713e-4` (source crates/ucal/src/clock.rs@2074788f713e):

> Make Session::reading a pure function of its inputs.
> Pure function lets tests drive wall clock backwards repeatedly and assert monotonic sequence.

**Diff line:** `/// backwards or forwards and check that the answer does not follow it.`

**Detector's reason:** The change modifies the behavior of the `reading` function, which contradicts the decision.

- verdict: wrong

## F067 (ucal)

**Recorded rejected alternative** `r/g31e52cc10b1f-4-1` (source git:31e52cc10b1f6079a130e3bc27479739d7e393eb):

> Rejected: Attempt to measure or estimate clock accuracy.
> Offline operation provides no reference, and short-baseline rate error reports quantisation as drift.

**Diff line:** `**The session clock's drift is reported too**, as `clock_measured.session_drift``

**Detector's reason:** This change introduces reporting of the session clock's drift, which contradicts the decision to reject attempting to measure or estimate clock accuracy.

- verdict: arguable
  - note: as F015: drift between two in-process clocks, disclaimed as not accuracy

## F068 (ucal)

**Recorded decision** `d/g5880e472260e-1` (source git:5880e472260eb32c58821c7fe4a7497154d590a8):

> Read notes for version being built rather than newest sorted file
> Sorting filenames picked newest by lexicographic order, causing 1.10.0 to be missed. Deriving the version from [workspace.package] removes ordering and avoids guessing.

**Diff line:** `/// **A version bumped without its notes is an error, not a fallback.**`

**Detector's reason:** The change now allows a missing notes file for the current version to fall back to the newest notes, contradicting the decision that such a case should be an error.

- verdict: correct
  - note: falls back to the newest notes (by version) when the current version's are missing

## F069 (ucal)

**Recorded decision** `d/g5880e472260e-2` (source git:5880e472260eb32c58821c7fe4a7497154d590a8):

> Error naming the version when current release notes missing
> The old sort silently read the previous cycle's file and passed when notes were never written. Now a missing notes file for the current version is an explicit error.

**Diff line:** `/// guess closes.`

**Detector's reason:** The change now allows a missing notes file for the current version to fall back to the newest notes, contradicting the decision that such a case should be an error.

- verdict: correct
  - note: a missing notes file for the current version is no longer an error while any older notes exist

## F070 (smysl)

**Recorded rejected alternative** `r/g90ec2f781421-6-1` (source tests/dispatch.rs@90ec2f781421):

> Rejected: inherit stdin from the terminal for dispatch test commands
> fmt and check read stdin when given no path; inherited terminal would hang instead of failing.

**Diff line:** `and explicit paths still win, and `smysl check -` still reads stdin, so a pipeline that spelled`

**Detector's reason:** The change introduces a new behavior where `smysl check` with no path checks `.smysl/store` instead of reading standard input, which contradicts the rejected alternative.

- verdict: arguable
  - note: the dispatch tests are untouched; check no longer reads stdin by default (as dev local14-a F010)

## F071 (smysl)

**Recorded decision** `d/g4968383dc452-15` (source Documentation/READINESS.md@4968383dc452):

> Revise gate 7 blocked command explanation, no longer 'decision about book'.
> The block was due to filename-keyed fixtures vs position-keyed chapters, not a book decision.

**Diff line:** `- run: python3 scripts/verify-spec-tables.py`

**Detector's reason:** The change introduces a new job 'dep-versions' that checks internal crate requirements against the workspace version, contradicting the decision to revise gate 7 blocked command explanation.

- verdict: wrong

## F072 (ucal)

**Recorded rejected alternative** `r/g2074788f713e-6-2` (source crates/ucal/src/clock.rs@2074788f713e):

> Rejected: Anchor the session at process start-up.
> A command that never asks the time should not read a clock, and most do not.

**Diff line:** `let nanos = base.elapsed().as_nanos();`

**Detector's reason:** The change introduces a new function `since` that calculates the elapsed time using `base.elapsed().as_nanos()`, which contradicts the recorded claim that rejected anchoring the session at process start-up.

- verdict: wrong

## F073 (smysl)

**Recorded decision** `d/g4968383dc452-1` (source nodejs/README.md@4968383dc452):

> Upgrade nodejs conformance target from C-Read to C-Produce.
> To provide a fourth independent derivation of uids and a third witness for §2.3 beyond the Rust, strengthening the evidence that the specification is sufficient.

**Diff line:** `- run: python3 scripts/verify-spec-tables.py`

**Detector's reason:** The change introduces a new job 'dep-versions' that checks internal crate requirements against the workspace version, contradicting the decision to upgrade nodejs conformance target from C-Read to C-Produce.

- verdict: wrong

## F074 (smysl)

**Recorded decision** `d/g4968383dc452-5` (source CHANGELOG.md@4968383dc452):

> NFC-normalise text in nodejs CBOR encoder.
> §3 constraint 6 asks for normalisation at the encoder; previously a no-op for re-encode but needed for authored text.

**Diff line:** `nothing on the wire.`

**Detector's reason:** The change introduces new features like relation identity, withdrawal, who asserted an edge, live rebuttal, and resolution, contradicting the decision to NFC-normalise text in nodejs CBOR encoder.

- verdict: wrong

## F075 (smysl)

**Recorded decision** `d/g4968383dc452-21` (source Documentation/READINESS.md@4968383dc452):

> Do not apply 22% artefact fraction to other modules.
> One module evidence is not generalizable; applying the fraction would repeat the error of quoting a number without its configuration.

**Diff line:** `Nothing shipped yet. What this cycle starts from, carried from 1.3.0 (details there):`

**Detector's reason:** The change indicates that nothing has been shipped yet for version 1.4.0, contradicting the decision to do not apply 22% artefact fraction to other modules.

- verdict: wrong

## F076 (ucal)

**Recorded rejected alternative** `r/g31e52cc10b1f-4-1` (source git:31e52cc10b1f6079a130e3bc27479739d7e393eb):

> Rejected: Attempt to measure or estimate clock accuracy.
> Offline operation provides no reference, and short-baseline rate error reports quantisation as drift.

**Diff line:** `it. §8.4 makes operation offline, so there is no reference — and a rate error`

**Detector's reason:** The change introduces a new feature `session_drift` that measures the difference between two clocks, which contradicts the rejection of measuring or estimating clock accuracy.

- verdict: arguable
  - note: as F015

## F077 (smysl)

**Recorded rejected alternative** `r/g532e4d229a6c-1-1` (source tests/cmd_fmt.rs@532e4d229a6c):

> Rejected: Writing unit tests that call cmd_fmt functions directly instead of running the binary
> The command reads the filesystem, parses, writes, and prints with no seam to call, so only running the binary reaches these decisions.

**Diff line:** `assert_eq!(canonical.code, SUCCESS, "{}", canonical.stderr);`

**Detector's reason:** This line calls `cmd_fmt` functions directly instead of running the binary, contradicting the rejection.

- verdict: wrong

## F078 (smysl)

**Recorded rejected alternative** `r/ga18e230ae345-6-1` (source CHANGELOG.md@a18e230ae345):

> Rejected: Leave the changelog entry as 'Nothing yet.'
> To record the removal and its justification for the 0.8.0 release notes.

**Diff line:** `Nothing yet.`

**Detector's reason:** The changelog entry is updated to describe the removal and its justification, contradicting the rejection of leaving it as 'Nothing yet.'

- verdict: wrong

## F079 (smysl)

**Recorded prerequisite** `p/ga18e230ae345-6-1` (source CHANGELOG.md@a18e230ae345):

> The changelog has an Unreleased — 0.8.0 section where the removal can be recorded.

**Diff line:** `## Unreleased — 1.2.0`

**Detector's reason:** The change updates the unreleased section to 1.3.0, contradicting the prerequisite that it should be 1.2.0.

- verdict: wrong

## F080 (ucal)

**Recorded prerequisite** `p/g2074788f713e-2-1` (source crates/ucal/src/clock.rs@2074788f713e):

> Ticks has `try_add` to compute anchor plus monotonic elapsed.

**Diff line:** `reading = max(wall_now, anchor + monotonic_elapsed)`

**Detector's reason:** The change removes the use of `last` to enforce monotonicity against smaller elapsed inputs.

- verdict: wrong

## F081 (ucal)

**Recorded rejected alternative** `r/g2074788f713e-4-1` (source crates/ucal/src/clock.rs@2074788f713e):

> Rejected: Have Session::reading read the wall clock itself.
> A pure function lets tests drive a wall clock backwards and assert the answer does not follow.

**Diff line:** `let nanos = base.elapsed().as_nanos();`

**Detector's reason:** The change introduces a new function `since` that calculates the elapsed time using `base.elapsed().as_nanos()`, which contradicts the recorded claim that rejected having `Session::reading` read the wall clock itself.

- verdict: wrong

## F082 (ucal)

**Recorded rejected alternative** `r/g2074788f713e-6-2` (source crates/ucal/src/clock.rs@2074788f713e):

> Rejected: Anchor the session at process start-up.
> A command that never asks the time should not read a clock, and most do not.

**Diff line:** `**The session clock's drift is reported too**, as `clock_measured.session_drift``

**Detector's reason:** This change introduces reporting of the session clock's drift, which contradicts the decision to reject anchoring the session at process start-up.

- verdict: wrong

## F083 (smysl)

**Recorded decision** `d/g69ea5c05d859-9` (source src/lib.rs@69ea5c05d859):

> Strengthen the facade re-export test to check every supported version is accepted and FORMAT_VERSION_DEFAULT …
> The previous test only checked the first entry, which would have kept passing while a second supported version went unchecked.

**Diff line:** `nothing on the wire.`

**Detector's reason:** The change introduces new features like relation identity, withdrawal, who asserted an edge, live rebuttal, and resolution, contradicting the decision to strengthen the facade re-export test to check every supported version is accepted and FORMAT_VERSION_DEFAULT.

- verdict: wrong

## F084 (ucal)

**Recorded rejected alternative** `r/g2074788f713e-1-1` (source git:2074788f713ef4331ede63d3dc963f8e74e7565c):

> Rejected: Let each wallclock tick read the raw system clock directly.
> A backward step would produce a face that goes back in time.

**Diff line:** `it. §8.4 makes operation offline, so there is no reference — and a rate error`

**Detector's reason:** The change introduces a new feature `session_drift` that measures the difference between two clocks, which contradicts the rejection of letting each wallclock tick read the raw system clock directly.

- verdict: wrong

## F085 (ucal)

**Recorded rejected alternative** `r/g5880e472260e-2-1` (source git:5880e472260eb32c58821c7fe4a7497154d590a8):

> Rejected: silently fall back to the previous cycle's notes file
> This fallback could not catch a version bumped without notes and would silently pass on the previous cycle's file.

**Diff line:** `/// **A version bumped without its notes is an error, not a fallback.**`

**Detector's reason:** The comment now states that a missing notes file will fall back to the newest notes, which contradicts the recorded rejection of silently falling back.

- verdict: correct
  - note: falls back to the newest notes when the current version's are missing

## F086 (ucal)

**Recorded rejected alternative** `r/g2074788f713e-2-1` (source git:2074788f713ef4331ede63d3dc963f8e74e7565c):

> Rejected: Freeze the clock until the wall clock catches up after a backward step.
> The monotonic branch keeps advancing at the oscillator's rate instead.

**Diff line:** `oscillator's rate rather than freezing until the wall clock catches up.`

**Detector's reason:** The change introduces a new behavior for backward steps that contradicts the rejected alternative.

- verdict: wrong

## F087 (ucal)

**Recorded rejected alternative** `r/g5880e472260e-3-1` (source xtask/src/citations.rs@5880e472260e):

> Rejected: keep the two old tests for version-sort behavior
> The old tests exercised sorting behavior that no longer exists after replacing the sort with current-version lookup.

**Diff line:** `// As strings: 0.1.1 < 0.1.10 < 0.1.2, and 1.10.0 < 1.12.0 < 1.9.0.`

**Detector's reason:** The test now checks for numerical sorting behavior, which contradicts the rejection of keeping the old tests for version-sort behavior.

- verdict: wrong

## F088 (smysl)

**Recorded decision** `d/g69ea5c05d859-14` (source git:69ea5c05d85933ca248ea9ca78669f8a76e2ed7e):

> Defer flipping the writer default to smysl/1.0 until 0.14 is published.
> §8.2 requires readers to refuse a version absent from their list. Flipping the writer before 0.14 is in the field would make every other implementation reject the output.

**Diff line:** `would resolve the new `smysl-ingest` against it and fail to build. Nothing here could see it,`

**Detector's reason:** The change introduces a new internal requirement for workspace version 1.3.0, contradicting the decision to defer flipping the writer default to smysl/1.0 until 0.14 is published.

- verdict: wrong

## F089 (ucal)

**Recorded rejected alternative** `r/g5880e472260e-4-1` (source xtask/src/citations.rs@5880e472260e):

> Rejected: assert only that some error came back and the message named the version
> The loose assertion passed even with the check disabled because read_to_string also fails and names the path containing the version.

**Diff line:** `/// **A version bumped without its notes is an error, not a fallback.**`

**Detector's reason:** The comment now states that a missing notes file falls back to the newest notes, contradicting the rejected alternative of asserting only that some error came back and the message named the version.

- verdict: wrong

## F090 (smysl)

**Recorded decision** `d/g4968383dc452-12` (source CHANGELOG.md@4968383dc452):

> Correct README claims that tables were asserted against document.
> The tables were hand-typed copies, not assertions against the document. Spec-tables gate now parses the document.

**Diff line:** `- run: python3 scripts/verify-spec-tables.py`

**Detector's reason:** The change introduces a new job 'dep-versions' that checks internal crate requirements against the workspace version, contradicting the decision to correct README claims that tables were asserted against document.

- verdict: wrong

## F091 (smysl)

**Recorded decision** `d/g4968383dc452-23` (source nodejs/src/uid.js@4968383dc452):

> Implement status integers and source/kind enums as constants in nodejs.
> These values are needed for uid derivation and rule M comparisons; previously only recoverable from fixtures.

**Diff line:** `Nothing shipped yet. What this cycle starts from, carried from 1.3.0 (details there):`

**Detector's reason:** The change indicates that nothing has been shipped yet for version 1.4.0, contradicting the decision to implement status integers and source/kind enums as constants in nodejs.

- verdict: wrong

## F092 (ucal)

**Recorded rejected alternative** `r/g5880e472260e-1-1` (source git:5880e472260eb32c58821c7fe4a7497154d590a8):

> Rejected: sort filenames numerically / by version to pick the newest
> Sorting numerically would fix only the one exposed bug and would leave the guess in place.

**Diff line:** `/// Parse the Contents table, newest version first.`

**Detector's reason:** The change introduces a new function `parse_rows` that sorts versions numerically, contradicting the claim that sorting filenames numerically would fix only one bug and leave guessing in place.

- verdict: wrong

## F093 (ucal)

**Recorded prerequisite** `p/g5880e472260e-1-2` (source xtask/src/citations.rs@5880e472260e):

> Release notes are stored as {version}.md files under Documentation/Release_Notes.

**Diff line:** `# 1.10.0 — 2026-08-28`

**Detector's reason:** The release notes for the current 1.10.0 cycle are now marked as released, contradicting the constraint that they should be kept in Documentation/Release_Notes/1.10.0.md.

- verdict: wrong

## F094 (smysl)

**Recorded decision** `d/g4968383dc452-12` (source CHANGELOG.md@4968383dc452):

> Correct README claims that tables were asserted against document.
> The tables were hand-typed copies, not assertions against the document. Spec-tables gate now parses the document.

**Diff line:** `### The lifecycle of edges and disagreements — specification draft and library`

**Detector's reason:** The change introduces new features like relation identity, withdrawal, who asserted an edge, live rebuttal, and resolution, contradicting the decision that tables were hand-typed copies, not assertions against the document.

- verdict: wrong

## F095 (smysl)

**Recorded rejected alternative** `r/ga18e230ae345-6-1` (source CHANGELOG.md@a18e230ae345):

> Rejected: Leave the changelog entry as 'Nothing yet.'
> To record the removal and its justification for the 0.8.0 release notes.

**Diff line:** `- **Publishing 0.10.0 and 0.11.0.** Both are tagged and merged and neither is on crates.io, so`

**Detector's reason:** The changelog entry now states that 0.11.0 is published, contradicting the rejected alternative of leaving the changelog entry as 'Nothing yet.'

- verdict: wrong

## F096 (ucal)

**Recorded prerequisite** `p/g2074788f713e-7-3` (source crates/ucal/src/clock.rs@2074788f713e):

> Raw `SystemTime` reading code already exists to move into `wall_instant`.

**Diff line:** `let nanos = base.elapsed().as_nanos();`

**Detector's reason:** The change introduces a new function `since` that calculates the elapsed time using `base.elapsed().as_nanos()`, which contradicts the recorded constraint that raw `SystemTime` reading code already exists to move into `wall_instant`.

- verdict: wrong
