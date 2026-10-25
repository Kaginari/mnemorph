#!/usr/bin/env python3
"""The bench's report: what bench/run.py measured, judged honestly.

    python3 bench/report.py [RESULTS.jsonl] [--modes solo,hierarchy] [--partial] [--json]

The matrix is fixed up front — every task in every mode, exactly once. A run missing, extra or
recorded twice is an error, never dropped; --partial reports a run still in progress and names
the gaps. A task succeeds only when its verify passed *and* the run ended answered: the CLI exited
0 — not a checkpoint, an escalation, a failure or a timeout — and a hierarchy run ran below the
throne. A correct file left by a run that did not end answered is a failed task.

The estimand is the difference in task success rate, hierarchy minus solo, over the same tasks.
Its interval is a paired bootstrap that resamples tasks — the groups — never rows, and the report
states the estimand and the group count beside it. There is no single aggregate score: each arm's
success, calls, weighted tokens and time stand side by side.
"""

import argparse
import json
import random
import statistics
import sys
from pathlib import Path

BENCH = Path(__file__).resolve().parent
SEED = 96
RESAMPLES = 10_000


def expected_tasks():
    return sorted(p.name for p in (BENCH / "tasks").iterdir() if p.is_dir())


def succeeded(r):
    """Verify passed, and the run ended answered."""
    return bool(r["passed"]) and r.get("exit") == 0 and r.get("valid", True)


def matrix(rows, tasks, modes):
    """The rows by (task, mode), and every way they miss the fixed matrix."""
    want = {(t, m) for t in tasks for m in modes}
    cells, errors = {}, []
    for r in rows:
        key = (r["task"], r["mode"])
        if key not in want:
            errors.append("extra run: %s %s" % key)
        elif key in cells:
            errors.append("recorded twice: %s %s" % key)
        else:
            cells[key] = r
    missing = sorted(want - set(cells))
    return cells, errors, missing


def bootstrap(diffs):
    """A 95% percentile interval for the mean of per-task differences, resampling tasks."""
    rng = random.Random(SEED)
    n = len(diffs)
    means = sorted(sum(rng.choice(diffs) for _ in range(n)) / n for _ in range(RESAMPLES))
    return means[int(0.025 * RESAMPLES)], means[int(0.975 * RESAMPLES) - 1]


def arm(cells, tasks, mode):
    rs = [cells[(t, mode)] for t in tasks if (t, mode) in cells]
    if not rs:
        return None
    med = lambda k: statistics.median(r.get(k) or 0 for r in rs)
    return {
        "runs": len(rs),
        "success": sum(succeeded(r) for r in rs) / len(rs),
        "verify_only": sum(bool(r["passed"]) for r in rs) / len(rs),
        "unanswered": sum(bool(r["passed"]) and not succeeded(r) for r in rs),
        "median_calls": med("calls"),
        "median_wt": med("wt"),
        "median_seconds": med("seconds"),
        "escalations": sum(r.get("escalations") or 0 for r in rs),
        "usd": round(sum(r.get("usd") or 0.0 for r in rs), 6),
    }


def report(rows, modes, partial):
    tasks = expected_tasks()
    cells, errors, missing = matrix(rows, tasks, modes)
    out = {"tasks": len(tasks), "modes": modes, "errors": errors, "missing": missing}
    if errors or (missing and not partial):
        return out, 1
    out["arms"] = {m: arm(cells, tasks, m) for m in modes}
    if len(modes) == 2:
        a, b = modes
        paired = [t for t in tasks if (t, a) in cells and (t, b) in cells]
        diffs = [succeeded(cells[(t, b)]) - succeeded(cells[(t, a)]) for t in paired]
        if paired:
            lo, hi = bootstrap(diffs)
            discordant = [sum(d == 1 for d in diffs), sum(d == -1 for d in diffs)]
            out["estimand"] = {
                "what": "task success rate, %s minus %s, over the same tasks" % (b, a),
                "groups": len(paired),
                "difference": sum(diffs) / len(paired),
                "interval_95": [lo, hi],
                # Tasks only the second arm won, and only the first: the interval rests on these.
                "discordant": discordant,
                "resamples": RESAMPLES,
                "seed": SEED,
            }
    return out, 0


def show(out, partial):
    lines = []
    if out["errors"]:
        lines += ["matrix error: " + e for e in out["errors"]]
    if out["missing"]:
        lines.append("%s %d of %d runs missing: %s" % (
            "partial —" if partial else "matrix error:", len(out["missing"]),
            out["tasks"] * len(out["modes"]), ", ".join("%s %s" % k for k in out["missing"][:6])
            + (" …" if len(out["missing"]) > 6 else "")))
    for m, s in (out.get("arms") or {}).items():
        if s:
            lines.append(
                "%-9s %2d runs  success %3.0f%% (verify alone %3.0f%%, %d correct but unanswered)  "
                "median %s calls, %s wt, %ss  escalations %d  $%s" % (
                    m, s["runs"], 100 * s["success"], 100 * s["verify_only"], s["unanswered"],
                    s["median_calls"], s["median_wt"], s["median_seconds"], s["escalations"], s["usd"]))
    e = out.get("estimand")
    if e:
        lines.append("estimand: %s — %+.3f, 95%% interval [%+.3f, %+.3f], %d task groups, "
                     "%d resamples (seed %d)" % (e["what"], e["difference"], *e["interval_95"],
                                                 e["groups"], e["resamples"], e["seed"]))
        won, lost = e["discordant"]
        lines.append("discordant tasks: %d won only by %s, %d only by %s%s" % (
            won, out["modes"][1], lost, out["modes"][0],
            " — a percentile bootstrap under-covers with this few; read the interval as rough"
            if won + lost < 10 else ""))
    print("\n".join(lines))


def main():
    p = argparse.ArgumentParser()
    p.add_argument("results", nargs="?", default="")
    p.add_argument("--modes", default="solo,hierarchy")
    p.add_argument("--partial", action="store_true", help="report a run still in progress")
    p.add_argument("--json", action="store_true")
    a = p.parse_args()
    path = Path(a.results) if a.results else next(iter(sorted((BENCH / "results").glob("*.jsonl"))), None)
    if not path or not path.exists():
        print("no results: name the results file")
        return 2
    rows = [json.loads(l) for l in path.read_text().splitlines() if l.strip()]
    out, code = report(rows, a.modes.split(","), a.partial)
    if a.json:
        print(json.dumps(out, indent=2))
    else:
        show(out, a.partial)
    return code


if __name__ == "__main__":
    sys.exit(main())
