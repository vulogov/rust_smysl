# S4 — does `cargo smysl check` operate as designed?

**The tool's promise** is that it operates as designed. It makes no promise about how an agent or a
person acts on what it reports. S4 is therefore a functional acceptance of one command: given a change
and the corpus of the repository's history, `check` reports the recorded decisions, prerequisites and
rejected alternatives the change contradicts, and reports nothing else.

S3 (`s3-protocol.md`) measured something different: whether packed rationale in an agent's prompt
changes the agent's behaviour. It did not. That result is kept as information about one usage pattern,
and shapes the design below: findings are produced from the change itself, after it is made, as a
signal a hook, CI or a reviewer can act on.

## 1. What `check` is designed to do

**Input:** a diff (a commit, a commit range, or the working tree against `HEAD`) and the corpus built
from the extracted commits that are ancestors of the diff's base. The commit message is not an input:
a working-tree change has none, and a real commit's message would let the check read its own answer.

**Pipeline:**
1. **Candidates, deterministic.**
   - Units anchored to a file the diff touches (a source `path@sha`).
   - BM25 hits for a query built from the diff: identifiers in added and removed lines, and the touched
     paths.
   - Only decisions, prerequisites (`constraint`) and rejected alternatives are candidates.
2. **Context.** The candidates are the focus of `smysl::pack` within a budget, so each arrives with its
   decision, its prerequisites and its live rebuttals (pack rule R).
3. **Judgement, one model call per diff.** Output: for each candidate, `contradicts`, `consistent` or
   `unrelated`, with the unit's label, one line of the diff as the quote, and one sentence of reason.
4. **Validation, deterministic.** A `contradicts` verdict is dropped when:
   - its label is not in the pack, or
   - its quote is not a line of the diff (the D8 quote rule, applied to the diff).
5. **Report.**
   - One finding per surviving `contradicts`: the unit's label, kind, text and source commit, the diff
     line, and the reason.
   - `--json` for machines.
   - **Exit codes:** 0 when there is no finding, 5 when there are findings (smysl's convention for items
     awaiting review), and anything else for an error.

**Parameters** (tunable on the development set only; frozen before held-out measurement): candidate
limit (start: 12), pack budget (start: 3000 tokens), diff truncation (start: 400 lines), units judged per
call (start: 6), provider and model, and the system prompt. Provider, endpoint, model, key variable,
context window and prompt are settings, not constants (D16): `--provider ollama|openai`, `--model`,
`--endpoint`, `--key-var`, `--num-ctx`, `--prompt-file`, each with an `SMYSL_CHECK_*` variable.

**Fitting (D17).** Each call is sized to the model's context limit less the tokens reserved for the
answer. Units are judged in one call where they fit, and split only when they do not; a split, and a
truncated diff, are reported in the result and on stderr, because a split changes what the model sees.

**Which model.** The measured configuration is named in the result. **Local first** (`ollama`,
`Qwen2.5-Coder:7B-Instruct`, the project's zero-budget default): the gate is attempted locally, and a
hosted model is measured only after a local run shows the pipeline works. Diff lines are numbered and a
verdict quotes by number, because a small model paraphrases a copied line.

**Where it lives during S4:** `smysl-eval s4-check` in the eval crate, using the corpus and git crates.
Its model client is development tooling. Porting it to `cargo smysl check` is Phase 3 work after S4,
under the self-contained rule and the plan's item 7 (own client or smysl's `model` feature).

## 2. Data

### Development set (tuning allowed)

| Source | Diffs | Label |
|---|---|---|
| S3 naive patches (`s3/validation/patches/`) | 12 | contradicting (each breaks its task's prerequisite, oracle-verified) |
| S3 run1 and run2 agent diffs (`s3/results/`) | 52 | oracle outcome: violated → contradicting; intact or stopped → not contradicting that prerequisite |
| Real commits: every 2nd non-merge commit in the development range (below) | about 59 | clean unless the owner confirms a flag |

### Held-out set (frozen before the detector is run on it)

| Source | Diffs | Label |
|---|---|---|
| New agent runs of the 12 S3 tasks, control arm (no corpus), 2 repetitions, same harness | 24 | oracle outcome, as above |
| Real commits: the non-merge commits in the development range not used above | about 59 | clean unless the owner confirms a flag |
| Real commits after the S3 bases: smysl `183be01..c7bf5a3` (5), ucal `425bb3a..HEAD` (5) | 10 | clean unless the owner confirms a flag |

**Development range:** non-merge commits that are not S0 commits, after the oldest S0 commit and up to the
S3 base, in smysl and ucal: about 88 in smysl (`a18e230..183be01`) and 30 in ucal (`8fdb9b3..425bb3a`). For each real commit the corpus holds only the S0 commits that are its
ancestors, as `check` would see it at that point in history.

**The split is fixed** in `s4/sets.toml` (`s4/sets.py`), generated before the detector existed: 12 naive
patches, 48 agent diffs (4 empty ones skipped) and 59 real commits for development; 59 + 10 real commits
and 24 agent runs still to be made for the held-out set.

**Freezing:** the detector's code and parameters are committed, and the commit id recorded in
`s4/frozen.txt`, before any held-out diff is checked. A held-out result obtained after a later change to
the detector is reported, but does not count for the gate.

## 3. What counts

**A hit** on a contradicting diff: at least one finding whose unit states the task's at-risk
prerequisite or the decision it protects. Decided by the pattern below, or by the owner when the pattern
does not match but the unit states the same thing.

| Task | Pattern on the finding's unit text (case-insensitive) | Prerequisite in the corpus? |
|---|---|---|
| 1 | `round.trip guard\|re-?parse` | yes |
| 2 | `stdin` | yes |
| 3 | `working directory\|ledger` | yes |
| 4 | `reuses build dir\|fixture` | yes |
| 5 | `bare name\|required arg\|before the router` | yes |
| 6 | `rule m\|status integer\|normative` | yes |
| 7 | `never asks the time\|process start\|first use\|lazil` | yes |
| 8 | `forward (step\|jump)\|correction` | yes |
| 9 | `numeric\|sort` | yes |
| 10 | `created when\|cycle opens\|refus` | **no** (extraction miss) |
| 11 | `cannot race\|committed (file\|fixture)` | yes |
| 12 | `unsigned\|direction\|magnitude` | **no** (extraction miss) |

**Confirmed by the owner, 2026-09-17, as written.**

**Every flag** on any diff is adjudicated by the owner, blind to whether the diff is an agent run, a
naive patch or a real commit:
- **correct**: the change contradicts the unit;
- **wrong**: it does not;
- **arguable**: counted as wrong for precision, reported separately.

A flag the owner marks correct on a real commit is a true finding, not a false one: real history does
reverse decisions.

## 4. Gate (held-out set)

`check` operates as designed **as a gate** when all of these hold:

| Measure | Bound |
|---|---|
| **Recall**: contradicting diffs hit, over tasks whose prerequisite is in the corpus | ≥ 0.70 |
| **Precision**: flags the owner marks correct, over all flags | ≥ 0.80 |
| **Clean real commits** with a flag the owner marks wrong | ≤ 10% |
| **Determinism**: candidates and pack for the same diff and corpus, across two runs | identical |
| **Validation**: `contradicts` verdicts dropped by step 4 | reported; no bound |
| **Cost and time**: model cost and wall time per diff | reported; no bound, feeds item 8 |

Also reported, not gated: recall on tasks 10 and 12, where the corpus lacks the prerequisite (a
correct detector misses both; a hit there is a wrong reason or a lucky unit); recall and precision per
repository.

**Either outcome is an outcome.** S4 decides how `check` ships, not whether the project continues.

- **All bounds met:** `check` ships as a gate. Exit 5 is meant to block (a CI step, a pre-commit or stop
  hook), and the measured recall and precision go into its documentation.
- **Any bound missed:** `check` ships as **advisory**. It exits 0 with findings unless `--strict` is
  given, and its documentation states the measured figures and which step loses hits (candidates, pack,
  judgement, validation). One step may then be reworked on the development set and measured again on a
  **fresh** held-out set (new agent runs, the next real commits). Each result is recorded, whichever way
  it goes.

## 5. After S4 (information, not a gate)

Whichever way S4 goes, an optional effect run uses the S3 harness with a third arm: a Claude Code Stop hook runs
`check` on the working tree and returns its findings to the agent. Outcomes:

- **silent violation**: oracle violated, and the final message does not name the flagged unit or its
  source commit;
- **disclosed violation**: oracle violated, and the final message names it;
- **stopped**;
- **intact**.

This measures how an agent uses the report. The tool's acceptance does not depend on it.

## 6. Cost estimate

- **Held-out agent runs:** 24 runs at about $1.50, about $36.
- **Detector calls:** about 200 diffs at DeepSeek prices, a few dollars.
- **Owner adjudication:** one line per flag.

## Result (2026-09-19): advisory

Measured on the frozen configuration (`s4/frozen.txt`): local Ollama `Qwen2.5-Coder:14b`, smysl 1.6.0,
12 candidates, a 3 000-model-token pack with `reserving`, 24 units judged in chunks of 6, two passes
(order seeds 0 and 5) with a flag counted only when both report it.

| Measure | Development (35 diffs) | Held-out (93) | Bound |
|---|---|---|---|
| Recall by pattern, agreed | 0.58–0.65 | **0.38** (5/13) | ≥ 0.70 |
| Flags, agreed | 75–76 | **96** | — |
| Real commits flagged | 0 of 11 | **9 of 69 (13%)** | ≤ 10% wrongly |
| Precision | 0.29 (owner-adjudicated, 76 flags) | **0.10** (10 of 96; 5 arguable) | ≥ 0.80 |

Adjudication closed 2026-09-19 (`results/held-a/adjudication.md`, 96 agreed flags, blind to provenance):
**10 correct, 5 arguable, 81 wrong.** All three bounds are missed on the frozen local configuration.

**`check` ships advisory**, per §4: it exits 0 with findings unless `--strict` is given, and its
documentation states these figures. The mode did not depend on the adjudication — two bounds were already
missed — but the precision figure settles how the output must be described: on a local 14B, nine flags in
ten are wrong, so `check` is a prompt to look, not a claim that something is wrong.

**Precision does not survive the move off the development set**: 0.29 there, 0.10 here, on the same
configuration. The development figure was tuned against, and is not a prediction. Any later claim about
this pipeline needs its own held-out set.

### What the measurement established

- **The deterministic half works.** Candidates, packing, label and quote validation and the determinism
  check all hold: repeated runs give identical candidates, packs and fingerprints, and validation dropped
  33 `contradicts` verdicts on the held-out set whose label or quoted line did not check out.
- **The judgement is the weak link, and it is model-bound.** Same retrieval, same packs, same prompt:
  the local 14B gave recall 0.65 and precision 0.29 on development; `deepseek-v4-pro` gave 0.75 and ~0.89
  on the flags already adjudicated, for about $2 a pass. The owner's budget decision (2026-09-17) is local,
  so the shipped default is local and advisory.
- **Two-pass agreement is a real filter**: 170 and 192 flags became 96, and it removed every false flag on
  a real commit in development. It costs recall (7/13 → 5/13 held-out).
- **Retrieval-side filtering is not.** `Hit::terms` (smysl 1.6, R22) let this be tested directly: the
  strongest term behind a correct flag and behind a wrong one overlap almost completely (median 14.7 vs
  11.7), so a threshold removes at best 4 of 32 wrong flags before it costs correct ones.
- **A stricter prompt made it worse**, not better: recall 0.30, and it dropped 10 of the owner's 22 correct
  flags while removing 35 of 48 wrong ones. Reverted; the protocol's one rework is spent.
- **smysl gained R10, R12, R15–R24 from this work**, all but the deliberately deferred ones implemented in
  1.4.0, 1.5.0 and 1.6.0.

### Caveats a reader should have

- **The held-out agent diffs are less finished than the development ones**: the same tasks and harness
  produced a median of 21 turns against 50, and 12 of 24 left a failing test (all the same test, the manual
  transcript check). Their oracle labels are unaffected, but the two sets are not drawn from the same
  distribution.
- **Two cases answered in an unreadable shape** and are recorded as finding nothing, rather than dropped.
- **Tasks 6, 10 and 12 have no such prerequisite in the corpus at all** (an extraction miss, measured in
  `validation/step4-run-corpus.md`), so no detector could hit them; recall is reported over the tasks whose
  prerequisite is present.
- **Prompt truncation was real and rare.** A single call exceeded the 16k window on one large commit per
  development run, silently, until the owner's Ollama log showed it. The configuration now counts tokens as
  the provider charges them, caps the diff at half the window, and fails loudly if a provider truncates.
