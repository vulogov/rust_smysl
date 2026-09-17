# cargo-smysl

A cargo subcommand that records **why** Rust code changed (the decisions a change makes, what had
to be true for them, what was rejected, what follows) as a [smysl](https://crates.io/crates/smysl)
corpus, and keeps that reasoning honest with deterministic facts about the code and test results as
evidence.

**Status:** scaffold. `cargo smysl doctor` works; every other command reports the plan phase that
implements it. See [`docs/implementation-plan.md`](docs/implementation-plan.md).

## Install

```sh
cargo install --path crates/cargo-smysl      # from a checkout
cargo install --git https://github.com/vulogov/rust_smysl cargo-smysl
```

It installs one binary, `cargo-smysl`, which cargo runs as a subcommand. It is self-contained:
smysl and syn are linked as libraries, and it needs no other tools at runtime.

## Use

```sh
cargo smysl doctor                  # versions, workspace, corpus, and whether the code parses
cargo smysl --help
cargo smysl doctor --manifest-path path/to/Cargo.toml
```

| Command | What it will do | Plan |
|---|---|---|
| `doctor` | Report what the tool sees | done |
| `why <item>` | Explain an item from the corpus | Phase 1 |
| `check` | Check the corpus | Phase 1 |
| `facts [rev]` | Extract deterministic facts | Phase 2 |
| `extract [rev]` | Extract decisions, prerequisites, alternatives, consequences | Phase 2 |
| `evidence [rev]` | Link tests, run them at the commit, record measured evidence | Phase 3 |
| `review` | Work through verdicts waiting for a person | Phase 3 |
| `stale [range]` | Report reasoning whose code has moved | later |

Exit codes: `0` success, `1` failure, `2` usage error, `3` not implemented yet.

## Workspace

| Crate | Role | Model |
|---|---|---|
| `cargo-smysl` | the subcommand | — |
| `cargo-smysl-corpus` | units, labels, `x.code/v1`, smysl staging and queries | free |
| `cargo-smysl-facts` | deterministic syntactic facts (syn) | free |
| `cargo-smysl-extract` | decisions and prerequisites from commits | uses |
| `cargo-smysl-verdict` | fact matching and status policy | uses |
| `cargo-smysl-evidence` | tests as measured evidence (candidate tests are deterministic; linking uses a model) | uses |

## Licence

Unlicense.
