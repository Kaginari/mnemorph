# Terminal-Bench, through Harbor

Status: **design, for review.** The adapter beside this page is a draft that has never run; the
container profile it needs is not built. Nothing here spends until both are reviewed.

## Why

[Terminal-Bench 4.0](https://www.tbench.ai/docs) is 66 tasks, each a Docker environment with hidden
tests, across software, science, ML, operations, hardware, security and media. The best published
harness and model pairs score about 65%. Every task runs through
[Harbor](https://pypi.org/project/harbor/), which installs an agent into the task's container, runs
it, then runs the tests. Harbor 0.25 calls a harness through a `BaseInstalledAgent`: `install`,
`run` and `populate_context_post_run`.

## What stops Mnemorph today

A terminal task installs packages, edits `/etc`, starts services and fetches files. Mnemorph
refuses all four, by design:

| A task does | Mnemorph's class today | So, headless and solo |
|---|---|---|
| `pip install`, `apt-get install`, `npm install` | `install`, outward | refused: no Kijin, no human to ask |
| `curl`, `wget` | network, outward | refused |
| `systemctl`, `crontab` | outward | refused |
| writes under `/etc`, `/usr/local` | outside the world | `write` and `edit` refuse; `bash` needs the guard's grant |
| any command | needs a kernel guard | Docker's default seccomp may block the guard's namespaces |

Run as it is, Mnemorph would fail most tasks on the first install. That would measure the rules,
not the harness.

## The container profile

A machine made to be thrown away is its own sandbox. One key in the human's **global** file says
so, and it is a loosening key, so a world's file naming it is a hole:

```toml
[sandbox]
disposable = true
```

It takes effect only when the process also sees a container (`/.dockerenv`, or a container cgroup).
It is never inferred from either alone. Under it:

| Rule | Normal | Disposable |
|---|---|---|
| The world | the repository | `--world`, with the solo body's soil the whole filesystem |
| `install`, `fetch` and network | outward: the Kijin's, or the human's | allowed to the body, each one journaled |
| Publishing, pushing, a forge's API, secret stores | refused or the human's | **still refused** |
| The kernel guard | required, or `allow_unguarded` | not required: the container is the boundary |
| Mnemorph's own journals and state | out of every body's reach | the same |
| The end gate, the ledger, the receipts | as ever | as ever |

What it does not loosen is what leaves the container for good: pushing, publishing and
credentials. The key in the run's environment goes only to the provider, as today.

## The adapter

`mnemorph_agent.py` subclasses `BaseInstalledAgent`:

1. **install:** uploads the three binaries a CI job built (`mnemorph`, `mnemorphd`,
   `mnemorph-guard`) from `$MNEMORPH_DIST` to `/usr/local/bin`. There is no release yet for
   `install.sh` to fetch. Once 1.0 is out, `install.sh` replaces the upload.
2. **run:** writes the global configuration to a private `XDG_CONFIG_HOME` (the provider, the
   model, `disposable`, the steps and limits), snapshots the workspace in git if it is not a
   repository, as Harbor's Aider adapter does, then runs
   `mnemorph run "<task>" --world . --json`. The events go to `/logs/agent/mnemorph.jsonl`.
3. **populate_context_post_run:** reads the `result` line's `usage` into Harbor's token and cost
   fields.

## In CI

`terminal-bench.yml` is a manual workflow. The bot cannot write `.github/workflows/`, so the owner
copies it there. It takes a model and a task limit, builds the binaries, installs Harbor with `uv`,
runs `harbor run -d terminal-bench@4.0 --agent-import-path`, and uploads the jobs directory. It
needs one secret, `OPENROUTER_API_KEY`. GitHub's Ubuntu runners have Docker.

## How it is judged

1. **Same model, two harnesses.** Mnemorph against Harbor's own Terminus 2, on one model, every
   task, the same attempts. This is the first claim worth making: what the harness adds.
2. **Then the arms that won on the large suite:** solo, then `auto`, then a checked arm, where
   the task's own text becomes the gate's check before the work.
3. **Only then a frontier model,** once a cheap one shows the harness adds something. A full
   frontier run costs hundreds of dollars.

## Order

1. Review this page.
2. Build the profile, test-first: classes allowed under it, refusals kept, the container check.
3. The adapter, with the workflow in `.github/workflows/`. One smoke run on three tasks.
4. The comparison in step 1 above.
