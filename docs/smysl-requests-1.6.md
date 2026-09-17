# smysl feature requests from rust_smysl — for 1.6.0

**For:** the smysl 1.6.0 development branch, once 1.5.0 is cut. cargo-smysl pins the latest published
release and adopts these when 1.6.0 ships.
**Previous:** R1–R9 (1.3.0), R10 and R12 (1.4.0), R16–R20 (1.5.0, all implemented and verified against
the working tree). R11, R13 and R14 stay open on purpose as S2 tasks.

All three come from running `cargo smysl check` against local and hosted models (S4), where the same
pipeline must fit a 16k local context and a hosted one, and a finding has to be explainable.

| # | Request | Kind | Priority | Where it bites |
|---|---|---|---|---|
| R21 | A pack that fits the caller's budget, not only its own | API | High | `check` packs to 3000 tokens, then fits the whole prompt to the model itself |
| R22 | Which query terms a hit matched | Retrieval | Medium | a missed prerequisite is invisible without reading packs by hand |
| R23 | Retrieval filtered by an extension schema's own kind | API | Medium | `Query::within` works, but the caller enumerates eligible units on every diff |

---

## R21 — a pack that fits the caller's budget, not only its own

### What happens

`PackRequest::budget(n)` is a budget in smysl's own units (`Estimator::Utf8Div4`, `ceil(len/4) + 2`),
and `PackInfo::estimator` says so honestly. A caller sending the pack to a model has a different budget:
the model's context window, less its system prompt, less the diff or question it is asking about, less the
room the answer needs. `check` therefore packs to a fixed 3000, builds its prompt, estimates the total,
and splits the judgement into several calls when it does not fit — with a warning, because a model
judging six units at a time sees less than one judging sixty (rust_smysl decision D17).

The packing decisions and the fitting decisions are then made in two places that cannot see each other:
smysl chooses what to carry without knowing the prompt it lands in, and the caller drops units from a
selection smysl built as a whole.

### Proposal

Let the caller state what else occupies the window, so one decision covers both:

```rust
PackRequest::budget(context_limit)
    .reserving(prompt_tokens + question_tokens + answer_tokens)
```

`reserving(n)` subtracts `n` before solving; `PackInfo` records both numbers. A caller that knows its
provider's tokenizer can also supply the estimator, which `Estimator` already models as an enum with one
variant — a `Estimator::External(fn(&str) -> u64)`, or a trait object, would let a tokenizer that is not
smysl's answer the cost question.

### Acceptance

- `budget(b).reserving(r)` selects exactly what `budget(b - r)` selects, and `PackInfo` reports budget
  `b`, reserved `r`, used `u`, with `u + r <= b`.
- `reserving(r)` with `r >= b` fails with a clear error rather than an empty pack.
- With an external estimator, `PackInfo::estimator` names it, and `verify` accepts a pack built under it.

## R22 — which query terms a hit matched

### What happens

`Hit { uid, score }` says how relevant, not why. In S4 one prerequisite ("Seven commands **require** an
argument, so clap rejects bare names before the router runs") was never retrieved for the five diffs that
broke it, because the diffs say `required`. Nothing in the result distinguished "retrieved weakly" from
"not retrieved at all", and the cause was found only by printing packs and reading them. 1.5.0's
`Tokenizer::folding()` fixes that case; the blindness stays.

A `check` that says "this decision was considered and judged consistent" is also more useful than one that
silently never saw it, and a finding that can name the terms behind its retrieval is auditable.

### Proposal

`Hit { uid, score, terms: Vec<(String, f32)> }` — the query terms that matched and their contribution,
under the same feature-free, deterministic rules as the score itself. Empty for a retriever that cannot
answer (the semantic backend), so the field is advisory, not a contract on every implementation.

### Acceptance

- For a BM25 hit, `terms` lists exactly the query terms with a non-zero contribution, ordered by
  contribution then term, and their sum relates to `score` by the documented formula.
- A query whose terms are all absent returns no hit, as today.
- `terms` is empty, not wrong, where a retriever does not compute per-term contributions.

## R23 — retrieval filtered by an extension schema's own kind

### What happens

`Query` filters by `KernelType` and, since 1.5.0, by uid (`within`). A corpus built on an extension
schema distinguishes its units by a payload field, not by kernel type: cargo-smysl writes `code:kind`
(`decision`, `prerequisite`, `rejected-alternative`, `consequence`, `anchor`, `reading`) under
`@schema x.code/v1`, and a rejected alternative and an anticipated consequence are both `Claim`.

`within` makes this correct but not cheap: `check` walks every unit of the store per diff to build the
eligible set, and the set is the same for every diff of that repository.

### Proposal

`Query::with_payload(key, values)` — restrict to units whose extension payload has `key` equal to one of
`values` — matched against the declared schema, so a store whose units carry no such key returns nothing
rather than everything.

### Acceptance

- A query restricted to `code:kind in {decision, prerequisite}` returns only those units, and the same
  result as `within` over the same set.
- A unit without the key is excluded.
- The restriction applies before `limit`, as `within` does, and IDF still comes from the whole index.
