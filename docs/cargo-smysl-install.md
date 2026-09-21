# Installing cargo-smysl, and how it plugs into cargo

Everything here is done on this repository, `rust_smysl`, because that is the first project the tool is
used on.

## 1. What you need

| | Why |
|---|---|
| Rust 1.86 or later | the shipped crates' MSRV; the development crates need current stable |
| A git repository | the tool reads commits itself, with `gix` — it never runs the `git` binary |
| A model, for three of the nine commands | `extract`, `check` and `evidence`. The default is a local one, so it costs nothing |

Nothing else. The default build links no TLS stack, no async runtime and no HTTP client: it speaks plain
HTTP/1.1 to a provider on your machine, written out rather than pulled in. That is a deliberate
constraint — see D18 in the plan — and it is checked in CI.

## 2. Install

From a checkout:

```sh
git clone https://github.com/vulogov/rust_smysl
cd rust_smysl
cargo install --path crates/cargo-smysl
```

Or straight from the repository:

```sh
cargo install --git https://github.com/vulogov/rust_smysl cargo-smysl
```

To use a **hosted** provider (anything over `https://`), add the feature — it brings in a TLS stack, which
is why it is not the default:

```sh
cargo install --path crates/cargo-smysl --features hosted
```

Either way you get one binary, `cargo-smysl`, in `~/.cargo/bin`.

## 3. How cargo finds it

Cargo treats any executable named `cargo-<name>` on your `PATH` as the subcommand `cargo <name>`. So with
`cargo-smysl` installed:

```sh
cargo smysl doctor        # cargo runs cargo-smysl with "smysl" as the first argument
cargo-smysl smysl doctor  # the same thing, called directly
```

Both forms work and are tested against each other — the tool must behave identically whether cargo
invoked it or you did.

Check that cargo sees it:

```sh
cargo --list | grep smysl
#       smysl
```

External subcommands are listed by name, without a description — that is cargo's doing, not an omission
here. If it is not listed at all, `~/.cargo/bin` is not on your `PATH`.

`doctor` says which way it was invoked, because the two paths differ in one respect that matters: run
under cargo, the tool inherits `$CARGO` and uses *that* cargo for anything it runs (`cargo test` for test
evidence). Run directly, it falls back to `cargo` from your `PATH`.

```
invoked by cargo: /Users/you/.rustup/toolchains/1.94.1-aarch64-apple-darwin/bin/cargo
invoked by cargo: no ($CARGO unset; run as `cargo smysl`)
```

## 4. First run

```sh
cd /path/to/your/rust/project
cargo smysl doctor
```

On this repository that prints roughly:

```
cargo-smysl: 0.1.0
smysl library: 1.6.0 (formats smysl/0.1, smysl/1.0)
code schema: x.code/v1
invoked by cargo: /Users/you/.rustup/toolchains/1.94.1-aarch64-apple-darwin/bin/cargo
workspace: /Users/you/Src/rust_smysl
members: 9
corpus: /Users/you/Src/rust_smysl/.smysl (present)
facts: 50 Rust file(s), 485 function(s), 0 parse failure(s)
```

`doctor` calls no model and writes nothing. It answers four questions: is the binary the version you
think, does cargo's idea of the workspace match yours, is there a corpus, and does your code parse with
the same `syn` the facts use.

Any command takes `--manifest-path` when you want to work on a workspace other than the current
directory's:

```sh
cargo smysl doctor --manifest-path ../other-project/Cargo.toml
```

## 5. Pointing it at a model

Three commands ask a model. Every part of that is a setting, with flags and environment variables, so a
team can put the choice in one place.

```sh
export SMYSL_CHECK_PROVIDER=ollama                        # or `openai` for any OpenAI-compatible endpoint
export SMYSL_CHECK_MODEL=qwen2.5-coder:14b
export SMYSL_CHECK_ENDPOINT=http://localhost:11434/api/chat
export SMYSL_CHECK_WINDOW=32768                           # context window, prompt and answer together
export SMYSL_CHECK_CHARS_PER_TOKEN=2.0                    # your model's rate; code runs about 2 on Qwen
export SMYSL_CHECK_PASSES=2                               # two passes, keeping only what both report
export SMYSL_CHECK_PROMPT_FILE=./my-prompt.txt            # optional: replace the built-in judgement prompt
```

A local provider, start to finish:

```sh
brew install ollama                       # or your platform's package
ollama serve &
ollama pull qwen2.5-coder:14b             # about 9 GB
cargo smysl extract HEAD
```

A hosted provider, with the `hosted` feature installed:

```sh
export SMYSL_CHECK_PROVIDER=openai
export SMYSL_CHECK_MODEL=deepseek-v4-pro
export SMYSL_CHECK_ENDPOINT=https://api.deepseek.com/chat/completions
export SMYSL_CHECK_KEY_VAR=DEEPSEEK_API_KEY   # the *name* of the variable holding the key, not the key
export DEEPSEEK_API_KEY=sk-…
```

`--key-var` names the environment variable rather than taking the key, so a key never appears in a command
line, a shell history or a run's record.

**`chars_per_token` matters more than it looks.** It is how the tool predicts whether a prompt fits. Set
it too high and the provider silently keeps the last N tokens and drops your system prompt — which is
exactly what happened here on a 16k window, visible only in Ollama's own log. Every run records predicted
against charged tokens, and a call the provider truncated is an error rather than an answer.

## 6. Cost and time, measured on this repository

On a local `qwen2.5-coder:14b`, 32k window, Apple silicon:

| Command | Calls | Time | Money |
|---|---|---|---|
| `doctor`, `facts`, `why`, `stale`, `review`, `bench` | 0 | seconds | none |
| `extract` of a small commit (100 KB) | 3–13 | 7–11 min | none |
| `extract` of a large commit (700 KB, read in 6 parts) | ~42 | ~29 min | none |
| `check` of one commit, two passes | 8 | ~4 min | none |
| `evidence` for one claim | 1–6 | 1–5 min | none |

The same `check` on a hosted `deepseek-v4-pro` cost about $2 per pass over 93 changes, and found twice as
many contradictions. Which trade you want is yours; the figures are in the README.

## 7. Using it in CI

`check` is advisory by default — it prints findings and exits 0 — because on a free local model about one
flag in ten was correct. To make it block, ask for that explicitly:

```yaml
- name: what does this change contradict?
  run: cargo smysl check --strict        # exits 5 when there are findings
```

Before wiring that into a pipeline that can fail, measure the model you will run it with
(`cargo smysl bench`) and read §"What has been measured" in the README. A gate that is wrong nine times in
ten teaches people to ignore it.

Deterministic commands are safe to gate on today, because they involve no model at all:

```yaml
- run: cargo smysl doctor               # the corpus loads, the code parses
- run: cargo smysl stale --json         # what rests on code that has moved
```

## 8. Uninstalling, and what is left behind

```sh
cargo uninstall cargo-smysl
```

The corpus stays in your repository under `.smysl/`. `.smysl/facts/` is a cache and can be deleted at any
time; `.smysl/commits/` and `.smysl/store.cbor` are the record. The store is derived from the documents,
so the documents are the thing to keep.

## 9. Troubleshooting

| What you see | What it means |
|---|---|
| `cargo metadata exited with an error: current package believes it's in a workspace when it's not` | you are inside a directory with a `Cargo.toml` that is not part of the workspace; pass `--manifest-path` |
| `no corpus recorded yet` | nothing has been extracted; run `cargo smysl extract <rev>` |
| `connecting to localhost:11434: Connection refused` | the provider is not running (`ollama serve`) |
| `ollama answered 500: model runner has unexpectedly stopped` | the machine ran out of memory for the model; stop other models (`ollama stop <model>`) or use a smaller one |
| `the commit is N characters and this model takes about M; it was read in K parts` | expected on large commits — it is telling you the reading was split, not truncated |
| `a call answered in an unreadable shape` | the model returned something that was not the JSON asked for; the run continues and says the call found nothing |
| exit code 5 | `check --strict` found something |

## Next

[`docs/smysl-workflow.md`](smysl-workflow.md) walks every command on this repository, in the order you
would actually use them.
