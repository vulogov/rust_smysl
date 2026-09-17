# S3 questions: "why is this like this?"

Ten questions about the code the S3 tasks touch. Each is answered twice, from the corpus (packed units
only) and from `git log` / `git blame` / the diff only, by the same agent configuration. The owner
judges both answers blind to their source: which is correct, and which is more useful. Gate: corpus
answers win at least 6 of 10 (docs/implementation-plan.md §4, eval/s3-protocol.md).

The **source** column says where the answer is recorded, so the owner can check correctness; it is
not shown to the answering agent.

| # | Repo | Question | Where the answer is recorded |
|---|---|---|---|
| 1 | smysl | Why does the dispatch test pass `/dev/null` as stdin to every command? | 90ec2f7: `fmt` and `check` read stdin when given no path |
| 2 | smysl | Why does each dispatch-test command run in its own scratch directory? | 90ec2f7: `ingest` and `usage` write `.smysl/…` relative to the working directory |
| 3 | smysl | Why is the list of 22 subcommands hardcoded in `tests/dispatch.rs` rather than read from the binary? | 90ec2f7: a list derived from the binary shrinks silently when a command is removed |
| 4 | smysl | Why is `fmt`'s round-trip guard not covered by a test? | 532e4d2: declined; the guard only triggers when the writer produces something the parser reads back differently |
| 5 | smysl | Why do the `cmd_fmt` tests write their own documents instead of using the corpus fixtures? | 532e4d2: cargo-mutants reuses build directories, so a mutant can leave a shared fixture rewritten |
| 6 | smysl | Why does the `COMMANDS.iter().find` `==`/`!=` mutant not have a test? | 90ec2f7: equivalent mutant; the only field read is in the `_` arm no input reaches |
| 7 | ucal | Why does `Session::reading` take the wall clock and elapsed time as arguments instead of reading the clock? | 2074788: the rule is a pure function; no clock is read inside it |
| 8 | ucal | Why does the session clock accept a large forward jump but never go backwards? | 2074788: a forward jump is a correction arriving; a backward step is not followed |
| 9 | ucal | Why are release notes not found by sorting file names by version? | 5880e47: numeric sorting was declined; the version comes from `[workspace.package]` |
| 10 | ucal | Why is the observations example a committed file? | cc3aafe: generated observations raced and shipped a flaky test; a committed fixture cannot race |
