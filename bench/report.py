#!/usr/bin/env python3
"""The bench's report: what bench/run.py measured, judged honestly.

    python3 bench/report.py [RESULTS.jsonl] [--modes solo,hierarchy] [--law on,off] [--partial] [--json]

The matrix is fixed up front — every task in every mode under every law arm, exactly once. A run
missing, extra or recorded twice is an error, never dropped, and so is a run whose own record of
the law (`law_said`, from `mnemorph run --json`) is not its arm's; --partial reports a run still in
progress and names the gaps. A task succeeds only when its verify passed, the run ended answered,
no rule refused any of its acts (a breach), and a hierarchy run ran below the throne. Every other
run is told by why it ended: a stop the harness made — an escalation, a checkpoint, the gate, a
breach, the bench's timeout, a hierarchy that never delegated — is a failed task, a correct file
left behind included; a stop the task does not own — no kernel guard, a provider or model failure,
an interrupt — measured nothing, so it is never counted, and is listed to rerun.

Each estimand is a difference in task success rate between two arms that differ in one dimension
— hierarchy minus solo under one law, law on minus law off in one mode — over the same tasks. Its
intervals are a paired bootstrap that resamples tasks — the groups — never rows, and Newcombe's
hybrid score interval for paired proportions (1998, method 10), which holds at small n; the report
states the estimand and the group count beside them. There is no single aggregate score: each
arm's success, breaches, calls, weighted tokens and time stand side by side.
"""

import argparse
import json
import math
import random
import statistics
from collections import Counter
import sys
from pathlib import Path

BENCH = Path(__file__).resolve().parent
SEED = 96
RESAMPLES = 10_000


def expected_tasks():
    return sorted(p.name for p in (BENCH / "tasks").iterdir() if p.is_dir())


# How a run ended, read from its recorded ending — or, for a run recorded before endings were,
# from the `mnemorph: <ending>` line the CLI writes to stderr for any ending but `answered`.
HARNESS = {"escalated", "checkpoint", "gate", "refused", "breach", "never delegated"}
# The rules a run's acts meet: one that refused an act is a breach, however the run ended.
RULES = ("envelope", "gate", "sandbox", "shelf")
LAWS = ("on", "off")
# A hang outlives the core's own one-hour clock, which ends slow work as a checkpoint: the bench
# kills only what stalled, at 65 minutes.
INFRA = {"machine", "provider", "interrupted", "hang", "harness bug"}


def ending(r):
    if r.get("ending"):
        return r["ending"]
    if r.get("exit") == 0:
        return "answered"
    lines = [l for l in (r.get("stderr") or "").splitlines() if l.startswith("mnemorph: ")]
    return lines[-1][len("mnemorph: "):] if lines else ""


def law(r):
    """The run's law arm, `on` or `off`; a row recorded before the arm existed ran with the law."""
    return r.get("law") or "on"


def breaches(r):
    """How many of the run's acts a rule refused — the envelope, the human gate, the sandbox's
    guard, the race's shelf — as `mnemorph run --json` named them."""
    return sum((r.get("refusals") or {}).get(k, 0) for k in RULES)


def cause(r):
    """Why the run ended as it did: `answered`, a stop the harness made and the task owns
    (escalated, checkpoint, gate, refused, a breach, never delegated), or one the task does not
    (machine, provider, interrupted, a hang, a harness bug) — that run measured nothing and is
    rerun. An answered run that broke a rule on the way is a breach."""
    e = ending(r)
    if r.get("exit") is None:
        return "hang"
    if e == "answered":
        if not r.get("valid", True):
            return "never delegated"
        return "breach" if breaches(r) else "answered"
    if e.startswith("escalate:"):
        return "escalated"
    if e.startswith("checkpoint:"):
        return "checkpoint"
    # Exactly the no-guard phrase: "could not verify twice" is the body's own check hanging.
    if e.startswith("failed: the end gate could not verify:"):
        return "machine"
    if e.startswith("failed: the end gate"):
        return "gate"
    if e.startswith("failed: the model refused"):
        return "refused"
    if e.startswith("failed: the conversation is malformed"):
        return "harness bug"
    if e == "interrupted":
        return "interrupted"
    return "provider"


def judged(r):
    """The run measured the task: it was not stopped by the machine or the provider."""
    return cause(r) not in INFRA


def succeeded(r):
    """Verify passed, the run ended answered, and no rule refused any of its acts."""
    return bool(r["passed"]) and cause(r) == "answered"


def matrix(rows, tasks, modes, laws=("on",)):
    """The judged rows by (task, mode, law), every way they miss the fixed matrix, and the runs the
    machine or the provider stopped, to rerun. A later row stands in for an earlier unjudged one;
    any other second row for a cell is an error, and so is a row whose run said another law than
    its arm."""
    want = {(t, m, l) for t in tasks for m in modes for l in laws}
    cells, unjudged, errors = {}, {}, []
    for r in rows:
        key = (r["task"], r["mode"], law(r))
        said = r.get("law_said")
        if said is not None and said != (law(r) == "on"):
            errors.append("mislabelled arm: %s %s law %s, but the run said law %s"
                          % (key + ("on" if said else "off",)))
            continue
        if key not in want:
            errors.append("extra run: %s %s law %s" % key)
        elif key in cells:
            errors.append("recorded twice: %s %s law %s" % key)
        elif judged(r):
            cells[key] = r
            unjudged.pop(key, None)
        else:
            unjudged[key] = r
    rerun = sorted(unjudged)
    missing = sorted(want - set(cells) - set(unjudged))
    return cells, errors, missing, rerun


def bootstrap(diffs):
    """A 95% percentile interval for the mean of per-task differences, resampling tasks."""
    rng = random.Random(SEED)
    n = len(diffs)
    means = sorted(sum(rng.choice(diffs) for _ in range(n)) / n for _ in range(RESAMPLES))
    return means[int(0.025 * RESAMPLES)], means[int(0.975 * RESAMPLES) - 1]


Z95 = 1.959963984540054


def wilson(x, n, z=Z95):
    """Wilson's score interval for a proportion x / n."""
    p = x / n
    centre, half = p + z * z / (2 * n), z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n))
    return (centre - half) / (1 + z * z / n), (centre + half) / (1 + z * z / n)


def newcombe(both, first_only, second_only, neither, z=Z95):
    """Newcombe's hybrid score interval (1998, method 10) for p1 - p2 of paired proportions: the
    Wilson limits of each margin, joined through the table's phi, its positive covariance less
    n / 2 (zero when a margin is empty). p1 is the first arm's success, p2 the second's."""
    a, b, c, d = both, first_only, second_only, neither
    n = a + b + c + d
    p1, p2 = (a + b) / n, (a + c) / n
    l1, u1 = wilson(a + b, n, z)
    l2, u2 = wilson(a + c, n, z)
    margins = (a + b) * (c + d) * (a + c) * (b + d)
    cov = a * d - b * c
    if cov > 0:
        cov = max(cov - n / 2, 0.0)
    phi = cov / math.sqrt(margins) if margins else 0.0
    lo = p1 - p2 - math.sqrt(max((p1 - l1) ** 2 - 2 * phi * (p1 - l1) * (u2 - p2) + (u2 - p2) ** 2, 0.0))
    hi = p1 - p2 + math.sqrt(max((u1 - p1) ** 2 - 2 * phi * (u1 - p1) * (p2 - l2) + (p2 - l2) ** 2, 0.0))
    return max(lo, -1.0), min(hi, 1.0)


def arm(cells, tasks, mode, lw="on"):
    rs = [cells[(t, mode, lw)] for t in tasks if (t, mode, lw) in cells]
    if not rs:
        return None
    med = lambda k: statistics.median(r.get(k) or 0 for r in rs)
    return {
        "runs": len(rs),
        "success": sum(succeeded(r) for r in rs) / len(rs),
        "verify_only": sum(bool(r["passed"]) for r in rs) / len(rs),
        "unanswered": sum(bool(r["passed"]) and not succeeded(r) for r in rs),
        # Runs a rule refused an act of, however they ended, and the refusals by rule.
        "breaches": sum(breaches(r) > 0 for r in rs),
        "refusals": {k: n for k in RULES
                     if (n := sum((r.get("refusals") or {}).get(k, 0) for r in rs))},
        "failed_by": dict(sorted(Counter(cause(r) for r in rs if not succeeded(r)).items())),
        "median_calls": med("calls"),
        "median_wt": med("wt"),
        "median_seconds": med("seconds"),
        "escalations": sum(r.get("escalations") or 0 for r in rs),
        "usd": round(sum(r.get("usd") or 0.0 for r in rs), 6),
    }


def name(mode, lw, laws):
    """An arm's name: its mode alone when every run held the law, else the mode and its law."""
    return mode if list(laws) == ["on"] else "%s, law %s" % (mode, lw)


def contrast(cells, tasks, first, second, what, names):
    """The paired estimand `second` minus `first` over the tasks both ran: the bootstrap over
    tasks, Newcombe's interval, and the discordant tasks both rest on."""
    paired = [t for t in tasks if (t,) + first in cells and (t,) + second in cells]
    if not paired:
        return None
    wins = [(succeeded(cells[(t,) + second]), succeeded(cells[(t,) + first])) for t in paired]
    diffs = [s - f for s, f in wins]
    lo, hi = bootstrap(diffs)
    table = [sum(s and f for s, f in wins), sum(s and not f for s, f in wins),
             sum(f and not s for s, f in wins), sum(not s and not f for s, f in wins)]
    return {
        "what": "task success rate, %s, over the same tasks" % what,
        "groups": len(paired),
        "difference": sum(diffs) / len(paired),
        "interval_95": [lo, hi],
        "method": "paired percentile bootstrap over task groups",
        "newcombe_95": list(newcombe(*table)),
        "newcombe_method": "Newcombe hybrid score, paired (1998, method 10)",
        # Tasks only the second arm won, and only the first: the intervals rest on these.
        "discordant": table[1:3],
        "discordant_arms": list(names),
        "resamples": RESAMPLES,
        "seed": SEED,
    }


def report(rows, modes, partial, laws=("on",)):
    tasks = expected_tasks()
    laws = list(laws)
    cells, errors, missing, rerun = matrix(rows, tasks, modes, laws)
    out = {"tasks": len(tasks), "modes": modes, "laws": laws, "errors": errors, "missing": missing,
           "rerun": rerun,
           "bugs": [k for k in rerun if cause(next(r for r in reversed(rows)
                                                   if (r["task"], r["mode"], law(r)) == k)) == "harness bug"]}
    if errors or ((missing or rerun) and not partial):
        return out, 1
    out["arms"] = {name(m, l, laws): arm(cells, tasks, m, l) for m in modes for l in laws}
    estimands = []
    if len(modes) == 2:
        a, b = modes
        for l in laws:
            what = "%s minus %s" % (b, a) + ("" if laws == ["on"] else ", law %s" % l)
            estimands.append(contrast(cells, tasks, (a, l), (b, l), what,
                                      (name(b, l, laws), name(a, l, laws))))
    if len(laws) == 2:
        for m in modes:
            estimands.append(contrast(cells, tasks, (m, "off"), (m, "on"),
                                      "law on minus law off, %s" % m,
                                      (name(m, "on", laws), name(m, "off", laws))))
    out["estimands"] = [e for e in estimands if e]
    return out, 0


def show(out, partial):
    lines = []
    if out["errors"]:
        lines += ["matrix error: " + e for e in out["errors"]]
    if out["missing"]:
        lines.append("%s %d of %d runs missing: %s" % (
            "partial —" if partial else "matrix error:", len(out["missing"]),
            out["tasks"] * len(out["modes"]) * len(out["laws"]),
            ", ".join("%s %s law %s" % k for k in out["missing"][:6])
            + (" …" if len(out["missing"]) > 6 else "")))
    bugs = [k for k in out["rerun"] if out.get("bugs") and k in out["bugs"]]
    if bugs:
        lines.append("HARNESS BUG — a malformed conversation, fix it before trusting a rerun: "
                     + ", ".join("%s %s law %s" % k for k in bugs))
    if out["rerun"]:
        lines.append("%s %d runs the machine or the provider stopped, to rerun "
                     "(run.py --redo-invalid): %s" % (
                         "partial —" if partial else "matrix error:", len(out["rerun"]),
                         ", ".join("%s %s law %s" % k for k in out["rerun"])))
    for m, s in (out.get("arms") or {}).items():
        if s:
            lines.append(
                "%-9s %2d runs  success %3.0f%% (verify alone %3.0f%%, %d correct but not a success)  "
                "breaches %d%s  median %s calls, %s wt, %ss  escalations %d  $%s" % (
                    m, s["runs"], 100 * s["success"], 100 * s["verify_only"], s["unanswered"],
                    s["breaches"], " (%s)" % ", ".join("%d %s" % (n, k) for k, n in s["refusals"].items())
                    if s["refusals"] else "",
                    s["median_calls"], s["median_wt"], s["median_seconds"], s["escalations"], s["usd"]))
            if s["failed_by"]:
                lines.append("          not a success: " + ", ".join(
                    "%d %s" % (n, c if c != "answered" else "answered wrong")
                    for c, n in s["failed_by"].items()))
    for e in out.get("estimands") or []:
        lines.append("estimand: %s — %+.3f over %d task groups" % (
            e["what"], e["difference"], e["groups"]))
        lines.append("  95%% interval [%+.3f, %+.3f], %s, %d resamples (seed %d)" % (
            *e["interval_95"], e["method"], e["resamples"], e["seed"]))
        lines.append("  95%% interval [%+.3f, %+.3f], %s" % (*e["newcombe_95"], e["newcombe_method"]))
        won, lost = e["discordant"]
        lines.append("  discordant tasks: %d won only by %s, %d only by %s%s" % (
            won, e["discordant_arms"][0], lost, e["discordant_arms"][1],
            " — a percentile bootstrap under-covers with this few; read Newcombe's interval"
            if won + lost < 10 else ""))
    print("\n".join(lines))


def main():
    p = argparse.ArgumentParser()
    p.add_argument("results", nargs="?", default="")
    p.add_argument("--modes", default="solo,hierarchy")
    p.add_argument("--law", default="on", help="the law arms: on, off, or on,off")
    p.add_argument("--partial", action="store_true", help="report a run still in progress")
    p.add_argument("--json", action="store_true")
    a = p.parse_args()
    path = Path(a.results) if a.results else next(iter(sorted((BENCH / "results").glob("*.jsonl"))), None)
    if not path or not path.exists():
        print("no results: name the results file")
        return 2
    rows = [json.loads(l) for l in path.read_text().splitlines() if l.strip()]
    laws = a.law.split(",")
    if any(l not in LAWS for l in laws) or len(set(laws)) != len(laws):
        print("--law is on, off, or on,off")
        return 2
    out, code = report(rows, a.modes.split(","), a.partial, laws)
    if a.json:
        print(json.dumps(out, indent=2))
    else:
        show(out, a.partial)
    return code


if __name__ == "__main__":
    sys.exit(main())
