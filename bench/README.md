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
