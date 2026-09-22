# cargo-smysl-facts

Deterministic facts about Rust code.

Part of [`cargo-smysl`](https://github.com/vulogov/rust_smysl), a cargo subcommand that records **why**
Rust code changed — the decisions a change makes, what had to be true for them, what was rejected — and
keeps that record honest.

## Use

```rust
use cargo_smysl_facts::{facts, Fact, Cache};

for fact in facts("src/lib.rs", source)? {
    if let Fact::Function(f) = fact {
        // What it is, what it calls, what its control context is, its `cfg`,
        // and a hash of its body so a function that only moved is recognised.
        println!("{} at {}:{} {}", f.label(), f.file, f.line, f.body_hash);
    }
}

// Cached by file bytes and extractor version; delete the cache freely.
let cache = Cache::at(".");
let facts = cache.facts_of("src/lib.rs", source)?;
```

`select` and `render` build the scope a model is shown: the items a change touched, anything those names
mention, and one hop along calls.

## The rule this crate exists for

**Author prose never verifies anything.** Doc comments, string literals and string constants are facts
about what someone *wrote*, and they are tagged as prose so a claim covered only by them can never come
back supported. Measured: every one of those laundered support until it was tagged.

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
