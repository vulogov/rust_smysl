# S3 validation, step 4, repeated on the run corpus

`step4-corpus-packs.md` checked single-commit stores from research-pro-v2, where the budget did not bind.
The run uses research-deepseek-v2 (every extracted commit before the base, staged against real commit
text), with a binding 4k budget: `smysl-eval s3-context`, output in `eval/s3/context/` and
`context/summary.txt`.

## Selection

- **Scope** (tasks): the commits whose diff touches the task's files (`tasks.toml`), plus the commits holding
  the statement's top six BM25 hits across the repository. The second part is needed: task 1's rationale
  (the round-trip guard is deliberately untested) was recorded in 532e4d2, which touched only
  `tests/cmd_fmt.rs`. When no extracted commit touched the files (task 6), the scope is the whole repository.
- **Questions**: the whole repository.
- **Focus**: the statement's (or question's) BM25 hits, up to six, reduced until focus and closure fit.
  Salience is seeded from units sourced in the task's files.

A first version packed the whole repository for every task. The fill was mostly L0 decisions from
unrelated commits (216 units, few with bodies), and the prerequisites of tasks 5 and 7 did not make it in.

## Is the at-risk prerequisite in the packed context?

| # | Guarded? | In the context | As packed |
|---|---|---|---|
| 1 | no | yes | "Do not write a test for the round-trip guard; record it as a known survivor." |
| 2 | no | yes | "fmt and check read stdin when given no path, so inherited terminal would hang." |
| 3 | no | yes | "ingest and usage write to .smysl in the working directory, so a shared checkout would be dirtied." |
| 4 | no | yes | "… cargo-mutants reuses build directories, so a mutant that misroutes a write can leave a fixture rewritten …" (a decision body) |
| 5 | no | yes | "Seven commands require an argument, so clap rejects bare names before the router runs." |
| 6 | control | yes | "These values are needed for uid derivation and rule M comparisons …" |
| 7 | no | yes | "A command that never asks the time should not read a clock, and most do not." |
| 8 | control | yes | "Backward steps lose to monotonic branch to keep advancing; forward steps win to track system corrections." |
| 9 | control | yes | "Rejected: sort filenames numerically / by version to pick the newest" |
| 10 | control | **no** | not extracted: no unit in research-deepseek-v2 says a cycle's notes are created when it opens |
| 11 | no | yes | "… a committed file cannot race and a reader can open it." |
| 12 | no | **no** | not extracted: no unit says a tick count is unsigned or that divergence is a magnitude plus a direction |

**Result:** 7 of 8 non-control tasks carry their prerequisite. Task 12's is an extraction miss (the
research-pro-v2 extraction had it as `p/g2074788-8-1`). The run keeps task 12 as is: S3 tests the product as
it would ship, extraction misses included, and the report counts it separately. Control 10 needs no corpus
to hold.
