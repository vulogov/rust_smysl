# cargo-smysl-evidence

Tests as measured evidence, and the opt-in mutation gate.

Part of [`cargo-smysl`](https://github.com/vulogov/rust_smysl), a cargo subcommand that records **why**
Rust code changed — the decisions a change makes, what had to be true for them, what was rejected — and
keeps that record honest.

## Use

```rust
use cargo_smysl_evidence::{candidates, run, import, edges, gate, Plan};

// Deterministic shortlist: a model classifies it, it never goes looking.
let shortlist = candidates(&facts, claim, &touched_files, 5);

let readings = run(".", &Plan { tests, ..Plan::default() }, commit)?;
let imported = import(&readings, "cargo test")?;    // `measured` units, via smysl's import
let records = edges(&links, &outcomes, &proposer)?; // backs / rebuts / x.code/exercises
```

## What the measurements decided here

- **A `backs` edge waits for a person.** S1 found 3 wrong links and 1 vacuous test in 17, so a model
  proposing a link is not a review.
- **A failing test never backs anything.** If it verifies the claim and failed, that is a `rebuts` edge,
  which goes to review like any other disagreement.
- **The mutation gate is opt-in and refuses only the vacuous.** It kept 2 of 5 valid links when measured,
  so it may not reject an edge on its own; a test that notices *no* mutation of the code becomes
  `exercises` rather than `verifies`. The file is always restored, and an interrupted run's backup is
  restored at the start of the next one.
- **A cheap static check ships instead:** an assertion comparing a thing with itself cannot fail, and an
  edge resting on it is worthless.

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
