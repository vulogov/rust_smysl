#!/usr/bin/env python3
"""Run the shipped `cargo smysl check` over exported S4 cases (Phase 3's first "done when").

  reproduce.py RUN [--set heldout] [--only contradicting|commits|all] [--limit N] [--passes N]

Cases are the trees `smysl-eval s4-export` wrote: the corpus the harness used, and the same change.
What differs is the code doing the checking — the shipped binary rather than eval/src/s4.rs — so a
difference in the figures is a difference in `check`. Results land in eval/s4/results/RUN in the shape
score.py reads, and an existing result is kept, so a run can be resumed.
"""

import json
import os
import subprocess
import sys
import time
from pathlib import Path

EVAL = Path(__file__).resolve().parent.parent
ROOT = EVAL.parent
# The release binary: a reproduction should not be measuring a debug build, and it must not be rebuilt
# while the run is going — the binary is what is being measured.
BINARY = Path(os.environ.get("SMYSL_BINARY", ROOT / "target" / "release" / "cargo-smysl"))


def main(argv):
    run = argv[0]
    opts = dict(zip(argv[1::2], argv[2::2]))
    export = EVAL / ".s4work" / "export"
    only = opts.get("--only", "all")
    passes = opts.get("--passes", "2")
    limit = int(opts.get("--limit", "0"))
    out = EVAL / "s4" / "results" / run
    out.mkdir(parents=True, exist_ok=True)

    cases = []
    for d in sorted(export.iterdir()):
        case = json.loads((d / "case.json").read_text())
        if case["set"] != opts.get("--set", "heldout"):
            continue
        if only == "contradicting" and not case.get("contradicting"):
            continue
        if only == "commits" and case["kind"] != "commit":
            continue
        cases.append((d, case))
    if limit:
        cases = cases[:limit]
    print(f"{len(cases)} case(s), {passes} pass(es) each, into {out}")

    for n, (d, case) in enumerate(cases, 1):
        path = out / f"{case['id']}.json"
        if path.exists():
            continue
        began = time.time()
        p = subprocess.run(
            [str(BINARY), "smysl", "check", "--patch", "change.diff", "--json",
             "--passes", passes],
            cwd=d, capture_output=True, text=True)
        try:
            outcome = json.loads(p.stdout)
        except json.JSONDecodeError:
            print(f"  {case['id']}: no JSON ({p.stderr.strip()[:200]})")
            continue
        # A case whose calls failed found nothing for a reason that has nothing to do with `check`.
        # Writing it would score a dead provider as a clean commit, so it is left for the next run.
        failed = [w for w in outcome.get("warnings", []) if w.startswith("a call failed")]
        if failed:
            print(f"  {case['id']}: {len(failed)} call(s) failed, not recorded — {failed[0][:120]}")
            continue
        result = dict(case)
        result.update(outcome)
        result["seconds"] = round(time.time() - began, 1)
        result["binary"] = str(BINARY)
        path.write_text(json.dumps(result, indent=2))
        print(f"  {n}/{len(cases)} {case['id']}: {len(outcome.get('findings', []))} finding(s), "
              f"{result['seconds']}s")


if __name__ == "__main__":
    main(sys.argv[1:])
