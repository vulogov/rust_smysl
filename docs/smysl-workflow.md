# Working with cargo-smysl, end to end

Every command, in the order you would actually use them, on this repository. The output below is real —
`rust_smysl` is the first project the tool is used on, so it is also the first corpus it wrote.

Installation and model setup: [`cargo-smysl-install.md`](cargo-smysl-install.md).
What has been measured, and on which model: the [README](../README.md).

---

## The shape of it

```
   facts ──────────────┐
   (free, no model)    │
                       ▼
   extract ───────► .smysl/commits/<sha>.smy ───────► why
   (a model)          the record                      stale
                          │                           check
                          ▼                           evidence
                       review ◄──── everything a model proposed
```

Three things to hold on to:

1. **The tool reads git itself.** Labels, sources and statuses are its own; a model proposes wording only.
2. **Every quote is checked.** A quote that is not in the commit caps its unit at `speculative`.
3. **Nothing a model proposes changes a status on its own.** Proposals queue for a person.

---

## 1. `doctor` — does the tool see what you see?

```sh
cargo smysl doctor
```

```
cargo-smysl: 0.2.0
smysl library: 1.6.0 (formats smysl/0.1, smysl/1.0)
code schema: x.code/v1
invoked by cargo: ~/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo
workspace: <your repository>
members: 9
corpus: <your repository>/.smysl (present)
facts: 50 Rust file(s), 485 function(s), 0 parse failure(s)
```

No model, no writes. If `facts` reports parse failures, the deterministic half of everything else is
working with less than your code.

---

## 2. `facts` — what the tool knows without asking anyone

Facts are syntactic and deterministic: functions, what they call, what calls them, their control context,
their `cfg`, and the body's hash. They live in a regenerable cache (`.smysl/facts/`), never in the corpus,
because they can always be recomputed and should never be argued with.

```sh
cargo smysl facts                       # the working tree
cargo smysl facts HEAD --json           # one commit, machine-readable
cargo smysl facts --file crates/cargo-smysl-verdict/src/matching.rs
```

`--scope` shows what the tool would put in front of a model for a given name — the touched items,
anything those names mention, and one hop along calls:

```sh
cargo smysl facts --scope --name retrieve
```

```
23 of 641 fact(s) bear on this change (0 file(s), 1 name(s), 1 hop(s))

[Calls] fn Labels::new(commit: &str, run: u32) -> Result<Labels, IdError> at crates/cargo-smysl-corpus/src/lib.rs:68
  doc (prose): `commit` is a hex revision (shortened to 12); `run` is 0 for the first extraction.
  Method method=to_ascii_lowercase receiver=commit.chars().take(12).collect::<String>() at :73
  …
```

Note `doc (prose)`. Author text — doc comments, string literals, commit subjects — is marked, and prose
never verifies a claim. It is there to help find the right code, not to confirm anything about it.

---

## 3. `extract` — record why a commit happened

This is the one command that must ask a model, and the slowest thing here.

```sh
cargo smysl extract HEAD
cargo smysl extract b2dbe48 --dry-run      # see what it would record, record nothing
cargo smysl extract b2dbe48 --force        # redo a commit already recorded
```

A real run on this repository:

```
10 decision(s), 23 prerequisite(s), 18 alternative(s), 14 consequence(s)
quotes: 19 present, 16 loose, 24 absent
recorded .smysl/commits/b2dbe4820872.smy (308 record(s) added)
```

What to read in that:

- **`quotes: … absent`** — 24 items quoted something not in the commit. Those units are `speculative`,
  not `cited`. That is D8 doing its job, and the count is worth watching: it is how you notice a model
  drifting from the commit.
- A commit larger than the model's window is **read in parts**, not truncated, and says so. On a 739 KB
  commit here: `it was read in 6 parts`, then `28 decision(s) were reported by more than one part and
  kept once`.
- **Extraction happens once per commit and recipe.** Improvements arrive as a new recipe, never as a
  re-extraction merged into the old record.

Cost here, local 14B: 7–11 minutes for a small commit, about 29 minutes for a 700 KB one.

---

## 4. `why` — what rests on a claim

```sh
cargo smysl why p/gb2dbe4820872-2-1
```

```
p/gb2dbe4820872-2-1: 2 unit(s) rest on it
  d/gb2dbe4820872-2  decision (cited)
      Add `matching` module to `lib.rs`
  q/gb2dbe4820872-2-2  claim (inferred)
      The project now has a dedicated module for checking one claim against the facts.
```

Labels tell you what you are looking at: `d/` decision, `p/` prerequisite, `r/` rejected alternative,
`q/` consequence, `e/` evidence, `a/` code anchor, `t/` test reading. The `g<sha>` in the middle is the
commit it came from, so a label is traceable without a lookup.

`why` also takes an item path (`crate::cli::cli`) when you want to start from code rather than a label.

---

## 5. `check` — what does this change contradict?

**Advisory.** On a local 14B, about one flag in ten was correct; read a finding as a place to look.

```sh
cargo smysl check                       # HEAD against the corpus
cargo smysl check --patch changes.diff  # a patch, or `-` for stdin
cargo smysl check --json
cargo smysl check --strict              # exit 5 on findings, for a hook or CI
cargo smysl check --passes 1            # faster, noisier
```

A finding has this shape (the example is constructed; the advisory footer is verbatim):

```
p/g90ec2f781421-8-1 constraint (cited, source tests/dispatch.rs@90ec2f781421)
    cli() registers all twenty-two subcommands from the COMMANDS table unconditionally
  at:   47| -    for cmd in COMMANDS.iter().filter(|c| c.enabled) {
  because: the change makes registration conditional, which this prerequisite says it is not

advisory: `check` reports, it does not block. `--strict` exits 5 on findings.
measured on a local 14B: about 1 flag in 10 was correct, and it finds about a
third of real contradictions. Read a finding as a place to look.
```

Two passes run by default, with different orderings, and only findings both report are kept — measured,
that turned 170 and 192 flags into 96 and removed every false flag on a real commit in development. Every
finding must name a unit that was actually judged and quote a line that is actually in the diff; a verdict
failing either check is dropped and counted.

---

## 6. `evidence` — check one claim against the code, and against the tests

```sh
cargo smysl evidence p/gb2dbe4820872-2-1                   # retrieve facts, judge, say what follows
cargo smysl evidence p/gb2dbe4820872-2-1 --tests           # also shortlist tests that might bear on it
cargo smysl evidence p/gb2dbe4820872-2-1 --tests --link    # run them, record readings, propose edges
cargo smysl evidence p/gb2dbe4820872-2-1 --tests --link --mutate 3   # and check the tests notice
```

```
30 structural fact(s) and 10 prose fact(s) retrieved

covered:
  The project needs a way to check one claim against the facts.

verdict: Partial — one run is not enough to raise a status: a second run or a person is needed
```

That verdict is the policy, not the model's opinion. `Supported` needs **every part of the claim covered
by structural facts** *and* two independent runs agreeing, or a person's word. Coverage that only adds up
across runs is not agreement. A normative rule ("a test must not depend on the machine") can reach
`ImplementedBy` at most: code can follow a rule, it cannot make one true.

With `--tests --link` the tests run at that commit and their results are imported as `measured` units:

```
  Exercises: a_contradiction_counts_only_when_it_names_a_fact_that_exists — …
running: cargo test --locked --no-fail-fast -- --exact a_contradiction_counts_…
  passed a_contradiction_counts_only_when_it_names_a_fact_that_exists (0s)

3 reading(s) recorded, 3 edge(s) proposed — each waits for a person: cargo smysl review
```

`--mutate N` is the opt-in gate: it changes the code the claim is about — a comparison flipped, a boolean
flipped — and asks whether the linked test notices. A test that notices *nothing* is recorded as
exercising the code rather than verifying the claim. It refuses nothing else, because when this was
measured it kept only 2 of 5 valid links. The file is always restored, and an interrupted run's backup is
restored at the start of the next one.

---

## 7. `review` — the part only a person can do

Everything a model proposed waits here. Nothing is ever deleted or rewritten.

```sh
cargo smysl review
```

```
12 item(s) waiting:
  0: backs b3:krv65sfccexdhaikt2ecqgg7v5 -> b3:qyrbxgbbdzqvcaq4ey3errzjyz
  1: x.code/exercises b3:i4xmpegfxnccvyh4ea6tdn6ijz -> b3:7d2kssacqjs6m5tgify6j2mg5k
  …

answer one with: cargo smysl review --as-person NAME --item N \
               --confirm | --reject "why" | --close "note"
```

```sh
cargo smysl review --as-person vulogov --item 0 --confirm
cargo smysl review --as-person vulogov --item 1 --reject "this test never touches that code path"
cargo smysl review --as-person vulogov --item 2 --close "both readings are right; the claim was vague"
```

- **confirm** writes your attestation on the edge — it becomes evidence attributed to you, not to a model.
- **reject** writes a withdrawal whose reason unit says why. The edge stays in the log and stops being
  followed, packed or counted. Nothing is deleted.
- **close** records that a disagreement was looked at, with a note, and decides nothing.

---

## 8. `stale` — reasoning whose code has moved

```sh
cargo smysl stale
cargo smysl stale --since v1.2.0
cargo smysl stale --json
```

```
b2dbe4820872: 26 unit(s) rest on code that has moved — 8 item(s) changed, 1 gone
  Retrieval::default changed in crates/cargo-smysl-verdict/src/matching.rs
  Shown<'_>::text changed in crates/cargo-smysl-verdict/src/matching.rs
  not_yet is no longer in crates/cargo-smysl/src/commands.rs
  …

1 of 1 recorded commit(s) rest on code that has moved.
Nothing is withdrawn: whether the reasoning still holds is a person's call.
```

The comparison is **by item and by body hash**, not by file: a function that moved down a file without
changing is not stale. A decision quoted from a file is anchored to that file's items, so `stale` names
the decisions behind each moved item; a decision quoted only from the commit message has the commit as its
scope and is reported at that level, because claiming otherwise would invent precision.

---

## 9. `bench` — measure the model *you* chose

The figures in the README are about three models on six commits. Yours are yours to measure.

```sh
cargo smysl bench init --last 5
```

Writes, per commit, into `.smysl/bench/`:

- `<sha>.diff` — the commit as one document: why it is here, its message, then its diff. Read this.
- `<sha>.toml` — the template you fill in, with the definitions in front of you.

Label **before** you look at what the tool extracted. A label written afterwards measures agreement with
the tool, not with the commit — which is the mistake this exists to prevent. The template states the
boundaries that are hardest:

- a **decision** is a choice that could have gone otherwise; a commit with no choice in it has none, and
  that is a valid label;
- a **prerequisite** is what had to be true for a decision to be right — *not* the motivation, *not* the
  argument, *not* the decision restated;
- an **alternative** is an option turned down.

Then:

```sh
cargo smysl bench status                # what is labelled, what is waiting, what does not parse
cargo smysl extract <sha>               # for each labelled commit
cargo smysl bench adjudicate            # pairs extracted items with your labels; you set `match`
cargo smysl bench score
```

```
6 commit(s), recipe v1

kind           extracted  labels  precision  recall  pending
decision              51      73        96%     63%        0
prerequisite          67      68        61%     54%        0
alternative           48      29        42%     69%        0

These are figures about the model you used, on these commits — not about the tool.
```

(The numbers above are this project's own measurement of a hosted `pro` model over six commits, shown to
say what the output looks like. Your run will print yours.)

`match` is either a label id of the same kind or `"none"`; `suggest` proposes candidates by word overlap
and decides nothing. Precision appears as soon as anything is paired; recall waits until nothing is
pending, because recall over half an adjudication flatters whatever was easy to pair.

---

## The corpus commits are not work

Recording a commit writes files, and those files are a commit. If the tool recorded *that*, you would
extract, commit, and find one commit left to extract — for ever.

A commit that touches nothing but `.smysl/` is the tool's own output and is skipped: by
`extract --since`, by `extract --queued`, and by the backlog `doctor` reports. A commit that touches
`.smysl/` *and* source is a real change and is recorded like any other.

```
$ cargo smysl extract --since HEAD~4 --estimate
4 commit(s) since HEAD~4, 3 already recorded, 1 that only write the corpus, 0 to do
```

## A week of using it

There is no ceremony here. What it looks like in practice:

```sh
# after a commit worth remembering the reasons for
cargo smysl extract HEAD

# before changing something old
cargo smysl why crate::verdict::check
cargo smysl check                      # what does my change contradict?

# periodically
cargo smysl stale                      # what rests on code that has moved?
cargo smysl review --as-person you     # answer what is waiting
```

And once, before trusting any of the model-dependent parts: `cargo smysl bench`, on your own commits,
with your own model. Everything else in this document is either deterministic or advisory, and says which
it is.
