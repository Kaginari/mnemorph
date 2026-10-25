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
```

The key is `OPENROUTER_API_KEY` from the environment, else `~/.config/mnemorph/openrouter.key`
(mode 600). It is handed only to each run's own process, never written and never printed; a run's
saved stderr has it scrubbed. `--model` (or `MNEMORPH_BENCH_MODEL`) picks the model; the default is
a free one.

Each run is one line in `results/<model>.jsonl`: whether it passed, its exit, seconds, model calls,
weighted tokens and dollars (from the world's usage ledger), and how many times solo escalated. A
rerun skips what is there. A rate limit or a daily cap stops the run with exit 3, unrecorded, and
the next run resumes from it — a free model's daily cap may take the sixty runs over a few days.
