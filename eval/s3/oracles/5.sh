#!/bin/sh
# Task 5 oracle: a bare `smysl trace` is refused by the argument parser before the router runs.
# tests/dispatch.rs relies on this: its NOT_REACHED marker ("required arguments were not provided")
# is how it tells "clap refused the invocation" apart from "the command is not wired".
# Behavioural: run `trace` with no arguments and no stdin, in an empty directory.
# Usage: 5.sh <smysl-repo-dir> <smysl-binary>
set -eu
bin="$2"
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
out=$(cd "$dir" && "$bin" trace </dev/null 2>&1 || true)
if echo "$out" | grep -q "required arguments were not provided"; then
  echo "intact"
  exit 0
fi
echo "violated: bare trace reached the router: $(echo "$out" | head -1)"
exit 1
