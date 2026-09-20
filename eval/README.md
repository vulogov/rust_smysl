# S0 — labelled evaluation set

The instrument the Phase 0 gates use (see [`docs/implementation-plan.md`](../docs/implementation-plan.md)
§4). It has 20 commits in [`commits.toml`](commits.toml), one label file per commit written by a person,
and a scorer that turns an extraction into precision and recall per item kind.

## Workflow

```sh
cargo run -p cargo-smysl-eval -- templates           # write label templates (skips files that exist)
cargo run -p cargo-smysl-eval -- worklist --studied  # labelling order, and a reading sheet per commit
# label eval/labels/<repo>/<sha>.toml by hand, then set status = "done"
cargo run -p cargo-smysl-eval -- adjudicate <system>  # pair extracted items with label items
# fill `match` in eval/adjudications/<system>/<repo>/<sha>.toml
cargo run -p cargo-smysl-eval -- score <system>       # precision / recall per kind, per repo, overall
```

**One commit is one document.** `worklist` writes `eval/labels/<repo>/<sha>.diff` beside each label file:
why the commit is in the set, its message, then its diff, so labelling needs nothing else open. The
sheets are regenerable and not committed. `--studied` lists the six commits the research already
extracted — each of those, once labelled, scores three systems at once.

**Label blind.** Write a commit's labels before looking at any extraction of that commit. Extractions
of the studied commits are in `extractions/`, so do not open that directory until the labels you
need are done. A label written after reading a model's answer measures agreement with the model,
not quality.

## What to label

For each commit, from its message **and** its diff:

- **decision:** a choice the change makes. `kind = "act"` for something done, `kind = "decline"`
  for something deliberately not done (held back, left unfixed, not chased). A refactor with no
  choice in it can have no decisions; that is a valid label.
- **prerequisite:** something that must hold for a decision to be correct, which the change relies
  on and does not itself establish.
- **alternative:** an approach considered or implicitly rejected, with the reason if one exists.

Consequences are not labelled in S0.

## Boundary cases (the ones models kept getting wrong)

| Not a prerequisite | Why | Example |
|---|---|---|
| **Motivation** (the problem) | It is why the change exists, not what it relies on | "seven commands could stop working with the suite green" |
| **Rationale** (the argument) | It explains the choice; a prerequisite is what the choice assumes | "a binding would test two callers of one implementation" |
| **The change itself** | Circular | "nodejs reaches C-Produce" for "implement C-Produce in nodejs" |
| **A restated decision** | Circular | "tests must not depend on the tree" for "tests build their own documents" |

| Is a prerequisite | Kind |
|---|---|
| "cli() registers all subcommands from COMMANDS unconditionally" | `existing-behaviour` |
| "clap rejects a missing required argument before the router runs" | `existing-behaviour` |
| "cargo-mutants reuses build directories" | `tool-setting` |
| "an anchor appears exactly once in its chapter" | `invariant` |
| "the doc-output test was wired into the suite in 0.13" | `prior-change` |

**Normative or factual.** Set `normative = true` for a rule the code should follow ("a test must not
depend on the machine"); leave it `false` for a statement of what code or a tool does. Facts can show
code follows a rule; they cannot show the rule is true.

**Evidence.** Optionally paste the verbatim span (message or diff line) the item rests on. It helps
adjudication and is not scored.

## Scoring

- **Adjudication** pairs each extracted item with at most one label item of the same kind. The tool
  suggests candidates by word overlap, and a person decides: a label id, or `none`.
- **Precision** = extracted items matched / extracted items.
- **Recall** = label items matched by at least one extracted item / label items.
- Two extracted items matching the same label count as correct for precision, and the label counts
  once for recall.
- Unlabelled (`status = "todo"`) or unadjudicated items are reported, never scored as zero.
