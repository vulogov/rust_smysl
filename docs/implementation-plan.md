# rust_smysl — implementation plan (draft)

**Status:** draft, 2026-09-16; updated 2026-09-17 (smysl 1.3 pin, S1 result, smysl 1.4.0 edge lifecycle, S3 run1: questions no-go, tasks narrowed to run2; **S3 no-go**). Written from the research experiments in this repository's history;
every "settled" item below cites the experiment that settled it.

> **S3 decided no-go (2026-09-17).** Packed rationale did not reduce violations: over 14 paired runs, 10
> with the corpus and 12 without (§4, S3). By the gate, the plan below does not proceed as written: no
> Phase 1–3 build-out, no later phases, and S0 and S2 are cancelled. What exists is kept as it is: the
> cargo subcommand scaffold, `cargo-smysl-corpus`, `cargo-smysl-git`, `cargo-smysl-facts`, the eval
> tooling and the results. §11 lists the owner's options.

**What the tool is:** a cargo subcommand that records *why* Rust code changed — the decisions a
change makes, what had to be true for them (prerequisites), what was rejected, and what follows —
as a smysl corpus, and keeps that record honest over time: deterministic facts about the code and
test results as evidence, stale reasoning flagged when the code it rests on moves.

**What it is not:** a transcript store (Entire, Agent Note, git-ai cover capture), a static
analyzer (it reuses `syn`, cargo, rust-analyzer, cargo-mutants), or an oracle that marks a
prerequisite true on a model's say-so.

---

## 1. Dependency pin

| Dependency | Pin | Why |
|---|---|---|
| `smysl` | `1.3` (crates.io, published 2026-09-17) with `default-features = false, features = ["stage"]` | The model-free route (`stage::prepare_declared`, `quote_support_in`, `resolve_label`, `dependents_via`, `EdgeSet::premises`, `from_csv`, label bindings in staged records) first exists in 1.3; 1.2 does not have it. |
| `syn` | `3` (`full`, `visit`, `extra-traits`, `printing`), `proc-macro2` with `span-locations` | Parses both test workspaces with no errors in ~0.3 s |
| `gix` | `0.87`, `default-features = false, features = ["sha1", "revision"]` | Commit message and changed-file text read in-process (`cargo-smysl-git`); no `git` binary |

**Pin rule:** always the latest published smysl release from crates.io, never a git dependency. A
git dependency would block `cargo publish` and `cargo install cargo-smysl`. Verified at `013cc20`
(pre-release): no `smysl-provider`, `ureq`, `tokio` or `rustls` in the `stage` tree.

**Next pin: smysl 1.4.0,** published once its tests are green. rust_smysl's whole workspace already
passes against dev/1.4.0 (`09271ab`). 1.4.0 brings the edge lifecycle this plan uses from Phase 3 on
(D15): relation identity, withdrawal (record 11), attestations on edges, live rebuttals, and resolution
(record 12). Until 1.4.0 adds surface syntax for records 11 and 12, stores holding them are CBOR.

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

## 4. Phase 0 — spikes with decision gates

**Result (2026-09-17): S3 is no-go, so S2 and S0 do not run.** Order before that result: **S3 first, then S2, then S0 only as far as tuning needs.** S3 measures
whether the product is useful with automatic oracles and needs no labels: the owner only judges ten
answers blind. S0's labels measure correctness per kind, which tunes extraction but does not decide
go/no-go, so they are deferred until S3 says go. The labels are a development test set, never part
of the product.

The spikes decide scope and order. None of Phases 1–3 is wasted by any outcome, but the order of
the later phases, and whether there is a v1 at all, depends on these gates.

### S0 — labelled evaluation set (deferred: tuning after S3 says go)

- **Do:** label 20 commits across at least 3 repositories (smysl, ucal, and at least one with
  ordinary commit messages): decisions, genuine prerequisites, rejected alternatives. Labelled by
  the repository owner. Record definitions for the boundary cases the models kept confusing
  (motivation vs prerequisite, rationale vs prerequisite, normative vs factual).
- **Output:** `eval/` with the labels and a scorer (precision/recall per kind).
- **Gate:** none; it is the instrument for tuning (model choice, prompt changes, Phase 2's "done").
- **State:** kit built (`eval/`, `cargo-smysl-eval`); extractions of all 20 commits by
  `research-deepseek-v2` and `research-flash-v2`, 6 by `research-pro-v2`, all staging with 0 errors
  against real commit text. Labelling starts only if S3 says go, and then with the 6 studied commits.

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

### S2 — recorder-first or extractor-first

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
- **Measure (updated):** by downstream effect rather than S0 labels. Each arm's rationale is packed
  into S3-style tasks on the changed code, and the arm whose context prevents more violations wins.
  Label scoring is added only if S0 is labelled by then.

### S3 — go / no-go: does the corpus change outcomes?

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
- **Gate:**
  - **Go** if violations drop by at least half and corpus answers win the majority of questions.
  - **No-go** otherwise: stop, or narrow to the one thing that did help.

#### Run1 result (2026-09-17): questions no-go; tasks invalid, narrowed to run2

Results: `eval/s3/results/run1/` (`report.txt`, `judging.md`).

- **Questions: no-go.** Corpus answers won 0 of 10. Both sources were correct on all 10, and git
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
  violations over all 36 pairs. Otherwise no-go: stop.
- **If go:** item 10's edit-time hook (pack before edit) moves ahead of everything in §8 and becomes
  the v1. The query commands (`why`) become secondary.

#### Run2 result (2026-09-17): no-go

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

The corpus adds nothing measurable over what the repository already holds. That is the no-go condition
in the gate, and no part of the corpus accounts for an effect to narrow to.

**Limits of the result.**
- The repositories have essay-style commit messages, so git history is a strong baseline. A repository
  with terse messages was not tested, but there extraction has little to extract (S0's clap commits).
- One agent configuration (Sonnet through Claude Code, a neutral prompt) was tested. An instruction to
  obey the recorded prerequisites was deliberately not given; with it, the measure would be instruction
  following rather than the corpus.
- The tasks ask for the violating change directly. A gentler task, where breaking the prerequisite is a
  side effect, might show a difference that these do not.

---

## 5. Phase 1 — smysl integration and data model

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

- `facts` crate:
  - port the research extractor (`facts/src/main.rs`): function events with control context, macro
    argument parsing, doc comments, consts with values, struct field types, `cfg` and file-level `cfg`;
  - scope selection and fact templates per D9;
  - prose tagging per D10;
  - CI-matrix `cfg` evaluation;
  - cache keyed by commit and extractor version.
- `extract` crate:
  - multi-pass prompts per D6;
  - quote checking per D8;
  - extract-once cache keyed by commit and recipe (D7);
  - labels and sources per D5.
- **Decision at phase start (item 7):** own model client vs smysl's `model` feature. The multi-pass
  schemas do not fit `PromptOverride`'s `units` array, which argues for an own client that hands units
  to `stage`.
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
- **Done when:**
  - on the S0 set, no single-run status raise;
  - wrong `backs` edges are withdrawn (by review or the mutation gate) or pending in the queue, never
    followed by packing or `why`;
  - every reading traces to `tool:smysl-import`.

---

## 8. Later phases — order set by the spikes

| Item | Work | If recorder-first (S2) | If extractor-first (S2) |
|---|---|---|---|
| 10 | **UX and integration:** `cargo smysl record / why / check / stale / review`, Claude Code hooks (pack before edit, record at stop), git hook or merge driver, PR rendering | **Phase 4** | Phase 6 |
| 4 | **Input sources for ordinary repositories:** PR descriptions, review threads, agent transcripts (Entire checkpoints, Agent Note), diff-only extraction | Phase 6 | **Phase 4** |
| 5 | **Staleness:** item-hash invalidation of facts, cascaded through `dependents_via` to decisions; an `x.code/touches` edge whose anchor moved is withdrawn with a reason naming the commit, not rewritten; "stale" report per commit range | Phase 5 | Phase 5 |
| 6 | **Storage:** `.smysl/` layout, surface vs CBOR in git, merge driver using smysl merge, growth over hundreds of commits, compaction | Phase 5 | Phase 5 |
| 8 | **Cost and privacy:** per-commit token budget and price; model routing (small model for decisions/alternatives, pro for prerequisites); quota handling; local model viability | Phase 6 | **Phase 4** |
| 9 | **Evaluation:** grow S0 into a regression suite run on every prompt or model change | continuous | continuous |

S3 is no-go (§4), so none of this proceeds as planned.

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

## 10. smysl requests for 1.4.0 (not blocking)

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
- **Not filed:** `--granularity` does not choose the profile units are checked under; smysl deferred
  it to 1.4 itself.

## 11. Open questions for the owner

1. Name of the tool and subcommand (`cargo smysl`, `cargo why`, other).
2. Who labels S0, and which third repository with ordinary commit messages?
3. Budget per commit for model calls, and whether proprietary code may go to a hosted model.
4. Is the corpus committed to the analysed repository, or kept beside it?
5. **After S3's no-go (2026-09-17), which of these?**
   - **Stop here** and archive the repository with the results as the record.
   - **Test one of the limits** before stopping, at the cost of another run: a task where breaking the
     prerequisite is a side effect rather than the request, or a repository whose history does not state
     its reasons.
   - **Keep only what stands without the corpus claim:** deterministic facts about Rust code
     (`cargo-smysl-facts`) and smysl as a record of test evidence. Neither was measured by S3, and neither
     has its own case yet.
