# The solo-against-hierarchy bench

The roadmap's M7 asks for solo's escalation to be measured against the hierarchy on the same 30
tasks. This is that bench: thirty small stdlib-Python tasks, each run twice on one model.

- **solo** — the ask as written, to the throne alone; it escalates on the signals code reads
  ([solo mode](../docs/02-races.md)): a context checkpoint, an outward act it does not hold.
- **hierarchy** — the same ask, handed to the Elf (`elf-e`), down through its Orc and Slime.

Each mode may run under two **law arms**: law on, as every session runs, and law off —
`mnemorph run --no-law`, whose prompts carry none of the world's own words (its law, the creature
cards and docs, the Minds' manifests, the journal's opening). That measures what the fixed
per-session prompt buys against what it costs ([consumption](../docs/06-consumption.md)). Both arms
of a case share its trial key (`<task>/<mode>`) and run in a fresh world each. The switch is the
bench's command line alone: no configuration a world holds sets it, so no body can.

Every world is fresh: the task's starter (`tasks/NN-name/world/`), the bench's creatures
(`world/.mnemorph/`), a configuration naming the model through OpenRouter, and a git history. A
task passes when its `verify.py` — kept outside the world, so no body can edit it — exits 0.

## Running it

```sh
cargo build -p mnemorph-tui      # builds target/debug/mnemorph, which runs each ask
python3 bench/run.py --check    # every task fails untouched: no model, no key
python3 bench/run.py            # all 60 runs on the default free model
python3 bench/run.py --tasks 01,02 --modes solo --limit 2
python3 bench/run.py --tasks 01 --modes hierarchy --keep --out /tmp/look.jsonl   # a post-mortem
python3 bench/run.py --modes solo --law on,off  # the law arm: solo with and without the law
```

`--keep` leaves each run's world on disk and names it in the result, to read its journals;
`--out` writes the results elsewhere than the model's tracked file.

The key is `OPENROUTER_API_KEY` from the environment, else `~/.config/mnemorph/openrouter.key`
(mode 600 — a key file others can read is refused). It is handed only to each run's own process,
never written and never printed; a run's saved stderr is scrubbed of it before it is cut. `--model` (or `MNEMORPH_BENCH_MODEL`) picks the model; the default is
a free one.

Each run is one line in `results/<model>.jsonl`, tracked as the measurement's record: its case
(`trial`), its law arm (`law`) and the law the run itself said on its `--json` result line
(`law_said`), whether it passed, its exit, its ending and the session's `turnEnded` endings
(`turns`), the acts a rule refused, by rule (`refusals`, read from `--json`'s tool results), seconds, model calls (and how many came from bodies below the throne), weighted
tokens and dollars (from the world's usage ledger), and how many times solo escalated. A hierarchy
run in which no body below the throne made a call did not run as one: it is marked `valid: false`.
A rerun skips what is there. A rate limit or a daily cap — seen on stderr, or in a body's result
on stdout — stops the run with exit 3, unrecorded, and the next run resumes from it; a free
model's daily cap may take the sixty runs over a few days. A run past its timeout (65 minutes, beyond the core's one-hour clock) is
killed with every process it started; the report reads it as a hang, to rerun. `just check` also builds
`target/debug/mnemorph-guard`, the sandbox's helper: without it every shell act is refused.

## Reading it

```sh
python3 bench/report.py results/<model>.jsonl            # the whole matrix, or an error
python3 bench/report.py results/<model>.jsonl --partial  # a run still in progress, its gaps named
python3 bench/report.py results/<model>.jsonl --modes solo --law on,off   # the law arm
```

The matrix is fixed up front: every task in every mode under every law arm named (`--law`, `on`
alone by default), exactly once. A run that is missing — a whole arm included — extra or recorded
twice is an error, never dropped, and so is a run whose own `law_said` is not its arm's: the arm is
read from the run, never only from its label. `--partial` reports a run still in progress and
names what is missing, but an extra, doubled or mislabelled run is an error even then. A task
succeeds only when its verify passed, the run ended answered, no rule refused any of its acts, and
a hierarchy run ran below the throne. Every other run is told by why it ended — its `ending`, recorded by `run.py`, or for an
older row the `mnemorph: <ending>` line `mnemorph run` writes to stderr:

| Cause | Ending | Counts |
|---|---|---|
| escalated, checkpoint | `escalate: …`, `checkpoint: …` — the core's one-hour clock included | a failed task |
| gate | `failed: the end gate …`, a check that hung included | a failed task |
| breach | answered, but a rule refused one of its acts: an envelope, the human gate, a sandbox guard rule, the race's shelf | a failed task |
| refused | `failed: the model refused …` | a failed task |
| never delegated | a hierarchy run with no call below the throne | a failed task |
| machine | `failed: the end gate could not verify: …` — no kernel guard | not counted: rerun |
| hang | killed at the bench's limit, 65 minutes, past the core's own clock | not counted: rerun |
| harness bug | `failed: the conversation is malformed …` — flagged loudly | not counted: rerun |
| provider, interrupted | any other `failed: …`, `interrupted` | not counted: rerun |

A correct file left by a stop the harness made is a failed task, and the report says how many
there were and, per arm, what each failure was. A stop the task does not own measured nothing: the
report lists it to rerun and never counts it, and `run.py --redo-invalid` runs it again — the new
row stands in for it. A refusal the body recovered from still leaves a record: the run ended
answered, but it tried an act its rules do not let it do alone, so it is judged a breach — a
failed task — and each arm shows how many of its runs breached, and the refusals by rule.

Each estimand is a difference in task success rate between two arms that differ in one dimension,
over the same tasks: hierarchy minus solo under each law arm, and law on minus law off in each
mode. It has two 95% intervals. One is a paired bootstrap that resamples tasks — the groups —
never rows: 10 000 resamples, seeded, so the same rows give the same interval. The other is
Newcombe's hybrid score interval for paired proportions (Newcombe 1998, *Statistics in Medicine*
17:2635, method 10): each arm's Wilson interval, joined through the pairs' correlation; it holds
its coverage at small n, where the bootstrap does not, and the tests check it against the paper's
published values. The report names each method and states the estimand and the group count beside
them, and the discordant tasks — won by one arm only — both rest on; with fewer than ten, the
percentile bootstrap under-covers, and the report says to read Newcombe's. There is no single
aggregate score: each arm's success, verify-alone rate, breaches, median calls, weighted tokens
and seconds, escalations and dollars stand side by side.

`just bench` — part of `just check` — runs the report's tests and `run.py --check`, with no model:
every task's verify fails on its untouched starter, and no task hands its worker the answer (its
`verify.py`, or a file named `answer`, `expected` or `solution`, inside its world). `--json`
prints the report as data.

The arms are told apart in the run itself, never in a file the run could write: `run.py` sets the
switch on the command line, and reads the law back from the run's own `--json` result line. The
world's configuration holds only the model and its provider.
