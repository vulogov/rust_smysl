# Changelog

## 0.2.0 — unreleased

Development is on `development/0.2.0`. What is known to be worth doing, from what 0.1.0 measured:

- **Anchors for every decision, not only the file-quoted ones.** A decision quoted from the commit
  message anchors to nothing today, and most decisions quote the message — so `stale` reports at commit
  level for most of a corpus.
- **Over-production is the dominant extraction error** on a local model: on one commit labelled with 4
  decisions it reported 54. Prompt wording was measured twice and is not the lever. The untried levers are
  deterministic ones and a second opinion between two cheap models.
- **The recipe caps shape the answer.** Four of six commits stopped at exactly `max_decisions_total`, and
  every part of the largest returned its per-part cap, so those counts are the recipe rather than the
  commit.
- **`bench` has never been run by anyone but its author.** The first real use will say more about it than
  any test here.

## 0.1.0 — 2026-09-21

First release. A cargo subcommand that records why Rust code changed and keeps the record honest.

Every figure below names the model it was measured on, because quality where a model is involved is a
property of that model and not of this code. Protocols and caveats:
[`docs/implementation-plan.md`](docs/implementation-plan.md) §4.

### Commands

| | |
|---|---|
| `doctor` | versions, workspace, corpus, backlog, whether the code parses |
| `facts` | deterministic syntactic facts into a regenerable cache; `--scope` shows what a model would be given |
| `extract` | decisions, prerequisites, alternatives and consequences, recorded as a smysl corpus; `--since` for a range, `--queued` for what the hook queued, `--estimate` to price a run first |
| `why` | what rests on a claim; `--commit` reads a whole commit back, `--markdown` for a pull request |
| `check` | what a change contradicts — **advisory**, two passes, `--strict` to block |
| `evidence` | one claim against the facts, with the tests that bear on it; `--mutate` for the opt-in mutation gate |
| `review` | what waits for a person: confirm, reject, close — always as records, never a rewrite |
| `stale` | reasoning whose code has moved, by item and body hash |
| `bench` | measure extraction against your own labels, with your own model |
| `hooks` | a post-commit hook that only queues, and a merge driver for corpus documents |

### What was measured

- **`check`**, held-out, 93 changes, two-pass agreement: local `qwen2.5-coder:14b` — recall 0.38,
  precision 0.10 (10 of 96 flags), 13% of ordinary commits flagged. Hosted `deepseek-v4-pro` — recall
  0.75, precision ~0.89 on the development set, about $2 a pass. It ships advisory for that reason.
- **`extract`**, six commits labelled blind, three systems adjudicated item by item: decisions are the
  reliable kind (82–100% precision); prerequisites are not (41–88%, with recall moving inversely);
  alternatives have the worst precision (31–79%).
- **The deterministic half holds** whatever the model: retrieval, packing, quote and label validation,
  and the shipped `check` agrees with the harness it was ported from on all 92 held-out cases for the
  diff shown, 91 for candidates, 70 for the packed set — the rest differing only in units never judged.

### What was tried and rejected

Recorded because each cost real time: a stricter `check` prompt (recall 0.65 → 0.30); a tightened
extraction prompt (no fewer false positives, more of the kind it forbade); ranking decisions by whether
their quote is in the commit (decisions with a quote are 88% real, those without 97%); retrieval-side
term filtering (overlapping profiles). Two-pass agreement did work, and is the default.

### Design rules

The tool assigns every label, source and status; a model proposes content only. Every quote is checked
against the commit, and an absent quote caps its unit at `speculative`. Author prose never verifies. A
single run never raises a status. A test linked to a claim waits for a person. Review is recorded, never
a rewrite. A prompt that had to be cut, an answer that had to be salvaged, a commit read in parts, a
provider that truncated — each is reported where it happened.

### Install

```sh
cargo install --git https://github.com/vulogov/rust_smysl cargo-smysl
```

The default build has no TLS stack and speaks plain HTTP to a model on your machine; `--features hosted`
is the opt-in for a hosted provider. MSRV 1.86 for the shipped crates.
