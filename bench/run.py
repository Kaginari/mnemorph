#!/usr/bin/env python3
"""The solo-against-hierarchy bench: 30 small tasks, each run twice — solo (the throne alone,
escalating on the signals code reads) and hierarchy (the ask handed to the Elf) — on one model.

    python3 bench/run.py --check                 # every task's verify fails on its starter: no model
    python3 bench/run.py [--model M] [--tasks 01,02] [--modes solo,hierarchy] [--limit N]

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

BENCH = Path(__file__).resolve().parent
REPO = BENCH.parent
DEFAULT_MODEL = "nvidia/nemotron-3.5-lightning:free"
HANDOFF = "Hand this whole ask to elf-e with the commission tool, then answer with its report:\n\n"
LIMITED = re.compile(r"\b429\b|rate.?limit|too many requests|free-models-per-day", re.I)


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
    (w / ".gitignore").write_text(".mnemorph/instruments/\n.mnemorph/tmp/\n.mnemorph/config.local.toml\n")
    git = ["git", "-c", "user.name=bench", "-c", "user.email=bench@localhost"]
    subprocess.run(["git", "init", "-q"], cwd=w, check=True)
    subprocess.run(git + ["add", "-A"], cwd=w, check=True)
    subprocess.run(git + ["commit", "-qm", "starter"], cwd=w, check=True)
    return w


def verify(task, world):
    out = subprocess.run([sys.executable, str(task / "verify.py")], cwd=world, capture_output=True, text=True)
    return out.returncode == 0


def check():
    """Every task is real: its verify fails on the untouched starter."""
    bad = []
    for t in tasks(None):
        w = world_for(t, DEFAULT_MODEL)
        if verify(t, w):
            bad.append(t.name)
        shutil.rmtree(w, ignore_errors=True)
    for name in bad:
        print("passes untouched:", name)
    print("%d tasks, %d already pass untouched" % (len(tasks(None)), len(bad)))
    return 1 if bad else 0


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


def run_one(task, mode, model, secret, timeout):
    world = world_for(task, model)
    ask = (task / "ask.md").read_text().strip()
    if mode == "hierarchy":
        ask = HANDOFF + ask
    cli = [shutil.which("node") or "node", str(REPO / "packages/cli/dist/cli.js"), "run", ask,
           "--world", str(world), "--json", "--core", str(REPO / "target/debug/mnemorphd")]
    env = dict(os.environ, OPENROUTER_API_KEY=secret)
    start = time.time()
    # Its own process group: a timeout ends the CLI and the core it started, key and all.
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
    calls, wt, usd, below = ledger(world)
    scrubbed = (err or "").replace(secret, "[key]")
    result = {
        "task": task.name, "mode": mode, "model": model,
        "passed": verify(task, world), "exit": code, "seconds": seconds,
        "calls": calls, "wt": wt, "usd": round(usd, 6),
        "escalations": escalations(world), "below": below,
        # A hierarchy run where no body below the throne made a call did not run as one.
        "valid": mode != "hierarchy" or below > 0,
        "limited": limited,
        "stderr": scrubbed[-400:],
    }
    shutil.rmtree(world, ignore_errors=True)
    return result


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--check", action="store_true")
    p.add_argument("--model", default=os.environ.get("MNEMORPH_BENCH_MODEL", DEFAULT_MODEL))
    p.add_argument("--tasks", default="")
    p.add_argument("--modes", default="solo,hierarchy")
    p.add_argument("--limit", type=int, default=0, help="at most this many runs now")
    p.add_argument("--timeout", type=int, default=1800)
    a = p.parse_args()
    if a.check:
        return check()
    secret = key()
    if not secret:
        print("no key: set OPENROUTER_API_KEY, or write ~/.config/mnemorph/openrouter.key (mode 600)")
        return 2
    out = BENCH / "results" / (re.sub(r"[^A-Za-z0-9._-]", "_", a.model) + ".jsonl")
    out.parent.mkdir(exist_ok=True)
    done = set()
    if out.exists():
        for line in out.read_text().splitlines():
            r = json.loads(line)
            done.add((r["task"], r["mode"]))
    only = set(a.tasks.split(",")) - {""}
    ran = 0
    for t in tasks(only):
        for mode in a.modes.split(","):
            if (t.name, mode) in done:
                continue
            if a.limit and ran >= a.limit:
                return 0
            r = run_one(t, mode, a.model, secret, a.timeout)
            if r["limited"]:
                print("rate-limited at %s %s: stopping; rerun later to resume" % (t.name, mode))
                return 3
            with out.open("a") as f:
                f.write(json.dumps(r) + "\n")
            ran += 1
            print("%s %-9s %s  %ss  %d calls (%d below)  %d wt  esc %d%s" % (
                t.name, mode, "PASS" if r["passed"] else "fail", r["seconds"], r["calls"], r["below"],
                r["wt"], r["escalations"], "" if r["valid"] else "  INVALID: nothing ran below"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
