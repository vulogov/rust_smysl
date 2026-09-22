# cargo-smysl-verdict

`check`, fact matching, the status policy, and staleness.

Part of [`cargo-smysl`](https://github.com/vulogov/rust_smysl), a cargo subcommand that records **why**
Rust code changed — the decisions a change makes, what had to be true for them, what was rejected — and
keeps that record honest.

## Use

```rust
use cargo_smysl_verdict::{check, Change, Settings};
use cargo_smysl_verdict::check::{check_agreed, AGREEMENT_SEEDS};

let change = Change::from_diff(&diff, &Settings::default());
let outcome = check_agreed(&store, &change, &judge, &settings, &AGREEMENT_SEEDS);

use cargo_smysl_verdict::stale::compare;          // by item and body hash, not by file
let moved = compare(&facts_as_recorded, &facts_now);
```

## The rules

- **A single run never raises a status.** `Supported` needs every part of a claim covered by structural
  facts *and* two independent runs agreeing, or a person's word. Coverage that only adds up across runs
  is not agreement.
- **A normative rule reaches `ImplementedBy` at most.** Code can follow a rule; it cannot make one true.
- **Two passes by default.** Measured: 170 and 192 flags became 96, and it removed every false flag on a
  real commit in development. It costs recall, and that trade is why the quoted figures are the agreed
  ones.
- **Every citation is validated.** A verdict naming a unit that was not judged, or quoting a line that is
  not in the diff, is dropped and counted.

`check` is advisory: 0.10 precision on a local 14B, about 0.89 hosted, neither a reason to block a
merge.

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
