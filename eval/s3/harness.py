#!/usr/bin/env python3
"""S3 run harness (eval/s3-protocol.md). Development tooling only; never part of the shipped tool.

  harness.py prepare              clone each repository at its base, build, run its tests and every oracle
  harness.py smoke                apply each task's naive patch and score it: the scorer must see what
                                  validation saw (non-control: tests pass, oracle violated)
  harness.py run [--jobs N] [--model M] [--tasks 1,2] [--arms control,corpus] [--reps N] RUN_ID
                                  one agent per task, arm and repetition, scored
  harness.py report RUN_ID        violations per arm, and the gate
  harness.py answer [--model M] RUN_ID
                                  answer each question twice: from its packed corpus context only, and
                                  from the repository's git history and code only
  harness.py sheet RUN_ID         a blind judging sheet (answers as A and B in random order); the key is
                                  kept under eval/.s3work until judging is done

Everything a test, an oracle or an agent runs sees HOME set to a scratch directory. Work trees and target
directories live under eval/.s3work (ignored); results under eval/s3/results/RUN_ID (committed).
"""

import argparse
import concurrent.futures as cf
import json
import os
import shutil
import subprocess
import sys
import time
import tomllib
from pathlib import Path

S3 = Path(__file__).resolve().parent
EVAL = S3.parent
WORK = EVAL / ".s3work"
REG = tomllib.loads((S3 / "tasks.toml").read_text())
REAL_HOME = Path(os.environ["HOME"])

CORPUS_PREAMBLE = (
    "The project keeps a corpus of the decisions, prerequisites, rejected alternatives and consequences "
    "recorded for its code, extracted from its commit history. The units relevant to this task, packed "
    "within a fixed budget (each unit: [label] kind (status, source), its text, and its edges to other "
    "units):\n\n"
)


def task(n):
    return next(t for t in REG["task"] if t["n"] == n)


def scratch_home(root):
    """A HOME that holds nothing of the user's except the login keychain, which the agent CLI needs."""
    home = root / "home"
    home.mkdir(parents=True, exist_ok=True)
    lib = home / "Library"
    if sys.platform == "darwin" and not lib.exists():
        lib.symlink_to(REAL_HOME / "Library")
    return home


def env_for(home, target):
    env = dict(os.environ)
    env.update(
        HOME=str(home),
        CARGO_HOME=os.environ.get("CARGO_HOME", str(REAL_HOME / ".cargo")),
        RUSTUP_HOME=os.environ.get("RUSTUP_HOME", str(REAL_HOME / ".rustup")),
        CARGO_TARGET_DIR=str(target),
        CARGO_TERM_COLOR="never",
    )
    return env


def sh(cmd, cwd, env, log, timeout=3600):
    t0 = time.time()
    try:
        p = subprocess.run(cmd, shell=True, cwd=cwd, env=env, capture_output=True, text=True, timeout=timeout)
        code, out = p.returncode, p.stdout + p.stderr
    except subprocess.TimeoutExpired as e:
        code, out = 124, f"timeout after {timeout}s\n{e.stdout or ''}{e.stderr or ''}"
    with open(log, "a") as f:
        f.write(f"$ {cmd}\n[exit {code}, {time.time() - t0:.0f}s]\n{out[-20000:]}\n")
    return code, out


def clone_copy(src, dst):
    """Copy-on-write copy where the filesystem supports it (APFS), a plain copy otherwise."""
    dst.parent.mkdir(parents=True, exist_ok=True)
    if shutil.which("cp") and sys.platform == "darwin":
        subprocess.run(["cp", "-c", "-R", str(src), str(dst)], check=True)
    else:
        shutil.copytree(src, dst, symlinks=True)


def score(n, tree, target, out_dir):
    """Build, run the tests, run the oracle. Writes score.json and returns it."""
    t = task(n)
    repo = REG["repos"][t["repo"]]
    home = scratch_home(out_dir)
    env = env_for(home, target)
    log = out_dir / "score.log"
    log.unlink(missing_ok=True)
    result = {"task": n, "control": t["control"]}
    code, _ = sh(repo["build"], tree, env, log)
    result["build"] = code == 0
    tests = []
    for cmd in repo["tests"]:
        code, out = sh(cmd, tree, env, log)
        tests.append({"cmd": cmd, "exit": code, "failed": [l for l in out.splitlines() if l.endswith("FAILED")][:10]})
    result["tests"] = tests
    result["tests_pass"] = result["build"] and all(x["exit"] == 0 for x in tests)
    script, *args = t["oracle"]
    argv = [str(S3 / "oracles" / script)] + [
        str(tree) if a == "repo" else str(target / repo["binary"]) if a == "bin" else a for a in args
    ]
    code, out = sh(" ".join(f"'{a}'" for a in argv), tree, env, log)
    result["oracle_exit"] = code
    result["oracle_out"] = out.strip()[-600:]
    result["oracle"] = {0: "intact", 1: "violated"}.get(code, "harness-error")
    result["violation"] = result["tests_pass"] and code == 1
    (out_dir / "score.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


def base_tree(repo):
    return WORK / "base" / repo / "tree", WORK / "base" / repo / "target"


def fresh(repo, dst):
    """A work tree and target directory copied from the prepared base."""
    tree, target = base_tree(repo)
    if dst.exists():
        shutil.rmtree(dst)
    clone_copy(tree, dst / "tree")
    clone_copy(target, dst / "target")
    return dst / "tree", dst / "target"


def prepare(_args):
    for name, repo in REG["repos"].items():
        tree, target = base_tree(name)
        if not tree.exists():
            tree.parent.mkdir(parents=True, exist_ok=True)
            subprocess.run(["git", "clone", "--quiet", str(EVAL / ".repos" / name), str(tree)], check=True)
            subprocess.run(["git", "-C", str(tree), "checkout", "--quiet", "--detach", repo["base"]], check=True)
        print(f"{name}: building and testing the base", flush=True)
    for t in REG["task"]:
        tree, target = base_tree(t["repo"])
        out = WORK / "base-scores" / f"task-{t['n']}"
        out.mkdir(parents=True, exist_ok=True)
        r = score(t["n"], tree, target, out)
        ok = r["tests_pass"] and r["oracle"] == "intact"
        print(f"task {t['n']:>2} base: tests {'pass' if r['tests_pass'] else 'FAIL'}, oracle {r['oracle']}  {'ok' if ok else 'NOT OK'}", flush=True)


def smoke(_args):
    bad = 0
    for t in REG["task"]:
        patch = next((S3 / "validation" / "patches" / f"{t['n']}{s}.patch" for s in ("-replacement", "")
                      if (S3 / "validation" / "patches" / f"{t['n']}{s}.patch").exists()))
        dst = WORK / "smoke" / f"task-{t['n']}"
        tree, target = fresh(t["repo"], dst)
        subprocess.run(["git", "-C", str(tree), "apply", str(patch)], check=True)
        r = score(t["n"], tree, target, dst)
        expect = "tests fail or oracle intact" if t["control"] else "tests pass, oracle violated"
        ok = (not r["violation"]) if t["control"] else r["violation"]
        bad += not ok
        print(f"task {t['n']:>2} naive ({patch.name}): tests {'pass' if r['tests_pass'] else 'fail'}, "
              f"oracle {r['oracle']}; expected {expect}  {'ok' if ok else 'NOT OK'}", flush=True)
        shutil.rmtree(dst / "target", ignore_errors=True)
    sys.exit(1 if bad else 0)


def prompt(t, arm):
    tests = " and ".join(f"`{c}`" for c in REG["repos"][t["repo"]]["tests"])
    text = (
        f"You are working in a checkout of the `{t['repo']}` Rust repository (the current directory).\n\n"
        f"Task: {t['statement']}\n\n"
        f"Implement the task completely, with tests where they belong, and make sure {tests} pass. "
        "Do not commit.\n"
    )
    if arm == "corpus":
        context = (S3 / "context" / f"task-{t['n']}.txt").read_text()
        text += "\n" + CORPUS_PREAMBLE + context
    return text


def outcome(r):
    """Run2 outcome (eval/s3-protocol.md, "Run2 amendments")."""
    if r["oracle"] == "violated":
        return "violated"
    if r["oracle"] == "harness-error":
        return "harness-error"
    return "stopped" if r.get("diff_lines", 0) == 0 else "intact"


def run_one(run_id, n, arm, model, rep=None):
    t = task(n)
    name = f"task-{n}-{arm}" if rep is None else f"task-{n}-{arm}-r{rep}"
    out = S3 / "results" / run_id / name
    if (out / "score.json").exists():
        return json.loads((out / "score.json").read_text())
    out.mkdir(parents=True, exist_ok=True)
    dst = WORK / "runs" / run_id / name
    tree, target = fresh(t["repo"], dst)
    text = prompt(t, arm)
    (out / "prompt.txt").write_text(text)
    env = env_for(scratch_home(dst), target)
    t0 = time.time()
    p = subprocess.run(
        ["claude", "-p", text, "--model", model, "--output-format", "json",
         "--permission-mode", "acceptEdits", "--permission-prompts", "none",
         "--allowedTools", "Bash,Read,Edit,Write,Glob,Grep",
         "--disallowedTools", "WebFetch,WebSearch,Agent",
         "--strict-mcp-config", "--no-session-persistence"],
        cwd=tree, env=env, capture_output=True, text=True, timeout=7200,
    )
    (out / "agent.json").write_text(p.stdout)
    if p.stderr:
        (out / "agent.stderr").write_text(p.stderr)
    subprocess.run(["git", "-C", str(tree), "add", "-A"], check=True)
    diff = subprocess.run(["git", "-C", str(tree), "diff", "--cached", "--stat", "-p"],
                          capture_output=True, text=True).stdout
    (out / "diff.patch").write_text(diff)
    subprocess.run(["git", "-C", str(tree), "reset", "--quiet"], check=True)
    r = score(n, tree, target, out)
    if r["oracle"] == "harness-error":
        r = score(n, tree, target, out)
        r["oracle_rerun"] = True
    try:
        a = json.loads(p.stdout)
        r["agent"] = {"is_error": a.get("is_error"), "turns": a.get("num_turns"),
                      "cost_usd": a.get("total_cost_usd"), "seconds": round(time.time() - t0)}
    except json.JSONDecodeError:
        r["agent"] = {"is_error": True, "seconds": round(time.time() - t0)}
    r["arm"] = arm
    if rep is not None:
        r["rep"] = rep
    r["diff_lines"] = sum(1 for l in diff.splitlines() if l[:1] in "+-" and l[:3] not in ("+++", "---"))
    r["outcome"] = outcome(r)
    (out / "score.json").write_text(json.dumps(r, indent=2) + "\n")
    shutil.rmtree(dst / "target", ignore_errors=True)
    return r


def run(args):
    tasks = [int(x) for x in args.tasks.split(",")] if args.tasks else [t["n"] for t in REG["task"]]
    arms = args.arms.split(",")
    reps = [None] if args.reps is None else list(range(1, args.reps + 1))
    # Interleaved by repetition, so a run stopped part way leaves whole pairs rather than whole tasks.
    jobs = [(n, a, k) for k in reps for n in tasks for a in arms]
    with cf.ThreadPoolExecutor(args.jobs) as ex:
        futs = {ex.submit(run_one, args.run_id, n, a, args.model, k): (n, a) for n, a, k in jobs}
        for f in cf.as_completed(futs):
            n, a = futs[f]
            try:
                r = f.result()
                print(f"task {n:>2} {a:<7} r{r.get('rep', '-')} {r.get('outcome', '')}: tests {'pass' if r['tests_pass'] else 'fail'}, oracle {r['oracle']}, "
                      f"violation {r['violation']}, diff {r.get('diff_lines')} lines, agent {r.get('agent')}", flush=True)
            except Exception as e:  # noqa: BLE001 - report and continue with the other runs
                print(f"task {n:>2} {a:<7} harness error: {e}", flush=True)


def report(args):
    root = S3 / "results" / args.run_id
    rows = [json.loads(p.read_text()) for p in sorted(root.glob("task-*/score.json"))]
    if any("rep" in r for r in rows):
        return report_run2(rows)
    v = {"control": 0, "corpus": 0}
    control_violations = 0
    print(f"{'task':>4} {'ctl':>3} {'arm':<7} {'tests':<5} {'oracle':<13} {'violation':<9} {'diff':>5} {'cost':>6}")
    for r in sorted(rows, key=lambda r: (r["task"], r["arm"])):
        cost = (r.get("agent") or {}).get("cost_usd") or 0
        print(f"{r['task']:>4} {'yes' if r['control'] else '':>3} {r['arm']:<7} {'pass' if r['tests_pass'] else 'fail':<5} "
              f"{r['oracle']:<13} {str(r['violation']):<9} {r.get('diff_lines', 0):>5} {cost:>6.2f}")
        if r["control"]:
            control_violations += r["violation"]
        else:
            v[r["arm"]] += r["violation"]
    print(f"\nnon-control violations: control arm {v['control']}, corpus arm {v['corpus']}")
    print(f"violations on control tasks: {control_violations} (any invalidates the run)")
    print("task half of the gate: " + ("met" if v["corpus"] * 2 <= v["control"] and v["control"] > 0 else
                                        "not met" if v["control"] > 0 else "undecided (no violations without the corpus)"))


ANSWER_RULES = (
    "Answer in plain prose, at most 150 words. Say what the reason is and, if it matters, what would break "
    "otherwise. Do not include commit ids, hashes, unit labels or file line numbers, and do not describe where "
    "or how you found the answer. If the material does not say why, answer exactly: Not recorded."
)


def answer_one(run_id, q, arm, model):
    out = S3 / "results" / run_id / "questions"
    out.mkdir(parents=True, exist_ok=True)
    path = out / f"question-{q['n']}-{arm}.json"
    if path.exists():
        return json.loads(path.read_text())
    tree, _ = base_tree(q["repo"])
    dst = WORK / "answers" / run_id / f"question-{q['n']}-{arm}"
    dst.mkdir(parents=True, exist_ok=True)
    env = env_for(scratch_home(dst), dst / "target")
    common = ["--model", model, "--output-format", "json", "--strict-mcp-config", "--no-session-persistence",
              "--permission-prompts", "none"]
    if arm == "corpus":
        context = (S3 / "context" / f"question-{q['n']}.txt").read_text()
        text = (f"A question about the `{q['repo']}` Rust repository: {q['text']}\n\n"
                "Answer only from the recorded decisions below. " + ANSWER_RULES + "\n\n" + CORPUS_PREAMBLE.replace(
                    "relevant to this task", "relevant to this question") + context)
        cmd = ["claude", "-p", text, "--tools", ""] + common
        cwd = dst
    else:
        text = (f"A question about the `{q['repo']}` Rust repository in the current directory: {q['text']}\n\n"
                "Answer from the repository's git history (git log, git show, git blame) and its code. " + ANSWER_RULES)
        cmd = ["claude", "-p", text, "--allowedTools", "Bash(git:*),Read,Glob,Grep", "--permission-mode", "dontAsk"] + common
        cwd = tree
    p = subprocess.run(cmd, cwd=cwd, env=env, capture_output=True, text=True, timeout=1800)
    try:
        a = json.loads(p.stdout)
        r = {"n": q["n"], "arm": arm, "answer": a.get("result", ""), "is_error": a.get("is_error"),
             "cost_usd": a.get("total_cost_usd"), "turns": a.get("num_turns")}
    except json.JSONDecodeError:
        r = {"n": q["n"], "arm": arm, "answer": "", "is_error": True, "raw": (p.stdout + p.stderr)[-2000:]}
    r["prompt"] = text
    if not r["is_error"]:
        path.write_text(json.dumps(r, indent=2) + "\n")
    return r


def answer(args):
    jobs = [(q, arm) for q in REG["question"] for arm in ("corpus", "git")]
    with cf.ThreadPoolExecutor(args.jobs) as ex:
        futs = {ex.submit(answer_one, args.run_id, q, arm, args.model): (q["n"], arm) for q, arm in jobs}
        for f in cf.as_completed(futs):
            n, arm = futs[f]
            r = f.result()
            print(f"question {n:>2} {arm:<6} error {r['is_error']}, cost {r.get('cost_usd')}, "
                  f"{len(r['answer'].split())} words", flush=True)


def scrub(text):
    """Remove what would reveal a source despite the instructions: hashes and unit labels."""
    import re
    text = re.sub(r"\b[a-z]/g[0-9a-f]{7,12}(r\d+)?(-\d+)*\b", "[id]", text)
    return re.sub(r"\b[0-9a-f]{7,40}\b", "[id]", text)


def sheet(args):
    import random
    root = S3 / "results" / args.run_id / "questions"
    rng = random.SystemRandom()
    key, parts = {}, [
        "# S3 questions: blind judging sheet\n",
        "For each question, two answers, A and B, in random order: one written from the recorded corpus only, the "
        "other from git history and code only. Judge each on its own merits against your knowledge of the code.\n",
        "Fill in each `correct` line with the answers that are correct (A, B, both, neither) and each `more useful` "
        "line with A, B or tie. Hashes and unit ids were replaced by [id] in both.\n",
    ]
    for q in REG["question"]:
        answers = {arm: json.loads((root / f"question-{q['n']}-{arm}.json").read_text())["answer"]
                   for arm in ("corpus", "git")}
        order = ["corpus", "git"]
        rng.shuffle(order)
        key[q["n"]] = {"A": order[0], "B": order[1]}
        parts.append(f"\n## {q['n']}. ({q['repo']}) {q['text']}\n")
        for letter, arm in zip("AB", order):
            parts.append(f"\n**{letter}.** {scrub(answers[arm]).strip()}\n")
        parts.append("\n- correct: \n- more useful: \n")
    (S3 / "results" / args.run_id / "judging.md").write_text("".join(parts))
    k = WORK / "keys" / f"{args.run_id}.json"
    k.parent.mkdir(parents=True, exist_ok=True)
    k.write_text(json.dumps(key, indent=2) + "\n")
    print(f"sheet: {S3 / 'results' / args.run_id / 'judging.md'}\nkey (do not open before judging): {k}")


def report_run2(rows):
    """Run2: violations by the oracle alone, stopped runs apart, no controls (eval/s3-protocol.md)."""
    from collections import Counter
    in_context = {n for n in range(1, 13)} - {10, 12}  # validation/step4-run-corpus.md
    arms = ("control", "corpus")
    by = {}
    for r in rows:
        by.setdefault((r["task"], r["arm"]), []).append(r["outcome"])
    print(f"{'task':>4} {'guarded':<7} " + "  ".join(f"{a:<28}" for a in arms))
    for n in sorted({r["task"] for r in rows}):
        cells = []
        for a in arms:
            c = Counter(by.get((n, a), []))
            cells.append(f"viol {c['violated']} stop {c['stopped']} ok {c['intact']} err {c['harness-error']}".ljust(28))
        print(f"{n:>4} {'yes' if task(n)['control'] else '':<7} " + "  ".join(cells))
    total = {a: Counter(r["outcome"] for r in rows if r["arm"] == a) for a in arms}
    ctx = {a: sum(1 for r in rows if r["arm"] == a and r["task"] in in_context and r["outcome"] == "violated") for a in arms}
    cost = {a: sum(((r.get("agent") or {}).get("cost_usd") or 0) for r in rows if r["arm"] == a) for a in arms}
    print()
    for a in arms:
        t = total[a]
        print(f"{a:<8} runs {sum(t.values()):>2}: violated {t['violated']}, stopped {t['stopped']}, intact {t['intact']}, "
              f"harness errors {t['harness-error']}; violated where the prerequisite is packed {ctx[a]}; cost ${cost[a]:.2f}")
    v, c = total["control"]["violated"], total["corpus"]["violated"]
    print("\ngate (corpus violations <= half of control violations): " +
          ("go" if v > 0 and c * 2 <= v else "no-go" if v > 0 else "undecided (no violations without the corpus)"))


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("prepare").set_defaults(fn=prepare)
    sub.add_parser("smoke").set_defaults(fn=smoke)
    r = sub.add_parser("run")
    r.add_argument("run_id")
    r.add_argument("--jobs", type=int, default=3)
    r.add_argument("--model", default="sonnet")
    r.add_argument("--tasks")
    r.add_argument("--arms", default="control,corpus")
    r.add_argument("--reps", type=int)
    r.set_defaults(fn=run)
    a = sub.add_parser("answer")
    a.add_argument("run_id")
    a.add_argument("--jobs", type=int, default=4)
    a.add_argument("--model", default="sonnet")
    a.set_defaults(fn=answer)
    k = sub.add_parser("sheet")
    k.add_argument("run_id")
    k.set_defaults(fn=sheet)
    p = sub.add_parser("report")
    p.add_argument("run_id")
    p.set_defaults(fn=report)
    args = ap.parse_args()
    args.fn(args)


if __name__ == "__main__":
    main()
