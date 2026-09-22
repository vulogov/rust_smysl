# cargo-smysl-git

Reading commits, with `gix` rather than the `git` binary.

Part of [`cargo-smysl`](https://github.com/vulogov/rust_smysl), a cargo subcommand that records **why**
Rust code changed — the decisions a change makes, what had to be true for them, what was rejected — and
keeps that record honest.

## Use

```rust
use cargo_smysl_git::{read_commit, commits_between};

let commit = read_commit(".", "HEAD")?;
println!("{}", commit.message);
for file in &commit.files {
    // `before` and `after` are the file's text either side of the commit;
    // `None` when added, deleted, binary, or larger than MAX_FILE_BYTES.
    println!("{} {}", file.path, file.after.is_some());
}

// Newest first: what HEAD reaches that v1.0.0 does not.
let range = commits_between(".", Some("v1.0.0"), "HEAD", 0)?;
```

## Why it is a crate

Because the tool reads git itself. Sources, labels and statuses in the corpus are the tool's own reading
of the commit, never a model's account of it, and shelling out to `git` would make the tool depend on
what is installed where it runs.

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
