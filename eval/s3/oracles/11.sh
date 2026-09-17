#!/usr/bin/env bash
# S3 oracle 11 — the residuals observations are a committed fixture that tests only read.
#
# STATIC, deliberately. The failure this prerequisite prevents is a race between two test binaries
# writing one path under std::env::temp_dir(): the original passed every local run and failed only in
# CI on the cut commit, so a behavioural run cannot expose it reliably. Instead:
#   1. Documentation/examples/observations.txt exists in the working tree, is tracked, and not ignored;
#   2. both test files that run `ephem residuals` read that committed path;
#   3. no test writes observations at run time (temp_dir + a write in the same test file).
# Exit 0 = intact, 1 = violated, 2 = harness error.
set -u
repo="${1:?usage: 11.sh <ucal repo dir>}"
cd "$repo" || exit 2
fixture=Documentation/examples/observations.txt
fail() { echo "violated: $1"; exit 1; }
[ -f "$fixture" ] || fail "$fixture is missing from the working tree"
git ls-files --error-unmatch "$fixture" >/dev/null 2>&1 || fail "$fixture is not tracked"
git check-ignore -q "$fixture" && fail "$fixture is ignored"
for t in crates/ucal/tests/json_surface.rs crates/ucal/tests/manual_fields.rs; do
  [ -f "$t" ] || exit 2
  grep -q "ephem_residuals" "$t" || continue
  grep -q '"/../../Documentation/examples/observations.txt"' "$t" || fail "$t does not read the committed fixture"
  code="$(grep -vE '^[[:space:]]*//' "$t")"   # comments may describe the old race; only code counts
  if echo "$code" | grep -q "temp_dir()" && echo "$code" | grep -qE "fs::write|File::create"; then
    fail "$t writes a file under temp_dir at run time"
  fi
done
exit 0
