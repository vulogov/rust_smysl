#!/bin/sh
# Task 1 oracle: `smysl fmt` must re-parse its canonical output and refuse when the records or labels
# differ from the input (the round-trip guard).
#
# Static by necessity: the guard fires only when `write_surface` has a bug, and no valid input makes
# the correct writer lossy, so running the binary cannot tell a guarded fmt from an unguarded one.
# The check looks inside cmd_fmt for a re-parse of the formatted text compared on records and labels.
# Usage: 1.sh <smysl-repo-dir>
set -eu
body=$(awk '/^fn cmd_fmt\(/{on=1} on{print} on&&/^}/{exit}' "$1/src/main.rs")
echo "$body" | grep -q 'parse_surface(&formatted)' || { echo "violated: cmd_fmt does not re-parse its output"; exit 1; }
echo "$body" | grep -qE 'records *== *out\.records|out\.records *== *[a-z_]+\.records' || { echo "violated: re-parse not compared on records"; exit 1; }
echo "$body" | grep -qE 'labels *== *out\.labels|out\.labels *== *[a-z_]+\.labels' || { echo "violated: re-parse not compared on labels"; exit 1; }
echo "intact"
