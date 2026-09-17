# smysl feature requests from rust_smysl — for 1.4.0

**For:** the smysl 1.4.0 development branch. cargo-smysl pins the latest published smysl (1.3.0) and
adopts these once 1.4.0 is released.
**Previous requests:** R1–R9, all implemented in 1.3.0 (`docs/smysl-requests.md`).

| # | Request | Kind | Priority | Affects cargo-smysl |
|---|---|---|---|---|
| R10 | Merge re-appends attestations and schema declarations | Bug (rule U) | High | yes: the corpus log grows on every merge |
| R11 | `smysl import` ignores `--format surface` | Bug | Low | no (library route) |
| R12 | `smysl import` summaries exceed the 30-token limit and fail `check` | Bug | Medium | yes: `from_csv` readings for test evidence |
| R13 | A configuration error exits 1 where the changelog says 6 | Bug | Low | no |
| R14 | An unknown provider kind still reports "malformed provider response" | Bug | Low | no |
| R15 | json-ast `GIST_MAX_CHARS` (240) disagrees with `check`'s summary bound | Bug | Low | no (verify first whether it is already fixed on 1.4.0) |

S2 (`eval/s2-protocol.md`) uses R11, R13, R14 and R15 as experiment tasks. If they are implemented
outside S2 first, S2 needs replacement tasks.

---

## R10 — Merge re-appends attestations and schema declarations

### What happens

Merging a store into a store that already holds all of its records appends every `Attestation` record
and the `SchemaDecl` again. Units, relations and label bindings are recognised as present; these two
are not.

Measured with smysl 1.3.0 on one staged batch (41 units, 33 relations, 41 label bindings, 41
attestations, 1 schema declaration, 157 records in all):

| Operation | `MergeReport.added` | Records in the store |
|---|---|---|
| `merge(empty, A)` | 157 | 157 |
| `merge(A, A)` | 42 | 199 |
| three more `merge(A, A)` | 42 each | 283 |

The 42 re-added records are exactly the 41 attestations and the schema declaration; all are
byte-identical to records already in the log.

### Where

`smysl-graph` `src/store/mod.rs`, `Store::contains` (used by `append`):

- `Record::SchemaDecl` falls through to `_ => false`, so a declaration is never recognised as present.
- `Record::Attestation` is checked as `units.get(&a.uid).is_some_and(|u| u.attestations.contains(a))`.
  The units do carry these attestations (41 attached), yet `contains` returns false for them. The cause
  is not yet isolated; a mismatch between the attached form and the record form (fields normalised on
  attach, or the uid an attestation names) is the likely place to look.

### Why it matters

Rule U promises merge is idempotent. Semantically it still is, because nothing new is reachable. But
the append-only log, and a store file on disk (`Store::path`), grows on every merge, so a corpus merged
on every commit or every CI run grows without bound. Detection is invisible from inside one store, as
the specification says of rule U.

### Reproduction

`crates/cargo-smysl-corpus/tests/guarantees.rs`, `merging_a_store_into_itself_adds_no_records`
(ignored until fixed): stage one extraction with `stage::prepare_declared`, build a `Store` from
`Staged::records()`, and merge it into a clone of itself.

### Acceptance

- `merge(A, A)` reports `added == 0` and leaves the record count unchanged, for a store holding
  attestations and schema declarations.
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
