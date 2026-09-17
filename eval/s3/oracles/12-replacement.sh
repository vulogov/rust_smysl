#!/usr/bin/env bash
# S3 task 12 (replacement) oracle: a tick count is unsigned (Rule B); a divergence between clocks is
# reported as a magnitude plus a direction, never as a signed number.
#
# Behavioural. Runs `ucal doctor --clock --json` and inspects every field whose name speaks of skew,
# drift, divergence, offset or delta. Such a field breaks the prerequisite when its value starts with
# a sign, or when it carries a tick magnitude with no direction word (ahead / behind / leads / lags)
# and no sibling field naming the direction. No such field at all is intact: the base reports none.
#
# usage: 12-replacement.sh <ucal-repo> [ucal-binary]
# exit: 0 intact, 1 violated, 2 harness error
set -u
repo=${1:?usage: 12-replacement.sh <ucal-repo> [ucal-binary]}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
export HOME="$work/home"
mkdir -p "$HOME"

bin=${2:-}
if [ -z "$bin" ]; then
  target=${CARGO_TARGET_DIR:-$repo/target}
  (cd "$repo" && cargo build -q -p ucal) || { echo "oracle 12: build failed" >&2; exit 2; }
  bin="$target/debug/ucal"
fi
[ -x "$bin" ] || { echo "oracle 12: no binary at $bin" >&2; exit 2; }

"$bin" doctor --clock --json > "$work/doctor.json" 2>"$work/err" || { echo "oracle 12: doctor --clock failed: $(cat "$work/err")" >&2; exit 2; }

python3 - "$work/doctor.json" <<'EOF'
import json, re, sys
try:
    doc = json.load(open(sys.argv[1]))
except Exception as e:
    print(f"oracle 12: output is not JSON: {e}", file=sys.stderr); sys.exit(2)
if "clock_measured" not in doc:
    print("oracle 12: no clock_measured section; --clock did not measure", file=sys.stderr); sys.exit(2)

SPEAKS = re.compile(r"skew|drift|diverg|offset|delta", re.I)
DIRECTION = re.compile(r"\b(ahead|behind|leads|lags|faster|slower)\b", re.I)
bad = []

def walk(obj, path):
    if isinstance(obj, dict):
        direction_sibling = any(re.search(r"direction|ahead|behind", k, re.I) for k in obj)
        for k, v in obj.items():
            p = f"{path}.{k}" if path else k
            if SPEAKS.search(k) and isinstance(v, (str, int, float)):
                text = str(v).strip()
                if re.match(r"^[-+]\s*\d", text):
                    bad.append(f"{p} is a signed number: {text[:60]}")
                elif re.search(r"\d", text) and not DIRECTION.search(text) and not direction_sibling:
                    bad.append(f"{p} has a magnitude with no direction: {text[:60]}")
            walk(v, p)
    elif isinstance(obj, list):
        for i, v in enumerate(obj):
            walk(v, f"{path}[{i}]")

walk(doc, "")
if bad:
    for b in bad:
        print(f"oracle 12: {b}")
    sys.exit(1)
print("oracle 12: no clock divergence is reported as a signed tick count")
sys.exit(0)
EOF
