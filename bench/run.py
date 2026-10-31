#!/usr/bin/env python3
"""The solo-against-hierarchy bench: 30 small tasks, each run twice — solo (the throne alone,
escalating on the signals code reads) and hierarchy (the ask handed to the Elf) — on one model,
and with `--law on,off` each of those again with the law off (`mnemorph run --no-law`).

    python3 bench/run.py --check                 # every task's verify fails on its starter: no model
    python3 bench/run.py [--model M] [--tasks 01,02] [--modes solo,hierarchy] [--law on,off] [--limit N]

The key: OPENROUTER_API_KEY from the environment, else ~/.config/mnemorph/openrouter.key. It is
handed only to the run's own process, never written, never printed. Results go one line per run
to bench/results/<model>.jsonl; a rerun skips what is there, so a run stopped by a rate limit or
a daily cap resumes where it stopped.
"""

import argparse
import json
import os
import re
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import report

BENCH = Path(__file__).resolve().parent
REPO = BENCH.parent
DEFAULT_MODEL = "nvidia/nemotron-3.5-lightning:free"
HANDOFF = "Hand this whole ask to elf-e with the commission tool, then answer with its report:\n\n"
LIMITED = re.compile(r"\b429\b|rate.?limit|too many requests|free-models-per-day", re.I)
DAILY = re.compile(r"free-models-per-day|daily limit", re.I)


def tasks(only):
    found = sorted(p for p in (BENCH / "tasks").iterdir() if p.is_dir())
    return [t for t in found if not only or t.name.split("-")[0] in only]


def world_for(task, model):
    """A fresh world: the task's starter, the bench's creatures, the model's configuration, git."""
    w = Path(tempfile.mkdtemp(prefix="mnemorph-bench-"))
    shutil.copytree(task / "world", w, dirs_exist_ok=True)
    shutil.copytree(BENCH / "world" / ".mnemorph", w / ".mnemorph", dirs_exist_ok=True)
    (w / ".mnemorph" / "config.local.toml").write_text(
        "[providers.openrouter]\n"
        'kind = "openai"\n'
        'base_url = "https://openrouter.ai/api"\n'
        'key_env = "OPENROUTER_API_KEY"\n'
        "[models]\n"
        'provider = "openrouter"\n'
        'default = "%s"\n' % model
    )
    (w / ".gitignore").write_text(".mnemorph/instruments/\n.mnemorph/tmp/\n.mnemorph/config.local.toml\n__pycache__/\n")
    git = ["git", "-c", "user.name=bench", "-c", "user.email=bench@localhost"]
    subprocess.run(["git", "init", "-q"], cwd=w, check=True)
    subprocess.run(git + ["add", "-A"], cwd=w, check=True)
    subprocess.run(git + ["commit", "-qm", "starter"], cwd=w, check=True)
    return w


def verify(task, world):
    out = subprocess.run([sys.executable, str(task / "verify.py")], cwd=world, capture_output=True, text=True)
    return out.returncode == 0


def check():
    """Every task is real: its verify fails on the untouched starter. And no task hands its
    worker the answer: its verify, and anything named for an answer, stay outside the world."""
    bad, leaks = [], []
    for t in tasks(None):
        shown = [p for p in (t / "world").rglob("*")
                 if p.name == "verify.py"
                 or re.fullmatch(r"(answers?|expected|solutions?)(\..*)?", p.name, re.I)]
        leaks += ["%s hands the worker %s" % (t.name, p.relative_to(t)) for p in shown]
        w = world_for(t, DEFAULT_MODEL)
        if verify(t, w):
            bad.append(t.name)
        shutil.rmtree(w, ignore_errors=True)
    for name in bad:
        print("passes untouched:", name)
    for leak in leaks:
        print(leak)
    print("%d tasks, %d already pass untouched, %d hand over an answer" % (
        len(tasks(None)), len(bad), len(leaks)))
    return 1 if bad or leaks else 0


def key():
    if os.environ.get("OPENROUTER_API_KEY"):
        return os.environ["OPENROUTER_API_KEY"]
    path = Path.home() / ".config" / "mnemorph" / "openrouter.key"
    if not path.exists():
        return None
    if path.stat().st_mode & (stat.S_IRWXG | stat.S_IRWXO):
        print("%s is readable by others: chmod 600 it first" % path)
        return None
    return path.read_text().strip()


def ledger(world):
    """Calls, weighted tokens, dollars, and the calls of bodies below the throne, from the world's
    usage ledger."""
    calls, wt, usd, below = 0, 0, 0.0, 0
    path = world / ".mnemorph" / "instruments" / "usage.jsonl"
    for line in path.read_text().splitlines() if path.exists() else []:
        try:
            row = json.loads(line)
        except ValueError:
            continue
        calls += 1
        wt += row.get("wt") or 0
        usd += row.get("usd") or 0.0
        below += row.get("race") != "rimuru"
    return calls, wt, usd, below


def escalations(world):
    loop = world / ".mnemorph" / "instruments" / "loop"
    n = 0
    for journal in loop.glob("*.jsonl") if loop.exists() else []:
        n += journal.read_text().count("solo escalates to")
    return n


def read_json(stdout):
    """What `mnemorph run --json` said: its result line, the session's own turn endings, and the
    acts a rule refused, by rule — the bodies below included. A line that does not parse is
    skipped: a run killed mid-line has no result."""
    result, endings, refusals = None, [], {}
    for line in (stdout or "").splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if not isinstance(event, dict):
            continue
        kind = event.get("type")
        if kind == "result":
            result = event
        elif kind == "turnEnded" and event.get("body") == "rimuru":
            endings.append(event.get("ending") or "")
        elif kind == "toolResult" and event.get("refused") in report.RULES:
            refusals[event["refused"]] = refusals.get(event["refused"], 0) + 1
    return result, endings, refusals


def run_one(task, mode, model, secret, timeout, keep=False, law="on"):
    # A fresh world for every run: the two law arms of a case share its trial key, never its state.
    world = world_for(task, model)
    ask = (task / "ask.md").read_text().strip()
    if mode == "hierarchy":
        ask = HANDOFF + ask
    cli = [str(REPO / "target/debug/mnemorph"), "run", ask, "--world", str(world), "--json"]
    if law == "off":
        cli.append("--no-law")
    env = dict(os.environ, OPENROUTER_API_KEY=secret)
    start = time.time()
    # Its own process group: a timeout ends the run and anything it started, key and all.
    proc = subprocess.Popen(cli, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            text=True, start_new_session=True)
    try:
        stdout, err = proc.communicate(timeout=timeout)
        code = proc.returncode
    except subprocess.TimeoutExpired:
        os.killpg(proc.pid, signal.SIGKILL)
        stdout, err = proc.communicate()
        code, err = None, (err or "") + "\ntimeout"
    seconds = round(time.time() - start, 1)
    # A rate limit reaches stderr when the throne's call dies, stdout when a body's does.
    limited = bool(LIMITED.search(err or "") or LIMITED.search(stdout or ""))
    daily = bool(DAILY.search(err or "") or DAILY.search(stdout or ""))
    calls, wt, usd, below = ledger(world)
    scrubbed = (err or "").replace(secret, "[key]")
    # How it ended: the result line `--json` writes, else the session's last `turnEnded`, else the
    # `mnemorph: <ending>` line stderr ends on. The report reads the cause.
    said, endings, refusals = read_json(stdout)
    told = [l[len("mnemorph: "):] for l in (err or "").splitlines() if l.startswith("mnemorph: ")]
    if code is None:
        ended = "hang"
    elif said and said.get("ending"):
        ended = said["ending"]
    elif endings:
        ended = endings[-1]
    else:
        ended = "answered" if code == 0 else told[-1] if told else ""
    result = {
        "task": task.name, "mode": mode, "model": model,
        # The case: both law arms of it carry the same key.
        "trial": "%s/%s" % (task.name, mode),
        "law": law,
        # The law as the run itself said it; None when it wrote no result line.
        "law_said": said.get("law") if said else None,
        "passed": verify(task, world), "exit": code, "ending": ended.replace(secret, "[key]"),
        "turns": [e.replace(secret, "[key]") for e in endings],
        "refusals": refusals,
        "seconds": seconds,
        "calls": calls, "wt": wt, "usd": round(usd, 6),
        "escalations": escalations(world), "below": below,
        # A hierarchy run where no body below the throne made a call did not run as one.
        "valid": mode != "hierarchy" or below > 0,
        "limited": limited,
        "daily": daily,
        "stderr": scrubbed[-400:],
    }
    if keep:
        result["world"] = str(world)
    else:
        shutil.rmtree(world, ignore_errors=True)
    return result


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--check", action="store_true")
    p.add_argument("--model", default=os.environ.get("MNEMORPH_BENCH_MODEL", DEFAULT_MODEL))
    p.add_argument("--tasks", default="")
    p.add_argument("--modes", default="solo,hierarchy")
    p.add_argument("--law", default="on", help="the law arms: on, off, or on,off")
    p.add_argument("--limit", type=int, default=0, help="at most this many runs now")
    # Past the core's one-hour clock: slow work ends as a checkpoint, and this kills only a hang.
    p.add_argument("--timeout", type=int, default=3900)
    p.add_argument("--keep", action="store_true", help="keep each run's world, for a post-mortem")
    p.add_argument("--out", default="", help="a results file other than the model's")
    p.add_argument("--redo-invalid", action="store_true",
                   help="run again what the machine or the provider stopped; the new row stands in")
    a = p.parse_args()
    if a.check:
        return check()
    laws = a.law.split(",")
    if any(l not in report.LAWS for l in laws) or len(set(laws)) != len(laws):
        print("--law is on, off, or on,off")
        return 2
    secret = key()
    if not secret:
        print("no key: set OPENROUTER_API_KEY, or write ~/.config/mnemorph/openrouter.key (mode 600)")
        return 2
    out = Path(a.out) if a.out else BENCH / "results" / (re.sub(r"[^A-Za-z0-9._-]", "_", a.model) + ".jsonl")
    out.parent.mkdir(exist_ok=True)
    done = set()
    if out.exists():
        for line in out.read_text().splitlines():
            r = json.loads(line)
            # A run the machine or the provider stopped measured nothing: with --redo-invalid it
            # is run again, unless a later row already stands in for it.
            if a.redo_invalid and not report.judged(r):
                continue
            done.add((r["task"], r["mode"], report.law(r)))
    only = set(a.tasks.split(",")) - {""}
    ran = 0
    arms = [(mode, lw) for mode in a.modes.split(",") for lw in laws]
    for t in tasks(only):
        for mode, lw in arms:
            if (t.name, mode, lw) in done:
                continue
            if a.limit and ran >= a.limit:
                return 0
            # A per-minute limit passes: wait it out a few times. The daily cap, or a limit that
            # stays, stops the run; what the limited attempts spent is counted in the one recorded.
            spent = {"calls": 0, "wt": 0, "usd": 0.0}
            attempts = 0
            for wait in (70, 140, 280, None):
                r = run_one(t, mode, a.model, secret, a.timeout, a.keep, lw)
                attempts += 1
                if not r["limited"]:
                    break
                for k in spent:
                    spent[k] += r[k]
                if r.get("world"):
                    print("a limited attempt's world is kept at %s" % r["world"])
                if r["daily"] or wait is None:
                    print("rate-limited at %s %s law %s: stopping; rerun later to resume" % (t.name, mode, lw))
                    return 3
                print("rate-limited at %s %s law %s: waiting %ds" % (t.name, mode, lw, wait))
                time.sleep(wait)
            for k in spent:
                r[k] += spent[k]
            r["attempts"] = attempts
            with out.open("a") as f:
                f.write(json.dumps(r) + "\n")
            ran += 1
            print("%s %-9s law %-3s %s  %ss  %d calls (%d below)  %d wt  esc %d  refused %d%s" % (
                t.name, mode, lw, "PASS" if r["passed"] else "fail", r["seconds"], r["calls"],
                r["below"], r["wt"], r["escalations"], report.breaches(r),
                "" if r["valid"] else "  INVALID: nothing ran below"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
