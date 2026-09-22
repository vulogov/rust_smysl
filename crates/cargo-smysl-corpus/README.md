# cargo-smysl-corpus

Units, labels, the `x.code` schema, and the store.

Part of [`cargo-smysl`](https://github.com/vulogov/rust_smysl), a cargo subcommand that records **why**
Rust code changed — the decisions a change makes, what had to be true for them, what was rejected — and
keeps that record honest.

## Use

```rust
use cargo_smysl_corpus::{build, stage, CommitText, Extraction};
use cargo_smysl_corpus::store::Corpus;

// A model proposes content; this assigns every label, source and status.
let batch = build(&extraction, &CommitText { sha, message, files, touched }, 0)?;
let staged = stage(&store, batch, 0);
Corpus::at(".").record(sha, &staged)?;      // .smysl/commits/<sha>.smy, then the store

// Reading it back for a person: decisions with what they rest on.
let record = cargo_smysl_corpus::report::commit_record(&store, &labels, sha);
println!("{}", cargo_smysl_corpus::report::as_markdown(&record));
```

## Rules it enforces

- **Every quote is checked against the commit.** A quote that is not there caps its unit at
  `speculative`, and the batch reports how many.
- **Labels carry the commit and the run** (`d/g<sha>-1`, `p/g<sha>-1-2`), so two extractions of the same
  commit cannot collide, and a label is traceable without a lookup.
- **Prerequisites link by `conditions`, not `grounds`**, because grounds are hashed into a decision's
  uid: rewording a prerequisite would otherwise move every decision built on it.
- **Documents are the record; the store is derived.** `rebuild()` reconstructs it from the documents.

Model-free by design, and CI checks that: this crate must never depend on the model-facing ones.

## Where this sits

| | |
|---|---|
| `cargo-smysl` | the subcommand people install |
| `cargo-smysl-git` | reading commits with `gix` |
| `cargo-smysl-facts` | deterministic facts about the code |
| `cargo-smysl-corpus` | units, labels, the `x.code` schema, the store |
| `cargo-smysl-extract` | the model client, and extraction in passes |
| `cargo-smysl-verdict` | `check`, fact matching, the status policy, staleness |
| `cargo-smysl-evidence` | tests as measured evidence, the mutation gate |
| `cargo-smysl-bench` | measuring extraction against a person's labels |

Measured figures — each naming the model it was measured on — and the design rules are in the
[repository](https://github.com/vulogov/rust_smysl).

Licence: Unlicense.
