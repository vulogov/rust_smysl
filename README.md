# cargo-smysl

A cargo subcommand that records **why** Rust code changed — the decisions a change makes, what had to be
true for them, what was rejected, what follows — as a [smysl](https://crates.io/crates/smysl) corpus, and
keeps that record honest: deterministic facts about the code, test results as evidence, and a report when
the code a decision rests on moves.

**What it promises:** that each command does what its design says, with the measured figures printed where
a model is involved. Acting on what it reports belongs to whoever reads it — a person, a CI step, a hook.

**What it is not:** a transcript store, a static analyzer, or an oracle that marks a claim true because a
model said so. A model proposes content; the tool assigns every label, source and status, and checks every
quote against the commit it came from.

**Version 0.2.0.** What changed, and what was measured: [`CHANGELOG.md`](CHANGELOG.md).

## Install

```sh
cargo install cargo-smysl                                     # from crates.io
cargo install --path crates/cargo-smysl                       # from a checkout
cargo install --git https://github.com/vulogov/rust_smysl cargo-smysl
```

One binary, `cargo-smysl`, which cargo runs as a subcommand. The default build has **no network stack**:
it speaks plain HTTP to a model on your machine. A hosted provider over TLS is the `hosted` feature, off
by default.

```sh
cargo install --path crates/cargo-smysl --features hosted     # to use a hosted provider
```

## The commands

A full walk-through, on this repository, is in
[`docs/smysl-workflow.md`](docs/smysl-workflow.md).

| Command | What it does | Needs a model |
|---|---|---|
| `doctor` | Versions, workspace, corpus, and whether the code parses | no |
| `facts [rev]` | Deterministic facts about the code into a regenerable cache | no |
| `extract [rev]` | Decisions, prerequisites, alternatives and consequences for a commit, recorded | **yes** |
| `why <label>` | What rests on a recorded claim, and what it rests on | no |
| `check [rev]` | What a change contradicts in the corpus — **advisory** | **yes** |
| `evidence <label>` | Retrieve facts for one claim, shortlist and run the tests that bear on it | **yes** |
| `review` | Work through what waits for a person, and record the answer | no |
| `stale` | Reasoning whose code has moved since it was recorded | no |
| `bench` | Measure extraction against your own labels, with your own model | no |

Exit codes: `0` success, `1` failure, `2` usage error, `5` findings under `check --strict`.

## Choosing a model

The model is yours, never the tool's. Provider, model, endpoint, key variable, context window,
characters per token, the number of passes and the system prompt are all settings — flags, environment
variables, or defaults — and every run records which were used.

```sh
cargo smysl check --provider ollama --model qwen2.5-coder:14b        # the default: local, free
cargo smysl check --provider openai --model deepseek-v4-pro \
                  --endpoint https://api.deepseek.com/chat/completions --key-var DEEPSEEK_API_KEY
```

The prompt is fitted to whatever model you name: the commit is sized to its window, a commit too large is
read in parts rather than truncated, and anything cut or salvaged is reported rather than passed off as a
clean run.

## What has been measured, and on which model

**Quality is a property of the model you choose.** These are the figures this project measured; they are
about those models, not about this tool. Full protocols and caveats:
[`docs/implementation-plan.md`](docs/implementation-plan.md) §4.

### `check` — finding what a change contradicts

Held-out set, 93 changes over two repositories, two passes with only agreed findings kept:

| | local `qwen2.5-coder:14b` | hosted `deepseek-v4-pro` |
|---|---|---|
| Contradictions found (recall) | 0.38 | 0.75 |
| Flags that were correct (precision) | **0.10** (10 of 96) | ~0.89 (development set) |
| Ordinary commits wrongly flagged | 13% | — |
| Cost per pass | free | about $2 |

So `check` **ships advisory**: it prints findings and exits 0 unless `--strict`. On a local 14B about one
flag in ten was correct — read a finding as a place to look, not as a claim that something is wrong. The
deterministic half (retrieval, packing, quote and label validation, determinism) holds regardless of the
model; the judgement is the part that moves.

### `extract` — decisions, prerequisites and alternatives

Six commits labelled blind by the repository owner (73 decisions, 68 prerequisites, 29 alternatives), three
systems adjudicated item by item. Precision / recall:

| System | decisions | prerequisites | alternatives |
|---|---|---|---|
| `deepseek` research run | 82% / 67% | 41% / 59% | 31% / 69% |
| `flash` research run | 100% / 37% | 88% / 22% | 79% / 52% |
| `pro` research run | 96% / 63% | 61% / 54% | 42% / 69% |

Decisions are the reliable kind; the systems differ in recall, not correctness. **Prerequisites are the
weak kind** — the system that finds most of them is right 41% of the time, and the one that is right 88%
of the time finds a fifth. Alternatives have the worst precision of the three.

On a local 14B the failure is over-production: on one commit labelled with 4 decisions it reported 54.
That is why nothing a model proposes changes a status on its own (see *The rules the tool follows*).

### Things that were tried and did not work

Recorded because they cost real time and would otherwise be tried again:

- **A stricter `check` prompt** — recall fell 0.65 → 0.30, and it dropped 10 of 22 correct flags.
- **A tightened extraction prompt** naming what is not a decision — no reduction in false positives, and
  more of the kind it forbade.
- **Ranking decisions by whether their quote is in the commit** — measured against the owner's labels,
  decisions with a quote in the commit are 88% real and those without are 97% real. The signal is not
  there; it ships off.
- **Retrieval-side term filtering** for `check` — correct and wrong flags have overlapping term profiles.
- **A term-weight threshold, agreement of a single model with itself** — see the plan.

What did work: **two-pass agreement** (170 and 192 flags became 96, and it removed every false flag on a
real commit in development), which is why `check` runs two passes by default.

## Measure your own model

The figures above are about three models on six commits. The only figure about *your* model on *your*
commits is the one you measure:

```sh
cargo smysl bench init --last 5     # a reading sheet and a label template per commit
#   read .smysl/bench/<sha>.diff, fill in .smysl/bench/<sha>.toml, set status = "done"
cargo smysl bench status
cargo smysl extract <sha>           # for each labelled commit
cargo smysl bench adjudicate        # pairs what was extracted with what you labelled
cargo smysl bench score             # precision and recall per kind
```

Label **before** reading what the tool extracted. A label written afterwards measures agreement with the
tool rather than with the commit.

## The rules the tool follows

These are design decisions, each made because something was measured. The plan cites the experiment for
every one.

- **The tool assigns labels, sources and statuses; a model proposes content only.** Sources are the
  commit as the tool read it with `gix`.
- **Every quote is checked against the commit.** A quote that is not there caps its unit at `speculative`.
- **What the code says about itself never verifies it.** Doc comments, string literals and commit subjects
  are marked as prose: they help find the right code and never establish a claim.
- **A single run never raises a status.** `Supported` needs every part of a claim covered by structural
  facts *and* two independent runs agreeing, or a person's word.
- **A test linked to a claim waits for a person.** A model proposing the link is not a review; a failing
  test never backs anything.
- **Review is recorded, never a rewrite.** Confirming writes an attestation, rejecting a withdrawal with a
  reason, closing a resolution with a note. Nothing is deleted.
- **Nothing is hidden.** A prompt that had to be cut, an answer that had to be salvaged, a set of units
  split across calls, a provider that truncated — each is reported where it happened.

## What it trusts

A commit, a model's answer and a cloned `.smysl/` are all treated as things someone else wrote. What that
means in practice, and what a security scan of this code found and changed, is in
[`docs/security.md`](docs/security.md). Two properties worth stating here: the default build cannot make
an HTTPS request at all, and it will not send a commit over plain HTTP to anywhere but this machine.

## Where things are kept

```
.smysl/
  commits/<sha>.smy     one document per commit: the record, in smysl's surface syntax
  store.cbor            the store, derived from the documents
  facts/                deterministic facts, regenerable, safe to delete
  bench/                your labels and adjudications
```

The documents are the record and the store is derived: `cargo smysl doctor` will rebuild the store from
the documents. Whether to commit `.smysl/` to the repository it describes is your call — this repository
does not, because its corpus is written by trial runs of the tool on itself.

## Workspace

| Crate | Role | Model |
|---|---|---|
| `cargo-smysl` | the subcommand | — |
| `cargo-smysl-corpus` | units, labels, `x.code/v1`, smysl staging and queries | free |
| `cargo-smysl-facts` | deterministic syntactic facts (syn) | free |
| `cargo-smysl-git` | reading commits with `gix` | free |
| `cargo-smysl-extract` | the model client, and extraction in passes | uses |
| `cargo-smysl-verdict` | `check`, fact matching, the status policy, staleness | uses |
| `cargo-smysl-evidence` | tests as measured evidence, and the mutation gate | uses |
| `cargo-smysl-bench` | labels, adjudication and scoring | free |

## Licence

Unlicense.
