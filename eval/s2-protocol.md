# S2 — recorder-first or extractor-first

**Question:** are prerequisites recorded *while a change is made* better than prerequisites extracted
*afterwards* from the commit? The answer decides the order of Phase 4 onward
(docs/implementation-plan.md §4, §8).

## Design

Each change is implemented **once**. The rationale for that one commit is captured three ways:

| Arm | Source | When |
|---|---|---|
| **A: recorder** | the agent writes decisions / prerequisites / alternatives as it works | during the change |
| **B1: extractor, rich message** | research pipeline (research-pro-v2) on the commit as the agent wrote it | after |
| **B2: extractor, plain message** | same pipeline on the same diff, with the message replaced by a one-line conventional message | after |

Implementing a change twice would bias the second attempt. B2 exists because an agent that records
rationale also writes better commit messages, and B1 alone would credit the recorder's message to
the extractor.

All three are scored with `cargo-smysl-eval` against **owner labels of the commit**, written blind
before any arm's output is opened. The label template shows the task statement, the diff, and B2's
plain message, never A's record or the agent's own message.

## The ten changes

Real, small, and useful, so the experiment also advances the project. None is merged as part of the
experiment; each lives on an experiment branch until the owner decides.

| # | Repository | Change |
|---|---|---|
| 1 | rust_smysl | `cargo smysl check`: load a corpus directory and report smysl `check` diagnostics |
| 2 | rust_smysl | `cargo smysl why <label>`: resolve a label (refusing ambiguity) and print its decision, prerequisites and dependents |
| 3 | rust_smysl | facts: port macro-argument parsing from the research extractor, with tests |
| 4 | rust_smysl | facts: const values and struct field types, with tests |
| 5 | rust_smysl | corpus: write a per-commit surface document and a merged CBOR store under `.smysl/` |
| 6 | smysl (1.4.0 branch) | `smysl import` honours `--format surface` |
| 7 | smysl (1.4.0 branch) | `smysl import` keeps summaries within the 30-token limit (key columns in the gist, the rest in the body) |
| 8 | smysl (1.4.0 branch) | a config error exits 6, as the changelog states |
| 9 | smysl (1.4.0 branch) | an unknown provider kind no longer reports "malformed provider response" |
| 10 | smysl (1.4.0 branch) | align json-ast `GIST_MAX_CHARS` with `check`'s summary bound (verify first whether `f740474` already did) |

Changes 6–10 are the smysl 1.4.0 requests from the plan (§10); they go to the owner as proposals, not
pushed.

## Arm A: what the agent records

The agent receives the task and one extra instruction: *as you work, write
`.smysl/record/<task>.json` in the extraction shape*:

```json
{
  "decisions":     [{"decision": "…", "kind": "act|decline", "rationale": "…", "quote": ""}],
  "prerequisites": [{"decision": 1, "text": "…", "kind": "existing-behaviour|invariant|tool-setting|prior-change|assumption", "quote": ""}],
  "alternatives":  [{"decision": 1, "alternative": "…", "reason": "…", "quote": ""}]
}
```

The same shape the extractor emits, so one scorer compares all arms. The record is written as the
agent learns something, not reconstructed at the end; the transcript is kept to check that.

## Procedure

1. For each change: branch, give the agent the task (arm A instruction included), let it commit.
2. Save A's record. Build B2's commit: same tree, message = one-line conventional message written by the
   experimenter from the task statement alone.
3. Owner labels the commit blind (template: task statement, diff, B2's message).
4. Run the extractor on B1 and B2 commits: `research-pro-v2`, same model and prompts as S0.
5. `smysl-eval adjudicate` for systems `s2-recorder`, `s2-extract-rich`, `s2-extract-plain`; the owner
   adjudicates; `smysl-eval score` each.

## Measures

- Prerequisite precision and recall per arm (primary); decision and alternative precision/recall
  (secondary).
- Declines found per arm (the research showed extraction finds them only with a pro model).
- Cost: tokens per change per arm; A's cost is the agent's extra output only.

## Gate

- **Recorder-first** if A's prerequisite precision ≥ B1's, **and** A's recall exceeds B1's by at least
  10 points. Then capture (hooks, agent integration) moves into Phase 4 and extraction becomes backfill.
- **Extractor-first** otherwise.
- Report B2 regardless: if B1 ≫ B2, message quality is doing the work, and the product needs a message
  policy in either order.
