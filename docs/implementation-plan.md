# rust_smysl — implementation plan (draft)

**Status:** draft, 2026-09-16; updated 2026-09-17 (smysl 1.3 pin, S1 result, smysl 1.4.0 edge lifecycle
and acceptance, S3 result, acceptance reframed, S4 result, pin to smysl 1.6.0). Written from the research experiments in this
repository's history; every "settled" item below cites the experiment that settled it.

> **Where the project stands (2026-09-17).**
> - **The tool's promise is that it operates as designed.** It makes no promise about how an agent or a
>   person acts on what it reports. Gates are functional acceptance, and a negative result is an outcome
>   that sets scope or documentation, not a verdict on the project (§4).
> - **Feasibility: met.** smysl works as a self-contained library under an installable cargo subcommand,
>   on real Rust history (§4, Feasibility).
> - **S3, done:** rationale packed into an agent's prompt did not change what the agent did (10
>   violations without, 12 with, over 14 pairs), and git history answered "why" questions as well as the
>   corpus in essay-style repositories. Kept as information about that usage pattern. It moves findings
>   out of the prompt and onto the change (§4, S3).
> - **S4, done (2026-09-19): `check` ships advisory.** On a free local model it finds 0.38 of the
>   contradictions on held-out data and flags 13% of ordinary commits, so it exits 0 with findings unless
>   `--strict` is given, and its documentation carries the measured figures (§4, S4). The deterministic
>   half — retrieval, packing, validation, determinism — holds; the judgement is model-bound, and a hosted
>   model reached 0.75 recall and ~0.89 precision on the same pipeline.
> - **Next:** Phase 1's remaining store work, then Phase 3 ports `check` into the shipped binary.

**What the tool is:** a cargo subcommand that records *why* Rust code changed — the decisions a
change makes, what had to be true for them (prerequisites), what was rejected, and what follows —
as a smysl corpus, and keeps that record honest over time: deterministic facts about the code and
test results as evidence, stale reasoning flagged when the code it rests on moves.

**What it promises:** that each command does what its design says, with measured figures where a model
is involved (recall, precision, cost). Acting on its reports belongs to whoever reads them: a person, a CI
step, a hook, an agent.

**What it is not:** a transcript store (Entire, Agent Note, git-ai cover capture), a static
analyzer (it reuses `syn`, cargo, rust-analyzer, cargo-mutants), or an oracle that marks a
prerequisite true on a model's say-so.

---

## 1. Dependency pin

| Dependency | Pin | Why |
|---|---|---|
| `smysl` | `1.5` (crates.io, published 2026-09-17) with `default-features = false, features = ["stage"]` | The model-free route (`stage::prepare_declared`, `quote_support_in`, `resolve_label`, `dependents_via`, `EdgeSet::premises`, `from_csv`, label bindings in staged records) first exists in 1.3. 1.4 added record-level merge idempotence (R10), imported readings that check clean (R12) and the edge lifecycle (D15); 1.5 adds R16–R20 and C8 packing, all of which `check` uses. |
| `syn` | `3` (`full`, `visit`, `extra-traits`, `printing`), `proc-macro2` with `span-locations` | Parses both test workspaces with no errors in ~0.3 s |
| `gix` | `0.87`, `default-features = false, features = ["sha1", "revision"]` | Commit message and changed-file text read in-process (`cargo-smysl-git`); no `git` binary |

**Pin rule:** always the latest published smysl release from crates.io, never a git dependency. A
git dependency would block `cargo publish` and `cargo install cargo-smysl`. Verified at `013cc20`
(pre-release): no `smysl-provider`, `ureq`, `tokio` or `rustls` in the `stage` tree.

**Pinned to smysl 1.5.0 on 2026-09-17,** the day it was published. `check` adopts all five of its
requests: `Query::within` (R16) restricts retrieval to decisions, prerequisites and rejected
alternatives; `Tokenizer::folding()` (R19) reaches a unit saying "require" from a diff saying
"required"; `units_with_source_prefix` (R20) replaces a full scan per diff; `labels_of` (R17) and
`support_span` (R18) are adopted as the corpus and validation are simplified; and
`PackRequest::resting_on` (C8) carries a decision's prerequisites, measured at 21% → 54%.

**MSRV:** the shipped crates build on 1.86. `cargo-smysl-eval` does not: its model client (`ureq`)
pulls an `icu`/`yoke` chain needing a newer rustc. It is development tooling and never published, so the
CI floor covers the four shipped crates.

**Previously pinned to smysl 1.4.0,** the day it was published. Acceptance from rust_smysl passed
twice before publish (`docs/smysl-1.4.0-acceptance.md`). On the published crate:
- the workspace tests pass with nothing ignored, including the R10 and R12 acceptance tests and the merge
  guarantee that was ignored under 1.3;
- clippy is clean, the workspace builds on Rust 1.86, and the dependency tree stays model-free;
- S3 packs regenerate byte-identically.

1.4.0 brings the edge lifecycle this plan uses from Phase 3 on (D15): relation identity, withdrawal
(record 11, `@withdraw`), attestations on edges, live rebuttals, and resolution (record 12, `@resolve`).

**Distribution:** `cargo-smysl` is an external cargo subcommand. It installs with
`cargo install cargo-smysl`, runs as `cargo smysl …`, and is self-contained: smysl and syn are linked
as libraries; no smysl CLI, scripts or other external tools are required at runtime. The only process
it starts is the `cargo` that invoked it (`$CARGO`).

---

## 2. Settled decisions

| # | Decision | Evidence |
|---|---|---|
| D1 | smysl is used as a **library**, never through its CLI | project decision |
| D2 | **Unit kinds:** `decision`; `constraint` (prerequisite); `claim` (rejected alternative, anticipated consequence); `finding` + `evidence` (verified consequence); `artifact-ref` (code anchor); `data` (measured readings) | extraction runs v1, v2, pro |
| D3 | **Prerequisites link by `conditions`, not `grounds`.** Grounds are hashed into the decision's uid, so a reworded prerequisite would move every decision and consequence built on it. "What breaks" uses `dependents_via` with `EdgeSet::premises()` | stability experiment: 12–24 of ~57 uids shared across three runs; R6 |
| D4 | **Edges:** `contrasts` (alternative), `causes` (consequence), `backs` / `rebuts` (evidence), `x.code/touches` (decision → anchor), `x.code/exercises` (test → prerequisite), declared with `@schema x.code/v1`. Granularity profile `fine` | R4, R6, `@schema` verification |
| D5 | **The tool assigns sources, labels and statuses;** a model proposes content only. Sources are `git:<sha>` or `path@<sha>`. Labels carry commit and run: `d/g<sha>-<n>`, `p/g<sha>-<d>-<n>` | generic ingest wrote `ref: CHANGELOG.md` for a commit message; label collisions across runs |
| D6 | **Extraction is multi-pass with the diff:** decisions first, then prerequisites / alternatives / consequences in parallel. A pro-class model for prerequisites at minimum | flash-lite ~30% genuine prerequisites vs pro ~75% |
| D7 | **Extract once per commit;** improvements arrive as `supersedes`, never as re-extraction merged into the store | stability experiment |
| D8 | **Quotes are checked against the message and diff** with `quote_support_in`; an absent quote caps the unit at `speculative` | 0 of 138 absent quotes with pro; fabricated alternatives caught with flash-lite |
| D9 | **Facts are deterministic and live in a regenerable cache, not the corpus.** Scope: items the diff touches, code identifiers named in the message or prerequisite, one call hop outward, reverse "called by" edges, manifest / CI-matrix / git facts | ~7 MB of facts per commit; rounds 1–6 |
| D10 | **Author text is prose and never verifies:** doc comments (current and parent), string literals, string-valued consts, commit subjects | rounds 3–6: every one of these laundered support until tagged |
| D11 | **Fact matching retrieves per prerequisite** (BM25 plus neighbours plus named items, 30 structural + 10 prose facts), and the model judges one claim at a time | round 5: 16 correct, 0 false contradictions |
| D12 | **Verdict policy:** a single run never raises a status. `SUPPORTED` requires model coverage "full" **and** every part covered by a structural fact, **and** two independent runs agreeing (or a person). `PARTIAL` / `PROSE_ONLY` / `IMPLEMENTED_BY` attach evidence only; `CONTRADICTED` goes to review. Normative prerequisites can reach `IMPLEMENTED_BY` at most. With smysl 1.4.0, the outcomes of review are records (D15), never a status rewritten in place | round 6: part-based coverage alone overclaimed 4 times; two-run agreement had 1 wrong of 7 |
| D13 | **Test evidence:** link tests to prerequisites (verifies / exercises / unrelated), run each at the commit with `--locked`, the features its `cfg` requires, and `--exact`; import results with `from_csv` as `measured` `data` units; `backs` / `rebuts` for verifies, `x.code/exercises` for exercises. Prerequisite statuses never change from test results alone. Every link edge carries an attestation naming who asserted it (the linking model, or a person), and a `backs` edge is pending until a person confirms it (D15) | test-evidence run: 53 tests, 50 passed, attested stores; S1: 3 of 17 "verifies" links wrong, 1 vacuous test |
| D14 | **Candidate tests are deterministic** (BM25 over test name, calls, assertions and doc; bonus for touched files and claim-named items); the model only classifies | test-linking run: 17 verifies / 42 exercises across 60 prerequisites |
| D16 | **The model is the operator's choice, never the tool's.** Provider (`ollama`, any OpenAI-compatible endpoint), model, endpoint, key variable, context window and the system prompt come from flags, the environment or configuration, with sensible defaults and the prompt tunable per provider; a run records which it used. The default is local (Ollama), so the tool costs nothing to try | S4: the detector runs on a local 7B model and on a hosted one, with the same code |
| D18 | **The model client is our own, and the default build has no network stack.** `check` speaks plain HTTP/1.1 to a local provider (Ollama, or any OpenAI-compatible endpoint on `http://`), written out rather than pulled in; a hosted provider over TLS is the `hosted` feature, off by default. This settles item 7: smysl's `model` feature would bring `smysl-provider` and a TLS stack into every install, and the multi-pass schemas do not fit `PromptOverride` anyway. `check` ships **advisory** — findings print, the exit code stays 0 unless `--strict`, which exits 5 | S4: measured recall 0.38 and 13% of ordinary commits flagged on a local 14B; the self-contained objective; the CI rule that keeps `ureq`, `rustls`, `tokio` and `smysl-provider` out of the default tree |
| D17 | **The prompt is fitted to the provider, and a split is reported.** `check` sizes each request to what the model takes (context limit less the room the answer needs), splits the units it judges only when they do not fit, and says so — a model judging six units at a time sees less than one judging sixty, and a user who is not told cannot read the result. The same warning covers a truncated diff | S4: a 7B local model at 16k answered badly on ~50 units and usefully on 6; the shipped command must make that visible |
| D15 | **Review is recorded with smysl 1.4.0's edge lifecycle, never by deleting or rewriting.** Each edge has an identity (rid). **Confirm:** a person's attestation on the edge (`human:<name>`). **Reject:** a `Withdrawal`, whose reason unit says why; the edge stays in the log and is no longer followed, packed or counted. **Close a disagreement:** a `Resolution` naming the contention or the unthreaded `rebuts` edge, with a note unit; it records that review happened and decides nothing. **Queue:** open contentions, unresolved `rebuts` edges, and `backs` / `x.code/exercises` edges with no person's attestation and no withdrawal. Two-run agreement (D12) counts attestations from independent runs on the same rid wherever both endpoints are stable units (test readings, decisions, anchors); for model-worded prerequisites, whose uids differ between runs, agreement stays at the verdict level | smysl dev/1.4.0 `09271ab` (spec draft 1.4); S1 outcome; stability experiment |

---

## 3. Architecture

```
cargo smysl <command>
│
├── facts      syn 3 extractor → per-commit fact cache (JSON, regenerable)
│              scope selection, fact templates, prose tagging, CI-matrix cfg evaluation
├── extract    model client + multi-pass prompts (decisions → prerequisites/alternatives/consequences)
│              quote check (smysl::quote_support_in), extract-once cache keyed by commit + recipe
├── corpus     unit construction, labels, @schema x.code/v1, stage::prepare_declared, store I/O,
│              resolve_label, dependents_via(EdgeSet::premises())
├── verdict    per-prerequisite retrieval, matching, D12 policy, review queue
├── evidence   test linking, cargo test runner, from_csv import, edges, mutation gate (Spike 1)
└── cli        cargo subcommand surface
```

Each module is a separate crate in one workspace, so `facts` and `corpus` stay model-free, the same
split smysl keeps between `stage` and `model`.

---

## 4. Phase 0 — spikes and acceptance gates

**What a gate measures.** The tool promises to operate as designed, so a gate is functional acceptance:
does a command do what its design says, measured where a model is involved. A negative result is an
outcome: it sets how a feature ships (gate or advisory), what its documentation states, or which step is
reworked. Whether people or agents act on a report is not the tool's promise. Measuring it (S3, S4 §5) is
information for design.

**Order (updated 2026-09-17):**
1. Feasibility: met.
2. S3: done, as information.
3. **S4: functional acceptance of `check`.**
4. S2 and S0: after S4, as tuning instruments.

### Feasibility — can smysl track a Rust codebase? (met)

Checked on smysl 1.3.0, and again on 1.4.0 after the pin.

| Needed | Evidence |
|---|---|
| smysl as a self-contained library | `stage` feature only; no smysl CLI, provider layer or network stack in the tree; git read in-process with gix; CI checks it |
| An installable cargo subcommand | `cargo install` then `cargo smysl doctor` on Linux, macOS and Windows; MSRV 1.86 |
| Real history into a corpus | 726 units from 20 commits in 3 repositories, 0 staging errors. Each quote is checked against the real commit text; 2 absent quotes were capped at speculative |
| Identity and merge guarantees | surface and CBOR round trips exact; merge order-independent; record-level idempotence with 1.4.0 (R10) |
| Deterministic retrieval and packing | S3 contexts byte-identical across two builds and two smysl versions |
| Rust-aware facts | `syn` function facts; test linking and mutation gating measured (S1) |
| Upstream fit | R1–R10, R12 and R15 closed; R11, R13 and R14 kept open by smysl as S2 tasks |

Known weak spots, all in extraction rather than smysl:
- prerequisite recall depends on the model (DeepSeek missed two that the pro model found);
- runs are unstable;
- fact matching plateaued at 13–16 of 67;
- terse commit messages give little to extract.

### S0 — labelled evaluation set (deferred: tuning after S4)

- **Do:** label 20 commits across at least 3 repositories (smysl, ucal, and at least one with
  ordinary commit messages): decisions, genuine prerequisites, rejected alternatives. Labelled by
  the repository owner. Record definitions for the boundary cases the models kept confusing
  (motivation vs prerequisite, rationale vs prerequisite, normative vs factual).
- **Output:** `eval/` with the labels and a scorer (precision/recall per kind).
- **Gate:** none; it is the instrument for tuning (model choice, prompt changes, Phase 2's "done").
- **State:** kit built (`eval/`, `cargo-smysl-eval`); extractions of all 20 commits by
  `research-deepseek-v2` and `research-flash-v2`, 6 by `research-pro-v2`, all staging with 0 errors
  against real commit text. Labelling starts after S4, with the 6 studied commits.

### S1 — mutation gating of test evidence (done)

- **Question:** does requiring a linked test to catch a mutant *by an assertion in the test* separate
  sound `backs` edges from wrong links and vacuous tests, at a cost that fits CI?
- **Result (2026-09-17):** 16 edges gave KEEP 7, DROP 4, WEAK 2, UNASSESSED 3. Against the hand grades:
  - **What it filtered:**
    - the **vacuous test was not kept** (WEAK: two panic kills, nine mutants missed);
    - two of three wrong links were not kept.
  - **What it cost:** only **2 of 5 valid links were kept**:
    - `each_tier_is_3125_of_the_one_below` catches mutants only by panicking in library code;
    - `every_command_dispatches` misses all 23 `cli` mutants;
    - one target was mis-chosen.
  - **Blind spots:**
    - a wrong link whose target comes from the test's own callees is KEEP (`worse`), because mutation
      measures a test's sensitivity, not whether the link is right;
    - `main` was sampled at 40 of about 240 mutants, and the sample may have skipped the match-arm deletions
      the dispatch test is built to catch.
- **Outcome against the gate:** it does not keep most valid edges, so mutation gating is **opt-in**,
  `backs` edges require review, and the static vacuity check ships in v1. The self-contained objective
  also rules out depending on an external cargo-mutants binary, so an opt-in gate needs an in-process
  implementation or stays a documented external step.
- **Measure:** per edge KEEP / WEAK / DROP / UNASSESSED against the hand grades (5 valid, 6 partial,
  1 counterfactual, 3 wrong links, 1 vacuous test); minutes per edge.
- **Gate:**
  - mutation gating becomes a **v1 requirement** for `backs` if it keeps most valid edges and removes
    the vacuous test, with a median under 10 minutes per edge;
  - otherwise it ships as an **opt-in** check, and `backs` edges require review;
  - either way, add the cheap static vacuity check (an `assert_eq!` whose two sides are the same
    tokens).

### S2 — recorder-first or extractor-first (after S4)

- **Question:** do prerequisites recorded **while the change is made** (by the coding agent or the
  author) beat prerequisites extracted afterwards from message and diff?
- **Do:** implement 10 small changes across 2 repositories twice. Arm A: the agent emits decision /
  prerequisite / alternative units as it works (a structured-output instruction plus a stop hook
  that writes them). Arm B: commit normally, run the D6 pipeline on the commit. Score both against S0
  definitions, judged blind by the repository owner.
- **Gate:**
  - **Recorder-first** if arm A's genuine-prerequisite precision is at least arm B's and recall is
    clearly higher. The capture path (hooks, agent integration) moves into Phase 4, and extraction
    becomes backfill.
  - **Extractor-first** otherwise. Input sources for ordinary repositories move into Phase 4.
- **Measure (updated after S3):** by `check` (S4). A later change that contradicts each arm's recorded
  prerequisites is checked against each arm's corpus, and the arm whose corpus lets `check` find more
  contradictions, at equal precision, wins. Label scoring is added only if S0 is labelled by then.

### S3 — does packed rationale change what an agent does? (done: information)

Originally the project's go/no-go gate. After the result, and with the tool's promise stated as operating
as designed (above), it is recorded as information about one usage pattern: rationale in the prompt.

- **Question:** does packed rationale stop an agent from breaking a recorded prerequisite?
- **Protocol:** `eval/s3-protocol.md`. 12 tasks validated: 8 non-control, 4 controls. The corpus is
  the `research-deepseek-v2` extraction of the repository's S0 commits, built and staged with
  `cargo-smysl-corpus` against real commit text, which is the product as it would ship, extraction
  errors included.
- **Do:** 12 tasks on smysl and ucal, each of which would violate a prerequisite already in the corpus
  (e.g. make `cli()` register commands conditionally; move `.smysl/staged.smy` resolution off the
  working directory; derive the dispatch test's command list from the binary). Run each with and
  without the relevant units packed (`smysl::pack` within a fixed budget) into the agent's context;
  count violations that reach a passing commit. Also score 10 "why is this like this?" questions
  answered from the corpus vs from `git log` / `blame`.
- **Gate as set then:**
  - **Go** if violations drop by at least half and corpus answers win the majority of questions.
  - **No-go** otherwise.

#### Run1 result (2026-09-17): questions lost (0 of 10); tasks invalid, narrowed to run2

Results: `eval/s3/results/run1/` (`report.txt`, `judging.md`).

- **Questions: lost, against the gate as set then.** Corpus answers won 0 of 10. Both sources were correct on all 10, and git
  history plus code was more useful on all 10. In essay-style repositories the commit messages already
  hold the reasons, so the corpus is a shorter summary of the same text. It keeps the reason and drops
  the detail (files, tests, the case ruled out) that made the git answers more useful. Caveats: a Claude
  session judged, having read the commits first, and the git answers were longer. Neither is likely to
  move 0 toward 6. **Consequence: answering "why is this like this?" is dropped as a product claim.**
- **Tasks: invalid.** Non-control violations were 6 without the corpus and 4 with it, one run per task
  and arm. Two controls were violated because agents rewrote the guarding test (task 8: renamed and
  re-asserted `a_forward_step_is_accepted`; task 10: replaced the refusal test). "Test-guarded" does not
  protect a prerequisite from an agent.
- **The one clear effect was push, not pull.** On task 7 the corpus agent quoted the recorded rejected
  alternative and stopped to ask. The agent without the corpus had the same git history and never looked.
  A prerequisite in context did not by itself stop a violation (task 2).
- **Narrowed hypothesis:** at edit time, declines and prerequisites that apply to the code being changed,
  pushed into the agent's context, reduce violations. That is item 10's "pack before edit" hook, not a
  query tool.

#### Run2 (narrowed task measure)

Protocol amendments: `eval/s3-protocol.md`, "Run2".

- **Violation:** the oracle detects the prerequisite broken in the final state, whether or not the
  agent edited tests to make them pass. Rewriting a guard test is the failure being measured, so there
  are no control tasks. The harness check is the base (every oracle intact) and the naive patches (every
  oracle violated).
- **Stopped:** the agent made no change and asked. Counted separately, and not as a violation.
- **Repetitions:** 3 per task and arm (72 runs), same agent configuration as run1.
- **Gate:** go for the narrowed product if corpus-arm violations are at most half of control-arm
  violations over all 36 pairs (as set then).
- **If go:** item 10's edit-time hook (pack before edit) moves ahead of everything in §8 and becomes
  the v1. The query commands (`why`) become secondary.

#### Run2 result (2026-09-17): no effect

Stopped by the owner after the first repetition (and task 1 and 2's second), once the gate could no
longer be met: the corpus arm had 11 violations, so a go needed at most 5 more in its remaining 23 runs
while it was violating in most. Three runs interrupted by the stop were discarded unscored. Results:
`eval/s3/results/run2/` (`report.txt`), about $49.

| | Without corpus | With corpus |
|---|---|---|
| Runs (14 complete pairs) | 14 | 14 |
| Violated | 10 | **12** |
| Stopped (no change) | 1 | 1 |
| Intact | 3 | 1 |
| Tests passing at the end | 14 | 14 |
| Mean diff / turns / total cost | 162 lines / 58 / $21.08 | 183 lines / 61 / $27.51 |

- **Paired:** in 10 pairs both arms violated. The corpus prevented a violation in **none**. It had one
  where the control did not in 2, tasks 10 and 12, the two whose prerequisite is not in the packed
  context.
- **Run1's task 7 effect did not repeat.** With the rejected alternative in context, the agent anchored
  the clock at program start as asked.
- **"Stopped" was not a refusal.** Task 4's corpus run made no change because an existing test already
  covered the request.
- **Agents did not engage with the context.** 1 of 14 corpus-arm summaries mentions the corpus at all.
  Task 1's agent used the recorded rationale (the guard never fires on user input) as its argument for
  removing the guard.
- **Every violating run left the tests green.** The prerequisites at risk are exactly the ones the tests
  do not state, which is the gap the tool was meant to fill. A packed rationale in the prompt did not fill
  it.

**Across S3:**
- **Questions:** git history plus code answered as well as the corpus and more usefully (0 of 10).
- **Tasks:** an agent given a task that conflicts with a recorded prerequisite does the task, with or
  without the prerequisite in its context.

In these repositories and with this usage, the corpus added nothing measurable over what the repository
already holds.

**Limits of the result.**
- The repositories have essay-style commit messages, so git history is a strong baseline. A repository
  with terse messages was not tested, but there extraction has little to extract (S0's clap commits).
- One agent configuration (Sonnet through Claude Code, a neutral prompt) was tested. An instruction to
  obey the recorded prerequisites was deliberately not given; with it, the measure would be instruction
  following rather than the corpus.
- The tasks ask for the violating change directly. A gentler task, where breaking the prerequisite is a
  side effect, might show a difference that these do not.

**What it changed in the design:**
- **Findings come from the change, not the prompt.** `check` reads the diff after it is made and reports
  what it contradicts, as a signal with an exit code (S4).
- **"Why" answers are not a claim over git history** in repositories whose messages already give reasons.
  `why` stays a store query, not a replacement for `git log`.
- **Guard tests do not protect a prerequisite from an agent,** which rewrote them twice (run1, tasks 8 and
  10). `check` must read test edits in the diff like any other change.

### S4 — does `cargo smysl check` operate as designed?

- **Protocol:** `eval/s4-protocol.md`.
- **Model policy (D16):** the gate is measured on a named configuration, and the local one is measured
  first. Paid models are used only after a local run shows the idea works.
- **Design under test:** given a diff and the corpus of the diff's ancestors, report the decisions,
  prerequisites and rejected alternatives the change contradicts, and nothing else.
  1. Candidates are deterministic: units anchored to the touched files, plus BM25 over the diff's
     identifiers.
  2. Candidates are packed with their closure.
  3. One model judgement per diff.
  4. Each verdict's label and diff quote are validated.
  5. Exit 5 when there are findings.
- **Data:**
  - **Development set (tuning allowed):** the S3 naive patches, the 52 S3 agent diffs with oracle labels,
    and half of about 118 real commits.
  - **Held-out set (detector frozen first):** 24 new agent diffs, the other half of the real commits, and
    10 commits after the S3 bases.
- **Gate (held-out):** recall ≥ 0.70 over tasks whose prerequisite is in the corpus; precision ≥ 0.80 of
  owner-adjudicated flags; ≤ 10% of clean real commits wrongly flagged; deterministic candidates and pack.
- **Outcome either way:**
  - all bounds met: `check` ships as a gate;
  - any bound missed: it ships advisory (`--strict` to block), with the measured figures documented and at
    most one step reworked against a fresh held-out set.
- **Information, not gated:** a third S3 arm with `check` as a Stop hook (silent vs disclosed violations).

#### S4 result (2026-09-19): advisory

Full write-up and caveats: `eval/s4-protocol.md`, "Result". Frozen configuration: `eval/s4/frozen.txt`.

| Measure | Development | Held-out | Bound |
|---|---|---|---|
| Recall by pattern, agreed over two passes | 0.58–0.65 | **0.38** | ≥ 0.70 |
| Real commits flagged | 0 of 11 | **9 of 69** | ≤ 10% wrongly |
| Precision | 0.29 (owner-adjudicated) | pending | ≥ 0.80 |

- **`check` ships advisory:** exit 0 with findings unless `--strict`, with these figures in its help. Two
  bounds are missed before precision is counted, so the adjudication changes how useful it is, not the mode.
- **What works:** the deterministic steps. Candidates, packing, label and quote validation, determinism; on
  the held-out set validation dropped 33 verdicts whose label or quote did not check out.
- **What does not:** the judgement, on a 7B or 14B local model. The same pipeline on `deepseek-v4-pro` gave
  0.75 recall and ~0.89 precision at about $2 a pass — the difference is the model, not the retrieval.
- **Levers measured:** two-pass agreement halves flags and costs some recall (kept); a term-weight filter
  does not separate correct from wrong flags (rejected, kept as a setting defaulting to off); a stricter
  prompt lost more correct flags than wrong ones (reverted).
- **Consequence for the plan:** `check` is worth shipping as a reviewer's aid and a `--strict` gate for
  those who choose a stronger model. It is not the automatic guard the S3 narrowing hoped for.

---

## 5. Phase 1 — smysl integration and data model

**Done (2026-09-19).** The corpus crate builds and stages units (D2, D5), declares `x.code/v1`, writes
`.smysl/commits/<sha12>.smy` per commit and merges into `.smysl/store.cbor`, and answers what rests on a
unit. `cargo smysl why <label>` is the first command past `doctor` that does its job.

- **The documents are the record; the store is derived.** `Corpus::rebuild` reconstructs the store from the
  surface documents, because surface text outlives a reader older than the writer and a store file does not.
- **Recording is idempotent:** the second recording of a commit adds nothing (rule U, held by a test).
- **`why` refuses to be silent:** an unbound or malformed label, or a missing corpus, is an error, because
  an empty answer would read as "nothing depends on this".
- **Both "done when" criteria hold**, on every research extraction of every repository: stores check clean
  under `fine`, and "what depends on prerequisite X" returns the decision it conditions and what that
  decision causes (`crates/cargo-smysl-corpus/tests/store.rs`).

- `corpus` crate:
  - build units per D2 with tool-assigned sources and labels (D5);
  - declare `x.code/v1`;
  - stage with `prepare_declared`;
  - write per-commit surface documents and a merged CBOR store.
- Store queries: `resolve_label` (refusing ambiguity), `dependents_via(EdgeSet::premises())`, trace.
- Round-trip tests: a hand-written change-rationale document parses, checks clean under `fine`,
  merges idempotently, and keeps uids stable when a prerequisite is reworded (D3).
- **Done when:**
  - the pro-extraction outputs from the research (`pro-*.json`, `ucal-*.json`) convert into stores that
    check with 0 errors;
  - "what depends on prerequisite X" returns the decision and its consequences.

## 6. Phase 2 — facts and extraction

- `facts` crate (**done, 2026-09-19**): the research extractor is ported and typed — per-function events
  (calls, methods, field reads, bindings, macros, string literals) each carrying the control context it
  sits in, plus doc comments, consts with their array elements, struct fields, and `cfg` from the item,
  its enclosing module and the file. Prose is tagged (D10): doc text, string literals and string-valued
  constants are extracted and marked, so no verdict can rest on one. `cargo smysl facts [rev]` runs it
  over a commit or the working tree; the cache is keyed by each file's bytes and `EXTRACTOR_VERSION`, so
  a changed file cannot hit a stale entry and deleting it costs only a reparse.
  - Still open here: scope selection and fact templates per D9, and CI-matrix `cfg` evaluation.
- `extract` crate (**done, 2026-09-19**): decisions first, then each decision's prerequisites,
  alternatives and consequences asked for on its own (D6, which the research measured: asked all at once
  a model returns thin prerequisites). The model proposes content only — labels, sources and statuses are
  assigned by the corpus from the commit the tool read (D5), and every quote is checked there (D8). The
  extraction is kept per commit and recipe (D7); an improvement is a new recipe, never a re-extraction
  merged into the old one. `cargo smysl extract [rev]` runs it and records the result, `--dry-run` stops
  before recording, `--force` is a deliberate redo.
  - The model client lives here (D18) and `check` uses it: the `Judge` trait's primitive is the model's
    text, so `extract` reads its own JSON and `check` reads verdicts, and both agree on what a malformed
    answer is.
  - **Its quality is unmeasured.** On this repository's own commit a local 14B returned one decision with
    one prerequisite; the research's pro-class runs returned eight. Measuring it is the S0 labelling work,
    still deferred.
- **Item 7, settled (D18):** our own client. The default build carries no network stack; `hosted` adds TLS
  for a paid provider. `extract` will use the same client.
- **Done when:** on the S0 set, prerequisite precision is at least the research pro run's (~75%
  genuine), with a measured recall figure; facts regenerate byte-identically.

## 7. Phase 3 — verdicts and test evidence

- `verdict` crate:
  - per-prerequisite retrieval (D11);
  - matching with parts, coverage and normative flag;
  - the D12 policy including two-run agreement;
  - a review queue built from the store (D15): open contentions, unresolved `rebuts` edges, and
    `backs` / `x.code/exercises` edges with no person's attestation and no withdrawal;
  - `cargo smysl review` writes the outcome as records: an attestation to confirm, a `Withdrawal` to
    reject, a `Resolution` to close a disagreement, each with a unit saying why.
- `evidence` crate:
  - test candidates (D14) and linking;
  - the cargo test runner (`--locked`, `cfg` features, bin-only crates, ignored tests recorded as
    readings);
  - `from_csv` import;
  - edges per D13, each attested by who asserted it (the linking model's agent id and recipe);
  - the mutation gate as S1 decided;
  - the static vacuity check.
- `check` (done, 2026-09-19): the S4 detector is `cargo smysl check` in `cargo-smysl-verdict`, advisory
  with `--strict` to block, its measured figures in its help, and provider, model, endpoint, window,
  token cost and prompt as settings (D16, D17, D18). It reads a commit (`rev`), a patch (`--patch`) or
  stdin, builds its own unified diff with `imara-diff`, and its deterministic half is tested against a
  scripted judge, with no model and no network.
- **Done when:**
  - `check` reproduces its S4 held-out figures from the shipped binary;
  - on the S0 set, no single-run status raise;
  - wrong `backs` edges are withdrawn (by review or the mutation gate) or pending in the queue, never
    followed by packing or `why`;
  - every reading traces to `tool:smysl-import`.

---

## 8. Later phases — order set by the spikes

`check` is not in this table: S4 decides how it ships, and it is built in Phase 3. The staleness report
(item 5) and the PR and hook integration of `check` (item 10) come first after Phase 3, whichever way S2
goes.

| Item | Work | If recorder-first (S2) | If extractor-first (S2) |
|---|---|---|---|
| 10 | **UX and integration:** `cargo smysl record / why / check / stale / review`, Claude Code hooks (pack before edit, record at stop), git hook or merge driver, PR rendering | **Phase 4** | Phase 6 |
| 4 | **Input sources for ordinary repositories:** PR descriptions, review threads, agent transcripts (Entire checkpoints, Agent Note), diff-only extraction | Phase 6 | **Phase 4** |
| 5 | **Staleness:** item-hash invalidation of facts, cascaded through `dependents_via` to decisions; an `x.code/touches` edge whose anchor moved is withdrawn with a reason naming the commit, not rewritten; "stale" report per commit range | Phase 5 | Phase 5 |
| 6 | **Storage:** `.smysl/` layout, surface vs CBOR in git, merge driver using smysl merge, growth over hundreds of commits, compaction | Phase 5 | Phase 5 |
| 8 | **Cost and privacy:** per-commit token budget and price; model routing (small model for decisions/alternatives, pro for prerequisites); quota handling; local model viability | Phase 6 | **Phase 4** |
| 9 | **Evaluation:** grow S0 into a regression suite run on every prompt or model change | continuous | continuous |



---

## 9. Risks and known limits

- **Extraction wording errors propagate:** "version 2.2" (a READINESS gate step) was contradicted
  by a crate version. Matching must always see the verbatim quote.
- **A summary can contradict its own quote;** quote checking cannot catch it (documented in smysl
  1.3).
- **Lexical retrieval misses facts** that share no words with the claim (guard conditions,
  `.required(true)` vs "require an argument"). Candidate fix: smysl's semantic retrieval.
- **Linked tests can be wrong or vacuous** (3 wrong links and 1 vacuous test of 17 "verifies").
  S1 addresses sensitivity; link validity still needs review.
- **Name-based resolution** in `syn` facts is reliable within one crate and one or two hops; beyond
  that it needs a rust-analyzer index.
- **Model quotas:** the pro model's 250 requests/day was exhausted by one research day. The design
  must batch and cache.
- **The research repositories share one author** and an essay style of commit message; results on
  ordinary repositories are unmeasured until S0/S2.
- **Agents do not act on context they are given** (S3: 1 of 14 corpus-arm summaries mentioned it) and
  rewrite tests that stand in their way (S3 run1). A report is only acted on where something reads its
  exit code.
- **A model-judged check has a false-flag rate.** Measured: 13% of ordinary commits flagged on held-out
  data with a local 14B, which is why `check` is advisory. A stronger model roughly halves the flags and
  triples the precision, at a price per run.
- **A provider can truncate a prompt silently.** Found only in the owner's Ollama log. The tool now counts
  tokens as the provider charges them, caps the diff at half the window, records predicted against charged,
  and fails rather than answer from a truncated prompt.

## 10. smysl requests (not blocking)

**1.7.0** (`docs/smysl-requests-1.7.md`): R24, export `ExternalCost` and `CostModel` from the facade so
1.6's `counting_with` can be called by a consumer that depends on `smysl` alone. Found when a pack sized
in smysl's bytes/4 units came to 9 400 model tokens and the provider truncated the prompt unseen.

**1.6.0** (`docs/smysl-requests-1.6.md`): R21 a pack that fits the caller's whole prompt budget (D17's
fitting lives outside smysl today), R22 which query terms a hit matched (task 5's miss was invisible),
R23 retrieval filtered by an extension schema's own kind (`code:kind`, which `KernelType` cannot express).

**1.5.0** (`docs/smysl-requests-1.5.md`), sent before the cut, **all five implemented and verified**: R16 retrieval restricted to a candidate set,
R17 a unit's labels without a scan, R18 where a quote matched, R19 optional suffix folding in the
tokenizer, R20 units by source prefix. All five come from building `check` (S4).

**1.5.0 features this plan adopts when it ships:** `PackRequest::resting_on` (C8) — a packed decision now
carries the prerequisite it rests on, which D3's `conditions` edges kept outside its uid; `rests_on` and
`trace_via` for `why`; `review_with(... confirming [Backs, …])` for D15's queue; `prepare_attested` for
D13's model-asserted edges; `attestations_of` and `agreement` for D12's two-run agreement.

### 1.4.0 (shipped)

New smysl features land on the 1.4.0 development branch. cargo-smysl stays pinned to the latest
published release, so a feature it needs becomes a 1.4.0 request and is adopted once 1.4.0 is on
crates.io. It never uses a git dependency.

The requests, with reproductions and acceptance tests, are in
[`docs/smysl-requests-1.4.md`](smysl-requests-1.4.md):

- **R10 (high):** merge re-appends label bindings and schema declarations (`Store::contains` has no
  arm for either), so the log grows on every merge. Found by the Phase 1 merge guarantees; reproduced
  on 1.3.0 and dev/1.4.0.
- **R11–R15:** `import` ignores `--format`; `import` summaries fail `check`; configuration error exit
  code; unknown provider kind message; summary bound mismatch.
- **Status (dev/1.4.0, 2026-09-17):** R10 and R12 fixed and accepted; R15 fixed; R11, R13 and R14 kept
  open by smysl as S2 tasks.
- **Not filed:** `--granularity` does not choose the profile units are checked under; smysl deferred
  it to 1.4 itself.

## 11. Open questions for the owner

1. Name of the tool and subcommand (`cargo smysl`, `cargo why`, other).
2. Who labels S0, and which third repository with ordinary commit messages?
3. Budget per commit for model calls, and whether proprietary code may go to a hosted model.
4. Is the corpus committed to the analysed repository, or kept beside it?
5. **S4 adjudication:** the owner judges every flag blind (correct, wrong, arguable). Confirm the
   hit patterns in `eval/s4-protocol.md` §3 before the detector is frozen.
6. **Model for `check`:** DeepSeek (the extraction model) for S4; the shipped choice follows item 7 and
   item 8.
