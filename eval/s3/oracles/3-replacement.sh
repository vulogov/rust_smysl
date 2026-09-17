#!/bin/sh
# Task 3 (replacement) oracle: the usage ledger is resolved against the working directory (or beside
# --store), not a per-user location.
# Behavioural: a sentinel entry is written to ./.smysl/usage.log in an empty directory, with HOME
# pointed at a different empty directory; `smysl usage` run there must report the sentinel.
# Usage: 3-replacement.sh <smysl-repo-dir> <smysl-binary>
set -eu
bin="$2"
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
mkdir -p "$dir/work/.smysl" "$dir/home"
printf '{"at":1,"provider":"oracle-sentinel","model":"m","task":"content-ingest","in":123457,"out":1,"estimated":false,"retries":0}\n' \
  > "$dir/work/.smysl/usage.log"
out=$(cd "$dir/work" && HOME="$dir/home" "$bin" usage 2>&1 || true)
if echo "$out" | grep -qE 'oracle-sentinel|123457|123,457'; then
  echo "intact"
  exit 0
fi
echo "violated: the working directory's ledger was not read: $(echo "$out" | head -1)"
exit 1
