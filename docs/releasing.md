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
git tag -a v<version> -F tag-message.txt
git checkout main && git merge development/<branch>
git push origin main v<version>
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
  sleep 630     # see the rate limit below: a new crate every 10 minutes after the first five
done
```

**Test the publish, not the pipe.** `cargo publish … | tail -3` in a retry loop tests `tail`, which always
succeeds; that reported two crates published when both had been refused. Check cargo's own exit status,
and confirm against the registry rather than against the script:

```sh
curl -s -A release-check https://crates.io/api/v1/crates/cargo-smysl \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['crate']['max_version'])"
```

**crates.io rate-limits new crates, and this workspace is eight of them.** The allowance is a burst of
five, then roughly one new crate per ten minutes — and each publish consumes the refill, so the wait
restarts from the last one that succeeded rather than from the first refusal. Publishing 0.2.0 took about
forty minutes for the last three crates, the final one succeeding on its seventh attempt. It stopped
after the fifth with:

```
429 Too Many Requests: You have published too many new crates in a short period of time.
```

Nothing is lost when that happens — the crates already accepted stay accepted, and the rest publish when
the clock allows. Check what landed before resuming:

```sh
for c in cargo-smysl-git cargo-smysl-facts cargo-smysl-corpus cargo-smysl-bench \
         cargo-smysl-extract cargo-smysl-verdict cargo-smysl-evidence cargo-smysl; do
  printf "%-22s " "$c"
  curl -s "https://crates.io/api/v1/crates/$c" | python3 -c \
    "import sys,json;print(json.load(sys.stdin).get('crate',{}).get('max_version','not published'))"
done
```

Then publish only what is missing, in wave order, ten minutes apart. The limit is on *new* crates only:
later versions of a crate that already exists are not held back this way.

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

Publish from the branch whose manifests carry the version you mean, and tag that same commit. A
pre-release such as `0.2.0-dev` can be published, but `cargo install cargo-smysl` will not pick it up
without `--version 0.2.0-dev`, so it is rarely what anyone wants — 0.2.0 was published only after the
manifests said `0.2.0` rather than `0.2.0-dev`.

**Tag and merge before or immediately after publishing, not later.** For a few hours 0.2.0 was on
crates.io while the repository's only tag was `v0.1.0`, so `cargo install` and a browser gave different
answers about what this project was.

## Before publishing anything

Publishing is outward-facing and permanent. Three things are worth being sure of first:

1. **The measured figures in the README are current.** They are the first thing a reader believes, and
   they are about specific models.
2. **`cargo install --git` works from a clean clone.** It is what the documentation tells people to run
   today, and it exercises the same build.
3. **The name is the one you want.** `cargo-smysl` on crates.io is claimed on first publish and cannot be
   renamed afterwards, only abandoned.
