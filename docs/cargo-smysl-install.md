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

From crates.io:

```sh
cargo install cargo-smysl
```

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
invoked by cargo: ~/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo
invoked by cargo: no ($CARGO unset; run as `cargo smysl`)
```

## 4. First run

```sh
cd /path/to/your/rust/project
cargo smysl doctor
```

On this repository that prints roughly:

```
cargo-smysl: 0.2.0
smysl library: 1.6.0 (formats smysl/0.1, smysl/1.0)
code schema: x.code/v1
invoked by cargo: ~/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo
workspace: <your repository>
members: 9
corpus: <your repository>/.smysl (present)
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

## 5a. Which provider and model — measured on this repository

Every model below was actually run against `rust_smysl`'s evaluation set by this project. The last column
is what matters most in practice: how many findings `check` produced per change. The set contains a
handful of real contradictions, so **fewer is not automatically better and more is not automatically
worse** — a model finding ten things per commit is not finding ten problems, and a model finding none is
not agreeing with you.

| Provider | Model | Size on disk | Cases run | Findings per change | Verdict for this repository |
|---|---|---|---|---|---|
| ollama | **`qwen2.5-coder:14b`** | 9.0 GB | 35 | **3.9** | **The default, and the one to use.** Every figure in the README comes from it. Needs ~15 GB of RAM while loaded |
| hosted | **`deepseek-v4-pro`** | — | 35 | **1.0** | Twice the recall of the 14B and far less noise, about $2 per pass over 93 changes. Worth it when a judgement matters |
| ollama | `Qwen2.5-Coder:7B-Instruct` | 4.7 GB | 35 | 9.7 | Runs, and floods you. At a 16k window it also answered badly on ~50 units at a time and usefully on 6 — the chunking exists because of this model |
| ollama | `llama3.1:latest` | 4.9 GB | 3 | 12.7 | Over-flags worse than the 7B on the few cases tried. Not a code model |
| ollama | `deepseek-coder:6.7b` | 3.8 GB | 6 | **0.0** | Found nothing at all on six changes. Silence is not agreement |

That table is about `check`. **Extraction was measured separately, and on different models** — three
hosted systems, judged item by item against six commits the owner labelled blind:

| Provider | Model | decisions P / R | prerequisites P / R | alternatives P / R | Verdict for this repository |
|---|---|---|---|---|---|
| hosted | **Gemini pro-class** (`research-pro-v2`) | 96% / 63% | **61% / 54%** | 42% / 69% | The best all-round extractor measured here: finds most decisions and gets nearly all of them right |
| hosted | **DeepSeek** (`research-deepseek-v2`) | 82% / 67% | 41% / 59% | 31% / 69% | Finds the most of everything and is right least often. Use when you would rather sift than miss |
| hosted | **Gemini flash-class** (`research-flash-v2`) | 100% / 37% | **88% / 22%** | 79% / 52% | Says little and is almost always right. Use when the corpus must stay clean |
| ollama | `qwen2.5-coder:14b` | unjudged | unjudged | unjudged | Over-produces: on a commit labelled with 4 decisions it reported 54. Free, and the corpus shows it |

The exact Gemini and DeepSeek versions are not recorded in this repository — the research runs are kept by
system name — so treat the rows as *that class of model*, not as a version you can pin. What the three
say together is stable across versions and worth more than any one number: **decisions are the reliable
kind, prerequisites are not, and where systems differ most is recall rather than correctness.**

### Reaching Gemini

Gemini exposes an OpenAI-compatible endpoint, so it needs no separate provider — check Google's current
documentation for the exact URL and model names:

```sh
cargo install --path crates/cargo-smysl --features hosted    # hosted providers need TLS
export SMYSL_CHECK_PROVIDER=openai
export SMYSL_CHECK_MODEL=<a current pro-class model>
export SMYSL_CHECK_ENDPOINT=https://generativelanguage.googleapis.com/v1beta/openai/chat/completions
export SMYSL_CHECK_KEY_VAR=GEMINI_API_KEY
export GEMINI_API_KEY=…
export SMYSL_CHECK_CHARS_PER_TOKEN=4.0     # prose-ish tokenizer; measure yours, see below
```

`chars_per_token` is per model. Run one command and compare what the tool predicted with what the
provider charged — every run records both — then set it so the prediction is the higher of the two. A
value that is too optimistic is how a provider ends up silently dropping your system prompt.

Two things this table cannot tell you, and one it can:

- **It cannot rank precision** except for the top two. Only `qwen2.5-coder:14b` (10 correct of 96 flags,
  adjudicated) and `deepseek-v4-pro` (~89% of judged flags on the development set) have been judged
  flag-by-flag by a person. For the rest, the count is all there is.
- **It cannot tell you about your repository.** These are figures from `smysl` and `ucal` commits. Run
  `cargo smysl bench` to get yours.
- **It can tell you what not to bother with.** A 7B model on this task produces about ten findings per
  change, and `check` is already advisory at one-in-ten precision on the 14B.

### What to run for what

| | `extract` | `check` | `evidence` |
|---|---|---|---|
| **Everyday, free** | `qwen2.5-coder:14b` | `qwen2.5-coder:14b`, two passes | `qwen2.5-coder:14b` |
| **When it matters** | Gemini pro-class, or DeepSeek if you would rather sift than miss | `deepseek-v4-pro` | a hosted pro-class model |
| **When the corpus must stay clean** | Gemini flash-class: 100% / 88% precision on decisions and prerequisites, at a fifth to a third of the recall | — | — |
| **Not worth it** | 7B and below: over-produces decisions | 7B and below | — |

Extraction is the one worth paying for if you pay for anything: it is written once per commit and
everything else reads it. `check` is advisory whatever model you use.

### Hardware

`qwen2.5-coder:14b` needs about 15 GB resident. On a 24 GB machine that leaves room for a build but not
for a second model — this project lost two hours to a model runner dying under memory pressure while
another model was loaded. `ollama ps` shows what is loaded; `ollama stop <model>` frees it.

Smaller machine, three honest options: use a 7B and treat everything it says as a prompt to look; use a
hosted provider; or use only the deterministic commands (`facts`, `why`, `stale`, `review`, `bench`
scoring), which need no model at all and are most of what the tool does.

## 6. Cost and time, measured on this repository

On a local `qwen2.5-coder:14b`, 32k window, Apple silicon:

| Command | Calls | Time | Money |
|---|---|---|---|
| `doctor`, `facts`, `why`, `stale`, `review`, `bench` | 0 | seconds | none |
| `extract` of a small commit (100 KB) | 3–13 | 7–11 min | none |
| `extract` of a large commit (700 KB, read in 6 parts) | ~42 | ~29 min | none |
| `check` of one commit, two passes | 8 | ~4 min sequential, less with `--jobs` | none |
| `evidence` for one claim | 1–6 | 1–5 min | none |

The same `check` on a hosted `deepseek-v4-pro` cost about $2 per pass over 93 changes, and found twice as
many contradictions. Which trade you want is yours; the figures are in the README.

### Making `check` quicker

Measured medians for a whole `check`, sequential: **80 s** on a local 14B, **78–144 s** hosted. A hosted
model is not meaningfully faster per call — it is better and cheaper to run at scale, not quicker.

What actually shortens the wait:

| | | Costs |
|---|---|---|
| `--jobs 4` | the calls are independent, so stop waiting for them one at a time | nothing, if your provider serves more than one at once |
| `--passes 1` | one pass instead of two | the agreement filter, which halves false flags |
| `--parts 1` (the default) | do not read the whole change | the files it does not read, which it names |

**`--jobs` helps a hosted provider most.** Ollama serves one request at a time unless `OLLAMA_NUM_PARALLEL`
says otherwise, and raising it costs memory per context — on a 24 GB machine running a 14B at a 32k
window, that is the limit worth watching.

`check` now says what it is doing while it does it: one line per call as it goes out, one as it comes
back with how long it took. A minute of silence reads as a hang; it was one.

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

This repository does both in its own `ci.yml`, in a job called `corpus`, which skips quietly when no
corpus is tracked. Two ready-made workflows to copy:

- **`.github/workflows/ci.yml`**, job `corpus` — the deterministic gate: install, `doctor`, `stale --json`
  summarised in the log and kept as an artefact.
- **`.github/workflows/smysl-advisory.yml`** — what a pull request looks like when a model is available.
  It builds the pull request as one diff, runs `check --patch … --json`, and **comments**; it cannot fail
  the build. It is `workflow_dispatch` only until you set `SMYSL_MODEL`, `SMYSL_ENDPOINT` and the
  `SMYSL_API_KEY` secret, because paying for a model on every push is a cost nobody asked for.

The advisory comment names the model that judged and repeats the caveat, so a reader who has never used
the tool knows what weight to give it:

```
### `cargo smysl check`: 2 thing(s) worth a look

_Advisory. Judged by `openai:deepseek-v4-pro`; read each as a place to look, not as a claim that
something is wrong._
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
