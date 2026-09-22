# cargo-smysl-extract

Extraction in passes, and the model client.

Part of [`cargo-smysl`](https://github.com/vulogov/rust_smysl), a cargo subcommand that records **why**
Rust code changed — the decisions a change makes, what had to be true for them, what was rejected — and
keeps that record honest.

## Use

```rust
use cargo_smysl_extract::{extract, Provider, ProviderJudge, Recipe, Source, SourceFile};

let judge = ProviderJudge { provider: Provider::local("qwen2.5-coder:14b") };
let (extraction, report) = extract(&source, &judge, &Recipe::default())?;

for warning in &report.warnings {
    eprintln!("{warning}");      // cut input, salvaged answer, parts, duplicates
}
```

`Judge` is a trait, so a pipeline is testable with no model and a caller may bring its own client.

## What it does that a plain prompt does not

- **Two passes.** Decisions first, then what surrounds each one, asked per decision — measured, asking
  for everything at once returns thin prerequisites.
- **Fitted to the model in front of it.** The commit is sized to the window the judge reports; a fixed
  size is fitted to nothing.
- **A large commit is read in parts**, each carrying the message, rather than truncated to its first
  slice. Truncating decides silently that the reasons live at the front of the diff.
- **An answer that stops part way is read up to its last whole item**, and says so. An answer that is
  malformed without being truncated is still an error.
- **A quote that is a span of the tool's own instructions is dropped.** Measured on a local 14B: it
  returned "Report at most 12 decisions." as evidence from the commit.

The client speaks plain HTTP/1.1 to a local provider and pulls in no TLS stack; `hosted` adds one.

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
