# smysl feature requests from rust_smysl — for 1.5.0, before the cut

**For:** the smysl 1.5.0 development branch (`dev/1.5.0`, `e09a55e`). cargo-smysl pins the latest published
release (1.4.0 today) and adopts these once 1.5.0 is on crates.io.
**Previous:** R1–R9 in 1.3.0, R10 and R12 in 1.4.0 (`docs/smysl-requests-1.4.md`); R11, R13 and R14 are
open on purpose as S2 tasks.

All five come from building `cargo smysl check` (S4: given a diff and the corpus, report the recorded
decisions the change contradicts). Each says what the workaround costs today.

| # | Request | Kind | Priority | Where it bites |
|---|---|---|---|---|
| R16 | Retrieval restricted to a caller's candidate set | API | High | every `check` run retrieves 200 hits to keep 12 |
| R17 | A unit's labels, without scanning the log | API | High | the corpus keeps its own uid → label map |
| R18 | Where a quote matched, not only whether it did | API | Medium | `check` numbers diff lines itself to cite one |
| R19 | Optional suffix folding in the lexical tokenizer | Retrieval | Medium | "required argument" does not retrieve "commands require an argument" |
| R20 | Units by source prefix | API | Low | anchoring a unit to a changed file is a full scan per diff |

**Already in 1.5.0 and wanted here, no request needed:** C8 packing (`PackRequest::resting_on`), `rests_on`
and `trace_via`, `review_with(... confirming ...)`, `stage::prepare_attested`, and `attestations_of` /
`agreement`. The first fixes a real hole for us: prerequisites hang off decisions by `conditions` (D3), so
a packed decision used to arrive without the prerequisite it rests on.

---

## R16 — retrieval restricted to a caller's candidate set

### What happens

`Query` restricts by `kinds` (kernel type) and `min_status`, but not by uid. `check` wants the best hits
*among units it can act on*: decisions, prerequisites and rejected alternatives that belong to the
repository's own schema. Today it asks for 200 hits and filters afterwards, because a store also holds
consequences, evidence, anchors and readings — all `Claim`, `Evidence` or `Data`, and a rejected
alternative is a `Claim` like any other, so `kinds` cannot separate them.

With a corpus of a few hundred units that is merely wasteful. Over a repository's whole history it is
wrong: the top 200 fill with ineligible units and the eligible ones never surface.

### Proposal

`Query::within(impl IntoIterator<Item = Uid>)`, or a `within: BTreeSet<Uid>` field (empty means no
restriction), honoured by `Retriever::search`: scoring unchanged, candidates restricted before the limit
applies.

### Acceptance

- A query with `within` returns only units in the set, up to `limit`, in the same order as the same query
  without it filtered afterwards.
- An empty `within` behaves exactly as today (no restriction).
- `Bm25` scores do not change: IDF still comes from the whole index, so a restriction does not silently
  re-weight terms.

## R17 — a unit's labels, without scanning the log

### What happens

`label_bindings(store, &Label) -> Vec<Uid>` and `resolve_label` go label → uid. Every consumer that
*prints* a store needs uid → label: a finding names `p/g90ec2f781421-4-1`, not `b3:xkcd…`. cargo-smysl
therefore keeps its own `BTreeMap<Uid, Label>`, filled from `Staged::labels` as it stages, and a store
read back from disk has no such map without walking every record.

### Proposal

`labels_of(store: &Store, uid: &Uid) -> Vec<Label>` (a unit may carry more than one label, as 1.3's
second-label work established), and `label_index(store: &Store) -> BTreeMap<Uid, Vec<Label>>` for a
printer that needs all of them at once.

### Acceptance

- `labels_of` returns every label bound to that uid, in canonical order, and an empty vector for none.
- For every label `l` in a store, `labels_of(store, resolve_label(store, l)?)` contains `l`.
- `label_index` agrees with `labels_of` on every unit, and a store opened from disk gives the same answer
  as the store that wrote it.

## R18 — where a quote matched, not only whether it did

### What happens

`quote::support(quote, source)` and `support_in(quote, &[(name, text)])` answer `Present` / `Loose` /
`Absent`, and `support_in` names which source matched. Neither says *where*. A finding about a diff has to
point at a line, so `check` numbers the diff lines itself, asks the model for a number, and validates the
number against its own table — re-implementing the matching that `support` already does, and with weaker
normalisation than smysl's.

### Proposal

`quote::support_span(quote: &str, source: &str) -> (Support, Option<Range<usize>>)`, the byte range in
`source` that matched, under the same normalisation rules; and the same range from `support_in`. A caller
turns a range into a line number.

### Acceptance

- For a `Present` match, the range's text normalises to the same string as the quote.
- For a `Loose` match, the range covers the loosely matching region; `Absent` gives `None`.
- `support_span(q, s).0 == support(q, s)` for every input: the verdict is unchanged.

## R19 — optional suffix folding in the lexical tokenizer

### What happens

`tokenize` lowercases, splits identifiers and keeps the whole token. It does not fold inflections, so a
diff saying `.required(true)` does not retrieve the prerequisite "Seven commands **require** an argument,
so clap rejects bare names before the router runs". Measured in S4: that prerequisite was never retrieved
for any of the five diffs that broke it, at any candidate limit or pack budget (`eval/s4/results/`).

### Proposal

An opt-in, deterministic, pure fold of common English suffixes (`s`, `es`, `ed`, `ing`, `ly`) applied to
both index and query, e.g. `Bm25::index_with(store, Tokenizer::folding())`. Off by default, so existing
scores and golden packs do not move.

### Acceptance

- With folding on, a query "required argument" retrieves a unit whose text says "commands require an
  argument"; with it off, behaviour and scores are byte-identical to 1.4.
- Folding is idempotent and the same on every platform (no locale, no Unicode case tables beyond what
  `tokenize` already uses).

## R20 — units by source prefix

### What happens

cargo-smysl records provenance as `SourceRef { kind: File, reference: "src/main.rs@90ec2f781421" }` (D5).
To find the units anchored to the files a diff touches, `check` iterates every unit in the store and
string-matches the prefix, per diff.

### Proposal

`Store::units_with_source_prefix(&str) -> Vec<Uid>`, backed by the index the store already maintains.

### Acceptance

- Returns exactly the units whose `source.reference` starts with the prefix, in canonical order.
- Agrees with a full scan on a store holding units with and without sources.
