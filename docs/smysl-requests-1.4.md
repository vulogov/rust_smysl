# smysl feature requests from rust_smysl — for 1.4.0

**For:** the smysl 1.4.0 development branch. cargo-smysl pins the latest published smysl (1.3.0) and
adopts these once 1.4.0 is released.
**Previous requests:** R1–R9, all implemented in 1.3.0 (`docs/smysl-requests.md`).

| # | Request | Kind | Priority | Affects cargo-smysl |
|---|---|---|---|---|
| R10 | Merge re-appends label bindings and schema declarations | Bug (rule U) | High | yes: the corpus log grows on every merge. **Fixed on dev/1.4.0 (5c64e02); acceptance passed** |
| R11 | `smysl import` ignores `--format surface` | Bug | Low | no (library route). Still open at c7bf5a3 |
| R12 | `smysl import` summaries exceed the 30-token limit and fail `check` | Bug | Medium | yes: `from_csv` readings for test evidence. Still open at c7bf5a3 |
| R13 | A configuration error exits 1 where the changelog says 6 | Bug | Low | no. Still open at c7bf5a3 |
| R14 | An unknown provider kind still reports "malformed provider response" | Bug | Low | no. Still open at c7bf5a3 |
| R15 | json-ast `GIST_MAX_CHARS` (240) disagrees with `check`'s summary bound | Bug | Low | no. Fixed (120, test-held) |

Acceptance against dev/1.4.0 at c7bf5a3: `docs/smysl-1.4.0-acceptance.md`.

S2 (`eval/s2-protocol.md`) uses R11, R13, R14 and R15 as experiment tasks. If they are implemented
outside S2 first, S2 needs replacement tasks.

---

## R10 — Merge re-appends label bindings and schema declarations

### What happens

Merging a store into a store that already holds all of its records appends every `LabelBinding` and
`SchemaDecl` again. Units, attestations and relations are recognised as present; these two are not.

Measured on one staged batch (41 units, 33 relations, 41 label bindings, 41 attestations, 1 schema
declaration, 157 records in all), identically on 1.3.0 and on dev/1.4.0 at `09271ab`:

| Operation | `MergeReport.added` | Records in the store |
|---|---|---|
| `merge(empty, A)` | 157 | 157 |
| `merge(A, A)` | 42 | 199 |
| three more `merge(A, A)` | 42 each | 283 |

The 42 re-appended records, by kind: **41 `LabelBinding`, 1 `SchemaDecl`**. A batch without labels
re-appends only its `SchemaDecl`. (An earlier version of this request blamed attestations. It was
wrong: all 41 attestation records are recognised as present.)

### Where

`smysl-graph` `src/store/mod.rs`, `Store::contains`, which `append` (and so `merge`) uses to skip
records already present. It has arms for `Unit`, `Attestation`, `Relation`, `Thread`, `View`,
`Contention`, and on 1.4.0 `Withdrawal` and `Resolution`. **`SchemaDecl`, `LabelBinding`, `PackInfo`
and unknown records fall through to `_ => false`**, so they are never recognised as present.

Likely the same gap in 1.4.0's own addition: an attestation whose uid is a relation id is checked with
`units.get(&a.uid)`, which finds no unit, so re-merging an edge attestation would append it again.

### Why it matters

Rule U promises merge is idempotent. Semantically it still is, because nothing new is reachable. But
the append-only log, and a store file on disk (`Store::path`), grows on every merge, so a corpus merged
on every commit or every CI run grows without bound. A label binding merged twice is also twice the
input for label-collision detection.

### Reproduction

`crates/cargo-smysl-corpus/tests/guarantees.rs`, `merging_a_store_into_itself_adds_no_records`
(ignored until fixed): stage one extraction with `stage::prepare_declared`, build a `Store` from
`Staged::records()`, and merge it into a clone of itself.

### Acceptance

- `merge(A, A)` reports `added == 0` and leaves the record count unchanged, for a store holding every
  record type: units, attestations on units and on relations, relations, threads, views, contentions,
  pack info, schema declarations, label bindings, withdrawals, resolutions, and an unknown record.
- A label bound to a *different* uid is still appended (it is a distinct record, and label-collision
  detection needs it).
- The same holds for a store built by `Store::open` from a file and merged with its own contents.

---

## R11 — `smysl import` ignores `--format surface`

`smysl import --format surface …` and `smysl --format surface import …` both write CBOR. Other commands
honour the flag. **Acceptance:** with `--format surface`, `import` writes surface text that `fmt` round
trips.

## R12 — `smysl import` summaries exceed the 30-token limit

`from_csv` puts every column into a unit's gist ("test <name>, commit <sha>: outcome passed,
run_seconds 0.1, toolchain …"). Wide rows, and even narrow rows with long test names, exceed the
30-token L0 bound, so imported units fail `smysl check` with `E022`. Measured: 48–62 tokens with seven
columns; 31–33 tokens with three columns and long test names.

**Proposal:** key columns (`--key`) in the gist, trimmed to the bound; all columns in the body and the
payload, as now.

**Acceptance:** importing a CSV with a 100-character key column produces units that check with no
`E022`, with every cell still in the payload.

## R13 — Configuration error exit code

A bad `ingest.path` in `.smysl/config.hjson` now gives a clear message but exits 1; the 1.3.0 changelog
says configuration errors exit 6. **Acceptance:** exit 6, as documented.

## R14 — Unknown provider kind message

An unknown provider kind still prints "malformed provider response: provider kind `…` is not compiled
into this build" before the configuration error. **Acceptance:** one configuration error naming the
provider and the kind, with no "malformed provider response".

## R15 — Summary bound mismatch

The json-ast schema allows `GIST_MAX_CHARS` = 240 while `check` enforces a 120-byte summary bound, so a
schema-valid answer can fail `check`. `f740474` ("gist bound") may already address it; verify on the
1.4.0 branch before implementing. **Acceptance:** the schema's bound and `check`'s agree.
