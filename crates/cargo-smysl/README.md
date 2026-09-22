# cargo-smysl

The cargo subcommand.

Part of [`cargo-smysl`](https://github.com/vulogov/rust_smysl), a cargo subcommand that records **why**
Rust code changed — the decisions a change makes, what had to be true for them, what was rejected — and
keeps that record honest.

## Install

```sh
cargo install cargo-smysl
cargo smysl doctor
```

Cargo runs any `cargo-<name>` binary on your `PATH` as `cargo <name>`, so this installs one binary and
gives you `cargo smysl`.

## What it does

| Command | | Needs a model |
|---|---|---|
| `doctor` | versions, workspace, corpus, backlog, whether the code parses | no |
| `facts` | deterministic facts into a regenerable cache | no |
| `extract` | record why a commit happened | **yes** |
| `why` | what rests on a claim; `--commit` reads a whole commit back | no |
| `check` | what a change contradicts — **advisory** | **yes** |
| `evidence` | a claim against the facts, and the tests that bear on it | **yes** |
| `review` | what waits for a person | no |
| `stale` | reasoning whose code has moved | no |
| `bench` | measure extraction with your own model, on your own commits | no |
| `hooks` | a post-commit hook that only queues, and a merge driver | no |

## Two things to know

**The default build has no network stack.** It speaks plain HTTP to a model on your machine; a hosted
provider over TLS is `--features hosted`, off by default.

**`check` is advisory.** Measured on held-out data: a local 14B found 0.38 of the contradictions with
0.10 precision; a hosted model, 0.75 and about 0.89. Neither justifies blocking a merge, so it exits 0
unless `--strict`. Read a finding as a place to look.

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
