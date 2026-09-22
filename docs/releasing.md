# Cutting a release, and publishing to crates.io

## What a release is here

A tag, a merged `main`, and a changelog whose every figure names the model it was measured on. Publishing
to crates.io is a separate, later step, and it is the only irreversible one: a version can be yanked but
never removed.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
# then: changelog, tag, merge, push
git tag -a v0.1.0 -F tag-message.txt
git checkout main && git merge --ff-only development/<branch>
git push origin main v0.1.0
```

## Publishing

Eight crates publish; `cargo-smysl-eval` does not (`publish = false` — it is the evaluation tooling and
reads files outside its own package).

**Order matters.** crates.io resolves path dependencies by version, so a crate cannot be published before
everything it depends on. Verified from the manifests:

| Wave | Crates | Depends on |
|---|---|---|
| 1 | `cargo-smysl-git`, `cargo-smysl-facts`, `cargo-smysl-corpus` | nothing in this workspace |
| 2 | `cargo-smysl-bench`, `cargo-smysl-extract` | git; corpus |
| 3 | `cargo-smysl-verdict`, `cargo-smysl-evidence` | corpus, facts, extract |
| 4 | `cargo-smysl` | all of them |

```sh
for c in cargo-smysl-git cargo-smysl-facts cargo-smysl-corpus \
         cargo-smysl-bench cargo-smysl-extract \
         cargo-smysl-verdict cargo-smysl-evidence \
         cargo-smysl; do
  cargo publish -p "$c" --locked || break
  # crates.io needs a moment to index each one before the next can resolve it.
  sleep 30
done
```

**`cargo package` fails before the first publish, and that is expected.** Until `cargo-smysl-git` exists
on crates.io, any crate depending on it reports `no matching package named cargo-smysl-git found`. Only
wave 1 can be packaged or verified locally; the rest can be checked only as the waves land.

## What each package carries

- **Its own `README.md`.** A published package cannot reach outside itself, so each crate has a short one
  and the full documentation stays in the repository.
- **Keywords and categories** from the workspace, except `cargo-smysl` itself, which carries its own: it
  is the crate people install and they find it by searching for a cargo subcommand.
- **`rust-version = "1.86"`**, the MSRV CI checks for the shipped crates.

## The version in the manifest is the version that publishes

The workspace version is `0.2.0-dev` on the development branch and `0.1.0` at the tag. Publishing 0.1.0
means publishing from `main` at that tag, where the manifests say `0.1.0`. A pre-release version such as
`0.2.0-dev` can be published, but `cargo install cargo-smysl` will not pick it up without
`--version 0.2.0-dev`, so it is rarely what anyone wants.

## Before publishing anything

Publishing is outward-facing and permanent. Three things are worth being sure of first:

1. **The measured figures in the README are current.** They are the first thing a reader believes, and
   they are about specific models.
2. **`cargo install --git` works from a clean clone.** It is what the documentation tells people to run
   today, and it exercises the same build.
3. **The name is the one you want.** `cargo-smysl` on crates.io is claimed on first publish and cannot be
   renamed afterwards, only abandoned.
