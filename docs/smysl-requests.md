# smysl feature requests from rust_smysl

**For:** smysl `dev/1.3.0`, baseline `ba894c3` ("Open 1.3.0").
**From:** rust_smysl, a cargo tool that records *why* code changed — decisions, their
prerequisites, rejected alternatives and consequences — as a smysl corpus.

## How rust_smysl uses smysl

As a **library**, never through the CLI. The pipeline that the experiments settled on:

1. A model proposes content in several passes over a commit message and its diff:
   decisions, then prerequisites, alternatives and consequences for those decisions.
2. rust_smysl, not the model, assigns labels, statuses and sources (`git:<sha>`,
   `path@<sha>`), and checks every quote against the message and diff.
3. Units go through `smysl::stage::prepare` for rule M, check and attestation.
4. Prerequisites are linked by `conditions`, so decision uids do not move when a
   prerequisite is reworded. "What breaks if this is false" is `reverse_closure` over
   `grounds`, `deps` and `conditions`.

The requests below are what that pipeline could not do, or did badly, against 1.2.0 and the
1.3.0 working tree. Each has a reproduction and an acceptance test. Priorities are for
rust_smysl; R1 and R2 also affect every `smysl ingest` user.

| # | Request | Kind | Priority |
|---|---|---|---|
| R1 | Repair turn copies the input marker and raises statuses | Bug | High |
| R2 | Surface template gives no source syntax and no quote channel | Bug | High |
| R3 | Caller-supplied source for ingested units | Feature (library) | High |
| R4 | Export the quote check from the facade | Feature (library) | High |
| R5 | Refuse ambiguous labels instead of picking one | Bug + library API | Medium |
| R6 | Uid-level dependents over chosen edges, and clearer retraction wording | Feature + wording | Low |

---

## R1 — The repair turn copies the input marker and raises statuses

### What happens

Surface-path ingest of a real commit with `gemini-3.5-flash-lite` degraded to one opaque prose
unit in **3 of 3 runs** on the 1.3.0 build, although the label fix works (no malformed-label
diagnostics in two of the runs). The only diagnostics shown were:

```
error: SMY-E001: stray Text outside a record (at 0..17)
warning: SMY-W304: span degraded to opaque prose after 3 attempt(s)
```

Replaying the repair turn directly against the API explains both halves. In 3 of 3 samples the
corrected answer:

1. **starts with `<<<SMYSL-INPUT>>>`** — 17 bytes, exactly the stray span. The repair template
   fences the previous answer with `FENCE`, and the model copies the fence into its answer.
2. **raises every `cited` to `measured`.** The previous answer's problem was
   `SMY-E032: measured/cited without source`. The repair template *replaces* the system prompt,
   so the model no longer sees "Never `measured`" or the status rules. Changing the status is
   the easiest edit that looks like a fix.

A third problem hides the first two: a degraded chunk reports only the **last** attempt's
diagnostics (`last` in `crates/smysl-ingest/src/lib.rs`, around lines 410–431), so the original
cause (`E032`) is invisible and the reported cause (stray text) was introduced by the repair.

### Where

- `crates/smysl-ingest/src/prompt.rs` — `repair()`: new system prompt, previous answer fenced
  with `FENCE`.
- `crates/smysl-ingest/src/lib.rs` — the repair loop keeps only `last`.

### Proposal

1. **Keep the content template's system prompt in the repair turn**, with the correction
   instruction added to it rather than replacing it. The rules the model broke must still be
   in front of it when it fixes them.
2. **Don't fence the previous answer with the input marker.** Use a distinct marker for the
   previous answer, and strip a leading or trailing line that is exactly a known marker (or a
   code fence) from any answer before parsing. The boundary should tolerate that echo, as it
   already tolerates CRLF.
3. **Give `E032` a suggestion that lowers, never raises:** "add a `source`, or weaken to
   `inferred` with grounds or `speculative`". The repair turn carries suggestions as `[try: …]`,
   as the 1.3.0 label fix does.
4. **Report every attempt's error diagnostics for a degraded chunk**, marked by attempt, or at
   least the first attempt's alongside the last. The first attempt shows what the model got
   wrong; the last only shows what the repair broke.

### Acceptance

- Offline: a mock provider whose repair answer is wrapped in `<<<SMYSL-INPUT>>>` lines parses
  without `E001`.
- Offline: the repair request's system text contains the label format and the "Never
  `measured`" rule.
- Offline: a degraded chunk's report includes the first attempt's `E032`.
- Live (optional, one command): surface ingest of the fixture below with flash-lite produces
  units instead of one degraded prose unit in at least 2 of 3 runs.

### Reproduction input

Any multi-section commit message rendered as Markdown reproduces it. The one used was smysl's
own commit `4968383`: `git show -s --format='# Commit %h: %s%n%n%b' 4968383`, run with
`smysl ingest --rung document --path surface`.

---

## R2 — The surface template gives no source syntax and no quote channel

### What happens

Replaying the surface content template (version 2) against flash-lite gives clean records with
correct labels — and every one of them is `cited` with no `source`:

```
@claim c/nodejs-c-produce { status: cited }
~ Node.js implementation reaches C-Produce readiness with comprehensive test vectors …
```

The template says "A `cited` record needs a source" but its only example is
`@<type> <label> { status: <status> }`, so the model has no way to write one. Every record fails
`E032`, which starts R1.

The same replay also produced a **factual inversion**:
*"blake3.js is a hand-rolled binding to the same C library as Rust"* — the commit says a binding
was rejected *because* it would test the same C library. On the json-ast path a unit can carry
a quote that is checked against the document; the surface template asks for none, so nothing
checked this one.

### Proposal

1. Show the full header in the template:
   `@<type> <label> { status: <status>, source: { kind: doc, ref: "<document>" }, "ingest:quote": "<exact span>" }`
   — the staged surface output already uses `"ingest:quote"`, so the format exists.
2. Say plainly: without a source you can name, use `inferred` with grounds or `speculative`.
3. Run the quote check on the surface path exactly as on json-ast, so a surface answer cannot
   attribute text that is not in the document.
4. Consider making `auto` prefer json-ast whenever the provider enforces schemas, independent of
   input size. The 1.3.0 config key `ingest: { path: json-ast }` works around it, but the
   default is what new users meet.

### Acceptance

- The surface template contains a `source` example and an `"ingest:quote"` example.
- A surface answer with a fabricated `"ingest:quote"` gets `SMY-E307`, as on json-ast.

---

## R3 — Caller-supplied source for ingested units (library)

### Why

In rust_smysl the **tool** knows provenance exactly (commit sha, file path at sha) and the
model does not. In the experiments, models wrote `ref: CHANGELOG.md` for a commit message, or
`ref: commit` with no sha. Provenance is the one field a model should never invent, and today
`IngestOptions` has no way to set it: `json_ast::convert` takes `source` from the model's
answer or leaves it out.

It also removes the R2 failure for every caller that knows its document: the model never has
to spell a source at all.

### Proposal

```rust
IngestOptions::with_source(SourceRef, SourcePolicy)

#[non_exhaustive]
pub enum SourcePolicy {
    /// Units the model gave no source get this one.
    FillMissing,
    /// Every unit gets this one; a model-supplied source is replaced and reported.
    Override,
}
```

Constraints:

- `source` is inside the uid, so the source must be applied **before** rule M and staging,
  not patched afterwards. Otherwise identities move under the report, the same bug the SM-P14
  gate hit with weakening.
- Record it in the recipe conditions, since it changes the output.
- With `Override`, report a replaced model source as a warning. A model naming a different
  document than the one it was given is worth knowing about.

### Acceptance

- `FillMissing`: a `cited` unit with no source in the answer stages as `cited` with the given
  source, with no `E032`.
- `Override`: a model source is replaced, a warning names the unit, and the staged uid equals
  the uid of the same unit authored with the caller's source.
- Recipe hashes differ with and without a source.

---

## R4 — Export the quote check from the facade (library)

### Why

rust_smysl builds units itself and sends them through `stage::prepare`, so it never reaches
`Ingestor`'s quote check. The check exists (`smysl_ingest::quote::support` returning
`Support::{Present, Loose, Absent}`) but is `#[doc(hidden)]` and not re-exported. The choice
is between reimplementing it — a second definition of "loose" and "absent" drifting from the
first — or exporting the one smysl already has.

### Proposal

```rust
pub use smysl_ingest::quote::{support as quote_support, Support as QuoteSupport, QUOTE_KEY};
```

And, because a change is evidenced by several texts (message plus one diff per file), a
multi-source form that says which text matched:

```rust
pub fn quote_support_in<'a>(quote: &str, sources: &[(&'a str, &str)]) -> (QuoteSupport, Option<&'a str>);
```

This goes into the frozen contract, so the normalisation rules should be written down as part
of it.

### A normalisation question to settle first

The prototype checker needed more normalisation than `normalise()` does today to avoid false
"absent" results on real commits:

| Model wrote | Source had | `normalise()` today |
|---|---|---|
| `'Deterministic CBOR'` | `"Deterministic CBOR"` | Absent — straight `'` vs `"` are not unified |
| `smysl, the CLI` | `` `smysl`, the CLI `` | Absent — backticks kept |
| `nodejs/ reaches C-Produce` | `**nodejs/** reaches C-Produce` | Absent — emphasis kept |

The prototype's first attempt replaced these characters with spaces, which *broke* a match
(`` `smysl`, `` became `smysl ,`). Deleting them is what works. Whether Markdown
emphasis and backticks count as typography (like curly quotes) or content is smysl's call. It
should be decided before the function is frozen.

### Acceptance

- Facade exports the names; `tests/public-api.txt` gains them.
- `quote_support_in` returns the first matching source name, preferring `Present` over `Loose`
  across all sources.

---

## R5 — Refuse ambiguous labels instead of picking one

### What happens

A store merged from three extraction runs of the same commit binds `d/g90ec2f7-1` to three
different uids (merge correctly reports `label-collision` contentions). Then:

```
$ smysl trace d/g90ec2f7-1 --grounds merged.cbor    # exit 0, one of the three
$ smysl retract --dry-run d/g90ec2f7-1 merged.cbor  # exit 0, one of the three
```

Neither says the label was ambiguous. `load_store` in `src/main.rs` collects `LabelBinding`
records into a `BTreeMap`, so the last binding in record order wins. For `retract` this means
retracting a unit the caller did not choose.

A library user building a label map from `LabelBinding` records hits the same trap, so the fix
belongs in the library, with the CLI using it.

### Proposal

```rust
#[non_exhaustive]
pub enum LabelError {
    Unbound,
    Ambiguous(Vec<Uid>),
}

/// Every uid a store binds this label to.
pub fn label_bindings(store: &Store, label: &Label) -> Vec<Uid>;
/// The one uid, or why there is not exactly one.
pub fn resolve_label(store: &Store, label: &Label) -> Result<Uid, LabelError>;
```

The CLI's `resolve` uses `resolve_label`. On `Ambiguous` it lists the candidate uids with
their gists and exits with the same code `relink` uses for a supersession fork (5). An
ambiguity is the same kind of refusal: the tool will not adjudicate.

Within a single surface document the 1.3.0 W054 fix already decides ownership (last
declaration wins, with a warning), so `ParseOutcome.labels` stays as it is.

### Acceptance

- A CBOR store with one label bound to two uids: `trace` and `retract` by that label exit 5 and
  print both uids; by uid they work.
- `resolve_label` returns `Ambiguous` with both uids sorted.

---

## R6 — Uid-level dependents over chosen edges, and clearer retraction wording

### Why

rust_smysl links prerequisites with `conditions` edges (see "How rust_smysl uses smysl"), so
"what depends on this prerequisite" must follow `grounds`, `deps` **and** `conditions`.
`lineage::dependents(store, uid)` hardcodes `EdgeSet::support()`. `reverse_closure` does
accept any `EdgeSet`, but works in `NodeId`s, which the contract says never to hold outside one
traversal. A uid-level form removes the NodeId round trip from every caller:

```rust
pub fn dependents_via(store: &Store, uid: Uid, edges: &EdgeSet) -> Vec<Uid>;
```

### Wording

`smysl retract --dry-run` prints "would reach N unit(s), orphaning M". The count is
*retracted plus orphaned*: units that lose **all** their grounds. Retracting one of a decision's
four prerequisites reports "reach 1", which reads as "nothing depends on this". That is correct
for the retraction and misleading as an impact report. Either:

- rename to "would retract N unit(s), orphaning M", or
- also print dependents that keep other grounds, as "M more rest partly on it".

### Acceptance

- `dependents_via` with `EdgeSet::support()` equals `dependents`.
- With `conditions` added, a decision linked by `@rel p --conditions--> d` appears among the
  dependents of `p`.

---

## Not requested

For the record, so they are not re-proposed:

- **Label namespacing across extraction runs.** Two runs binding the same label to different
  uids is rust_smysl's naming problem (labels will carry a run or recipe id). smysl reporting
  it as a contention is correct.
- **Stable re-extraction.** Three runs of the same commit shared 12–24 of ~57 uids. That is
  model wording, not smysl. rust_smysl extracts once per commit and supersedes; it does not
  re-extract and merge.
- **Retraction following `conditions`.** Retraction semantics stay as they are; R6 gives the
  impact query separately.
