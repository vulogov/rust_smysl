# cargo-smysl-bench

Measuring extraction against a person's labels.

Part of [`cargo-smysl`](https://github.com/vulogov/rust_smysl), a cargo subcommand that records **why**
Rust code changed — the decisions a change makes, what had to be true for them, what was rejected — and
keeps that record honest.

## Use

```rust
use cargo_smysl_bench::{extracted_items, adjudication, score, sheet};

// One commit as one document to label from, and the template to fill in.
let (sha, template, reading) = sheet::of_commit(".", "my-repo", "HEAD", 400)?;

let items = extracted_items(&extraction_json);
let adj = adjudication("v1", &labels, &items, previous.as_ref());  // `suggest` proposes; a person decides
let tallies = score(&labels, &adj)?;                               // precision and recall per kind
```

From the subcommand: `cargo smysl bench init | status | adjudicate | score`.

## Why it ships

Extraction quality is a property of the model an operator chose. The figures this project measured name
their models — decisions 82–100% precision, prerequisites 41–88%, alternatives 31–79% across three
systems — and the only figure about anyone else's model is one they measure themselves.

**Label before reading what the tool extracted.** A label written afterwards measures agreement with the
tool rather than with the commit, which is the error this instrument exists to prevent. Precision is
reported as soon as anything is paired; recall waits until nothing is pending, because recall over half
an adjudication flatters whatever was easy to pair.

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
