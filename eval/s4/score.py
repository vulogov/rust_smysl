#!/usr/bin/env python3
"""S4 scoring (eval/s4-protocol.md §3–4). Development tooling only.

  score.py report RUN [RUN…]   recall by pattern, flag counts, validation drops, cost, determinism
  score.py agreed RUN_A RUN_B  the two passes' agreement: recall, flags kept, flags dropped
  score.py sheet RUN [RUN…]    blind adjudication sheet for every flag: eval/s4/results/RUN/adjudication.md,
                               diffs under …/flags/, key under eval/.s4work/keys/RUN.json
  score.py gate RUN [RUN…]     after adjudication: recall, precision and clean-commit false flags against
                               the bounds

Several RUNs are read as one result set (a set run in batches). The first RUN names the sheet and key.
"""

import json
import random
import re
import sys
import tomllib
from pathlib import Path

EVAL = Path(__file__).resolve().parent.parent
RESULTS = EVAL / "s4" / "results"
KEYS = EVAL / ".s4work" / "keys"
NOT_IN_CORPUS = {10, 12}
BOUNDS = {"recall": 0.70, "precision": 0.80, "clean_false": 0.10}


def patterns():
    text = (EVAL / "s4-protocol.md").read_text()
    return {int(m.group(1)): re.compile(m.group(2).replace("\\|", "|"), re.I)
            for m in re.finditer(r"^\| (\d+) \| `([^`]+)` \|", text, re.M)}


def agree(run_a, run_b):
    """Findings both passes report. A pass sees the same units grouped differently, so a flag only one
    pass makes is an artefact of its grouping (eval/s4-protocol.md, two passes)."""
    a, b = load([run_a]), load([run_b])
    out = {}
    for case, ra in a.items():
        rb = b.get(case)
        if rb is None:
            continue
        keep = {f["label"] for f in rb["findings"]}
        r = dict(ra)
        r["findings"] = [f for f in ra["findings"] if f["label"] in keep]
        r["dropped_by_agreement"] = [f for f in ra["findings"] if f["label"] not in keep]
        out[case] = r
    return out


def agreed(runs):
    a, b = runs[0], runs[1]
    pats = patterns()
    both = agree(a, b)
    ra, rb = load([a]), load([b])
    def tally(rows):
        contra = [r for r in rows.values() if r.get("contradicting") and r["task"] not in NOT_IN_CORPUS]
        commits = [r for r in rows.values() if r["kind"] == "commit"]
        return (sum(hit(r, pats) for r in contra), len(contra),
                sum(len(r["findings"]) for r in rows.values()),
                sum(1 for r in commits if r["findings"]), len(commits))
    for name, rows in ((a, ra), (b, rb), ("agreed", both)):
        h, n, f, cf, c = tally(rows)
        print(f"{name:<14} recall {h}/{n}  flags {f:>4}  real commits flagged {cf}/{c}")
    return both


def load(runs):
    rows = {}
    for run in runs:
        for p in sorted((RESULTS / run).glob("*.json")):
            if p.name == "params.json":
                continue
            r = json.loads(p.read_text())
            if "findings" in r:
                rows[r["id"]] = r
    return rows


def hit(r, pats):
    return any(pats[r["task"]].search(f["text"]) for f in r["findings"])


def report(runs):
    pats = patterns()
    rows = load(runs)
    contra = [r for r in rows.values() if r.get("contradicting") and r["task"] not in NOT_IN_CORPUS]
    other = [r for r in rows.values() if r.get("contradicting") and r["task"] in NOT_IN_CORPUS]
    agents_ok = [r for r in rows.values() if r["kind"] == "agent" and r.get("contradicting") is False]
    commits = [r for r in rows.values() if r["kind"] == "commit"]
    h = sum(hit(r, pats) for r in contra)
    print(f"cases scored: {len(rows)}")
    if contra:
        print(f"recall by pattern (prerequisite in corpus): {h}/{len(contra)} = {h / len(contra):.2f}")
        per = {}
        for r in contra:
            per.setdefault(r["task"], [0, 0])
            per[r["task"]][0] += hit(r, pats)
            per[r["task"]][1] += 1
        print("  by task: " + " ".join(f"{t}:{a}/{b}" for t, (a, b) in sorted(per.items())))
        missed_flagged = [r["id"] for r in contra if not hit(r, pats) and r["findings"]]
        print(f"  misses with other findings (owner may count a hit): {missed_flagged}")
    if other:
        print(f"tasks 10 and 12 (not in corpus), pattern hits: {sum(hit(r, pats) for r in other)}/{len(other)}")
    for name, group in (("agent diffs that kept the prerequisite", agents_ok), ("real commits", commits)):
        if group:
            flagged = sum(1 for r in group if r["findings"])
            print(f"{name}: {flagged}/{len(group)} flagged, {sum(len(r['findings']) for r in group)} flag(s)")
    drops = sum(len(r.get("dropped", [])) for r in rows.values())
    flags = sum(len(r["findings"]) for r in rows.values())
    print(f"flags {flags}; contradicts verdicts dropped by validation {drops}")
    pt = sum((r.get("usage") or {}).get("prompt_tokens", 0) for r in rows.values())
    ct = sum((r.get("usage") or {}).get("completion_tokens", 0) for r in rows.values())
    secs = [(r.get("usage") or {}).get("seconds", 0) for r in rows.values() if r.get("usage")]
    if secs:
        secs.sort()
        print(f"tokens: prompt {pt}, completion {ct}; median seconds per diff {secs[len(secs) // 2]:.0f}")


def sheet(runs):
    # Two runs are read as the two passes of one measurement: only agreed flags are adjudicated.
    rows = agree(runs[0], runs[1]) if len(runs) == 2 else load(runs)
    run = runs[0]
    out = RESULTS / run
    flags_dir = out / "flags"
    flags_dir.mkdir(parents=True, exist_ok=True)
    items = [(r, f) for r in rows.values() for f in r["findings"]]
    rng = random.SystemRandom()
    rng.shuffle(items)
    key = {}
    parts = [
        f"# S4 adjudication sheet ({', '.join(runs)})\n\n",
        "For each flag: does the change contradict the recorded unit? Set `verdict:` to correct, wrong or ",
        "arguable. The change is in `flags/<id>.diff`; where it came from (agent run, naive patch, real commit) ",
        "is hidden.\n",
    ]
    for i, (r, f) in enumerate(items, 1):
        fid = f"F{i:03d}"
        key[fid] = {"case": r["id"], "label": f["label"]}
        text = diff_text(r)
        (flags_dir / f"{fid}.diff").write_text(text)
        parts.append(f"\n## {fid} ({r['repo']})\n\n**Recorded {f['kind']}** `{f['label']}` (source {f['source']}):\n\n")
        parts.append("> " + f["text"].replace("\n", "\n> ") + "\n\n")
        parts.append(f"**Diff line:** `{f['diff_line']}`\n\n**Detector's reason:** {f['reason']}\n\n")
        parts.append(f"- verdict: \n")
    (out / "adjudication.md").write_text("".join(parts))
    KEYS.mkdir(parents=True, exist_ok=True)
    (KEYS / f"{run}.json").write_text(json.dumps(key, indent=2) + "\n")
    print(f"{len(items)} flag(s): {out / 'adjudication.md'}; key {KEYS / (run + '.json')}")


def diff_text(r):
    """The diff as the detector read it, without an agent run's --stat preamble."""
    import subprocess
    sets = {c["id"]: c for c in tomllib.loads((EVAL / "s4" / "sets.toml").read_text())["case"]}
    c = sets[r["id"]]
    if c["kind"] == "commit":
        return subprocess.run(["git", "-C", str(EVAL / ".repos" / c["repo"]), "show", "--format=", "--no-color",
                               "-U3", c["source"]], capture_output=True, text=True, check=True).stdout
    text = (EVAL / c["source"]).read_text()
    i = text.find("diff --git ")
    return text[i:] if i >= 0 else text


def gate(runs):
    pats = patterns()
    rows = load(runs)
    run = runs[0]
    key = json.loads((KEYS / f"{run}.json").read_text())
    text = (RESULTS / run / "adjudication.md").read_text()
    verdicts = dict(re.findall(r"^## (F\d{3}) .*?^- verdict: *(\w*)", text, re.M | re.S))
    missing = [f for f in key if verdicts.get(f, "") not in ("correct", "wrong", "arguable")]
    if missing:
        sys.exit(f"not adjudicated yet: {len(missing)} flag(s), e.g. {missing[:5]}")
    by_case = {}
    for fid, k in key.items():
        by_case.setdefault(k["case"], []).append((k["label"], verdicts[fid]))
    all_flags = [v for fl in by_case.values() for _, v in fl]
    precision = sum(v == "correct" for v in all_flags) / len(all_flags) if all_flags else 1.0
    contra = [r for r in rows.values() if r.get("contradicting") and r["task"] not in NOT_IN_CORPUS]
    hits = 0
    for r in contra:
        if hit(r, pats):
            hits += 1
    recall = hits / len(contra) if contra else 0.0
    commits = [r for r in rows.values() if r["kind"] == "commit"]
    wrong_commits = sum(1 for r in commits if any(v != "correct" for _, v in by_case.get(r["id"], [])))
    clean_false = wrong_commits / len(commits) if commits else 0.0
    print(f"recall    {recall:.2f} ({hits}/{len(contra)})  bound ≥ {BOUNDS['recall']}")
    print(f"precision {precision:.2f} ({sum(v == 'correct' for v in all_flags)}/{len(all_flags)}; arguable "
          f"{sum(v == 'arguable' for v in all_flags)})  bound ≥ {BOUNDS['precision']}")
    print(f"real commits wrongly flagged {clean_false:.2f} ({wrong_commits}/{len(commits)})  bound ≤ {BOUNDS['clean_false']}")
    met = recall >= BOUNDS["recall"] and precision >= BOUNDS["precision"] and clean_false <= BOUNDS["clean_false"]
    print("check ships as: " + ("a gate (all bounds met)" if met else "advisory (a bound missed)"))


if __name__ == "__main__":
    cmd, *runs = sys.argv[1:]
    {"report": report, "sheet": sheet, "gate": gate, "agreed": agreed}[cmd](runs)
