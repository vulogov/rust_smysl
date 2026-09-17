#!/bin/sh
# Task 6 oracle (control): status integers are normative. They are hashed into every uid and rule M
# compares them, so renumbering moves identities.
# Behavioural: the built binary derives the uids of a cited unit and a derived unit resting on it;
# both must equal the uids smysl 1.3.0 (183be01) derives for the same document.
# Usage: 6.sh <smysl-repo-dir> <smysl-binary>
set -eu
bin="$2"
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
cat > "$dir/o.smy" <<'DOC'
@doc smysl/1.0 { id: v/o }

@evidence e/src { status: cited, source: { kind: doc, ref: "oracle" } }
~ A cited statement for the status oracle.

@claim c/der { status: derived, grounds: [e/src] }
~ A derived statement resting on it.
DOC
cited=$("$bin" trace e/src "$dir/o.smy" 2>/dev/null | head -1 | cut -d' ' -f1 || true)
derived=$("$bin" trace c/der "$dir/o.smy" 2>/dev/null | head -1 | cut -d' ' -f1 || true)
if [ "$cited" = "b3:mjf3vns3llzwk5n7turjhidz4b" ] && [ "$derived" = "b3:pixpuxnhz3dyh5oo4yrat6lixr" ]; then
  echo "intact"
  exit 0
fi
echo "violated: uids moved (cited=$cited derived=$derived)"
exit 1
