#!/bin/sh
# Task 3 oracle: with no --store, `.smysl/` is resolved against the working directory.
# Behavioural: inside a git repository, a subdirectory carries its own `.smysl/config.hjson` naming a
# sentinel provider; `smysl providers` run from that subdirectory must list it. `providers` without
# --probe opens no socket.
# Usage: 3.sh <smysl-repo-dir> <smysl-binary>
set -eu
bin="$2"
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
mkdir -p "$dir/.git" "$dir/sub/.smysl"
cat > "$dir/sub/.smysl/config.hjson" <<'CFG'
{
  providers: {
    oracle-sentinel: {
      kind: ollama
      endpoint: "http://127.0.0.1:9"
      model: "none"
    }
  }
}
CFG
out=$(cd "$dir/sub" && "$bin" providers 2>&1 || true)
if echo "$out" | grep -q oracle-sentinel; then
  echo "intact"
  exit 0
fi
echo "violated: the working directory's .smysl/ was not used: $(echo "$out" | head -1)"
exit 1
