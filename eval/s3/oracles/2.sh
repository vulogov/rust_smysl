#!/bin/sh
# Task 2 oracle: `smysl check` given no path reads the document from stdin.
# Behavioural: pipes a valid document into `check` with no path argument, in an empty directory
# (so a default store path cannot be what gets checked), and expects it to be checked and accepted.
# Usage: 2.sh <smysl-repo-dir> <smysl-binary>
set -eu
bin="$2"
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
cd "$dir"
doc='@doc smysl/1.0 { id: v/oracle }

@claim c/piped { status: speculative }
~ A document that arrives on stdin.
'
if out=$(printf '%s' "$doc" | "$bin" check 2>&1) && echo "$out" | grep -q '1 unit'; then
  echo "intact"
  exit 0
fi
echo "violated: check with no path did not check stdin: $(echo "$out" | head -2)"
exit 1
