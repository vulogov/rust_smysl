#!/bin/sh
# Task 4 oracle: `tests/cmd_fmt.rs` must not depend on a repository fixture's formatting state.
# Behavioural, and it reproduces the hazard the prerequisite records: cargo-mutants reuses build
# directories, so a mutant that misroutes a write can leave `F1`/`F9` canonicalised for every later
# mutant. The oracle does exactly that in a copy of the tree, then runs the cmd_fmt tests. A test
# that relies on "F1 is not canonical" fails; tests that build their own documents pass.
# Usage: 4.sh <smysl-repo-dir>    (honours CARGO_TARGET_DIR)
set -eu
src="$1"
copy=$(mktemp -d)
trap 'rm -rf "$copy"' EXIT
( cd "$src" && tar --exclude=.git -cf - . ) | ( cd "$copy" && tar -xf - )
cd "$copy"
cargo build -q -p smysl --features cli 2>/dev/null
bin="${CARGO_TARGET_DIR:-$copy/target}/debug/smysl"
"$bin" fmt --write fixtures/corpus/F1-incident.smy fixtures/corpus/F9-forward-compat.smy 2>/dev/null || true
"$bin" fmt --check fixtures/corpus/F1-incident.smy >/dev/null 2>&1 || { echo "setup failed: F1 not canonical after --write"; exit 2; }
if cargo test -q -p smysl --test cmd_fmt >/tmp/oracle4.$$ 2>&1; then
  rm -f /tmp/oracle4.$$
  echo "intact"
  exit 0
fi
echo "violated: a cmd_fmt test depends on a fixture's formatting state: $(grep -E '^test .* FAILED|panicked' /tmp/oracle4.$$ | head -2 | tr '\n' ' ')"
rm -f /tmp/oracle4.$$
exit 1
