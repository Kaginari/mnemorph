# The solo-against-hierarchy bench

The roadmap's M7 asks for solo's escalation to be measured against the hierarchy on the same 30
tasks. This is that bench: thirty small stdlib-Python tasks, each run twice on one model.

- **solo** — the ask as written, to the throne alone; it escalates on the signals code reads
  ([solo mode](../docs/02-races.md)): a context checkpoint, an outward act it does not hold.
- **hierarchy** — the same ask, handed to the Elf (`elf-e`), down through its Orc and Slime.

Every world is fresh: the task's starter (`tasks/NN-name/world/`), the bench's creatures
(`world/.mnemorph/`), a configuration naming the model through OpenRouter, and a git history. A
task passes when its `verify.py` — kept outside the world, so no body can edit it — exits 0.

## Running it

```sh
just check                      # builds target/debug/mnemorphd and the CLI
python3 bench/run.py --check    # every task fails untouched: no model, no key
python3 bench/run.py            # all 60 runs on the default free model
python3 bench/run.py --tasks 01,02 --modes solo --limit 2
python3 bench/run.py --tasks 01 --modes hierarchy --keep --out /tmp/look.jsonl   # a post-mortem
```

`--keep` leaves each run's world on disk and names it in the result, to read its journals;
`--out` writes the results elsewhere than the model's tracked file.

The key is `OPENROUTER_API_KEY` from the environment, else `~/.config/mnemorph/openrouter.key`
(mode 600 — a key file others can read is refused). It is handed only to each run's own process,
never written and never printed; a run's saved stderr is scrubbed of it before it is cut. `--model` (or `MNEMORPH_BENCH_MODEL`) picks the model; the default is
a free one.

Each run is one line in `results/<model>.jsonl`, tracked as the measurement's record: whether it
passed, its exit, seconds, model calls (and how many came from bodies below the throne), weighted
tokens and dollars (from the world's usage ledger), and how many times solo escalated. A hierarchy
run in which no body below the throne made a call did not run as one: it is marked `valid: false`.
A rerun skips what is there. A rate limit or a daily cap — seen on stderr, or in a body's result
on stdout — stops the run with exit 3, unrecorded, and the next run resumes from it; a free
model's daily cap may take the sixty runs over a few days. A run past its timeout (30 minutes) is
killed with every process it started, and recorded as a fail. `just check` also builds
`target/debug/mnemorph-guard`, the sandbox's helper: without it every shell act is refused.

## Reading it

```sh
python3 bench/report.py results/<model>.jsonl            # the whole matrix, or an error
python3 bench/report.py results/<model>.jsonl --partial  # a run still in progress, its gaps named
```

The matrix is fixed up front: every task in every mode, exactly once. A run that is missing,
extra or recorded twice is an error, never dropped; `--partial` reports a run still in progress
and names what is missing, but an extra or doubled run is an error even then. A task succeeds
only when its verify passed *and* the run ended answered: the CLI exited 0 — not a checkpoint,
an escalation, a failure or a timeout — and a hierarchy run ran below the throne. A correct file
left by a run that did not end answered counts as a failed task, and the report says how many
there were. A gate refusal the body recovered from leaves the exit at 0, so this reads how the run
ended, not every rule it met on the way.

The estimand is the difference in task success rate, hierarchy minus solo, over the same tasks.
Its 95% interval is a paired bootstrap that resamples tasks — the groups — never rows: 10 000
resamples, seeded, so the same rows give the same interval. The report states the estimand and the
group count beside it, and the discordant tasks — won by one arm only — the interval rests on;
with fewer than ten, a percentile bootstrap under-covers, and the report says so. There is no
single aggregate score: each arm's success, verify-alone rate,
median calls, weighted tokens and seconds, escalations and dollars stand side by side.

`just bench` — part of `just check` — runs the report's tests and `run.py --check`, with no model:
every task's verify fails on its untouched starter, and no task hands its worker the answer (its
`verify.py`, or a file named `answer`, `expected` or `solution`, inside its world). `--json`
prints the report as data.
