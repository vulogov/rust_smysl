# What this is for, and who for

Written for someone deciding whether to spend an afternoon on this. It is a review of the tool as it was
built rather than as it was designed, and it states what was measured, including the parts that did not
work.

## The short version

Two thirds of the commands never call a model. Those are the ones worth your time today: they are
deterministic, tested, and cost nothing to run. The model-dependent third is honest about being weak.

| | Commands | Worth it today? |
|---|---|---|
| **No model** | `doctor`, `facts`, `why`, `stale`, `review`, `bench`, `hooks` | yes |
| **A model, and measured** | `extract`, `check`, `evidence` | depends on your model, and on patience |

## Who it is for

**A maintainer whose decisions outlive the people who made them**, working in a repository whose commit
messages already carry reasons. The tool cannot extract what was never written: on terse
conventional-commit histories there is nothing to extract, which S0 found directly.

**Anyone evaluating a model on their own code.** `cargo smysl bench` works on its own — label a handful
of your commits, run your model, get precision and recall per kind. No corpus, no commitment to the rest
of the tool.

**Not a team wanting automated review.** `check` was right about one flag in ten on a free local model
and about nine in ten on a hosted one. The first is a prompt to look; the second is not a gate either,
and neither is sold as one.

## What it does that `git log` does not

This is the question that decides whether the tool earns its place, and the project measured the
uncomfortable version of it.

**S3, on this project's own repositories:** asked "why is this like this?", git history plus code beat
the corpus on **10 of 10** questions. Essay-style commit messages already hold the reasons, and the
corpus is a shorter summary of the same text — it keeps the reason and drops the detail that made the git
answer more useful. *Answering "why is this like this?" was dropped as a product claim.*

So the tension is real, and it cuts both ways:

- where commit messages are good, `git log` wins for a single commit;
- where they are poor, extraction has little to work with.

What survives that squeeze is what git cannot do at all:

| | |
|---|---|
| **`why <label>`** | what rests on a claim, across commits — a graph rather than a search |
| **`stale`** | which recorded reasoning is now *unexamined* because the code under it changed, compared by item and body hash so a function that merely moved is not flagged |
| **`review`** | the lifecycle: confirmed, withdrawn with a reason, closed with a note — never deleted, never rewritten |
| **`facts --scope`** | what a model should be shown about a change, chosen deterministically |

If you want one sentence for why this exists: **a queryable, code-anchored record of decisions, with
staleness detection, and an honest account of what the model half is worth.**

## Is any of it new?

The extraction is not. "A model summarises a commit" is crowded, and by this project's own measurement it
is the weakest part here. Four things are less common:

1. **The tool assigns labels, sources and statuses; the model proposes wording only.** Every quote is
   checked against the commit, and an absent quote caps its unit at `speculative`. Most tooling in this
   area stores what the model said as though it were established.
2. **Staleness by item body hash.** Reasoning is attached to code identity rather than to line ranges, so
   a function that moved is not stale and one that changed is.
3. **Review as records.** Confirming writes an attestation, rejecting writes a withdrawal whose reason
   unit says why, closing writes a resolution. The log keeps everything.
4. **It ships its own instrument.** `bench` measures the model *you* chose on *your* commits, and every
   published figure names the model it came from — including four experiments that failed and are listed
   so nobody repeats them.

If there is a contribution here, it is those four, not the extraction.

## What was measured, and what it cost

| | Result |
|---|---|
| `check`, held-out, local 14B | recall 0.38, precision 0.10 (10 of 96 flags), 13% of ordinary commits flagged |
| `check`, hosted `deepseek-v4-pro` | recall 0.75, precision ~0.89 on the development set, about $2 a pass |
| `extract`, three models, six blind-labelled commits | decisions 82–100% precision; prerequisites 41–88%; alternatives 31–79% |
| `extract` on a local 14B | over-produces: 54 decisions on a commit labelled with 4 |
| Cost of `extract` locally | 7 minutes for a small commit, ~29 for a 700 KB one |
| Two-pass agreement | 170 and 192 flags became 96, removing every false flag on a real commit in development |

**Tried and rejected**, each recorded so the time is not spent twice: a stricter `check` prompt (recall
0.65 → 0.30), a tightened extraction prompt (no fewer false positives), ranking decisions by whether
their quote is in the commit (88% real with a quote, 97% without — the signal is not there), retrieval
term filtering.

## Honest limits for a first release

- **One user, two repositories, one author.** Every figure here comes from `smysl` and `ucal`. That is
  why `bench` exists: the numbers are not expected to transfer, and you can check yours.
- **Over-production is the failure mode that matters.** A corpus of 54 decisions for a four-decision
  commit is worse than no corpus, and prompt wording was measured twice as not being the lever.
- **Extraction is slow.** Minutes per commit on a local model. It is meant to be run over a range when
  you choose to, which is why the hook only queues and `--estimate` prices a run before it starts.
- **A cloned corpus is someone else's claim.** Units carry the status the tool gave them; read
  `speculative` as what it says.

## Where to start

```sh
cargo smysl doctor                  # what the tool sees; no model, no writes
cargo smysl bench init --last 5     # measure your model before trusting any of it
cargo smysl stale                   # once you have recorded something
```

The full walk-through is [`smysl-workflow.md`](smysl-workflow.md); installation and choosing a model is
[`cargo-smysl-install.md`](cargo-smysl-install.md); what the tool trusts is
[`security.md`](security.md).
