# Terminal-Bench, through Harbor

Status: **the profile is built (#365); the adapter has a hierarchy mode and keeps its journals,
and has not yet run in a container.** The first runs are on the owner's server, not in CI.

## Why

[Terminal-Bench 2.0](https://www.tbench.ai/docs) is 89 tasks (4 easy, 55 medium, 30 hard), each a
Docker environment with hidden tests, across software, science, ML, operations, hardware, security
and media. It is the newest Terminal-Bench in Harbor 0.25's registry: `terminal-bench@4.0`, which
this page named before, is not there. Every task runs through
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

Built in #365. A machine made to be thrown away is its own sandbox. One key in the human's
**global** file says so, and it is a loosening key, so a world's file naming it is a hole:

```toml
[sandbox]
disposable = true
```

It takes effect only when the process also runs in a container: Docker's or Podman's marker file,
or a container runtime in `/proc/1/cgroup`. Under it:

| Rule | Normal | Disposable |
|---|---|---|
| `install`, `fetch` and writes outside the world through `bash` | outward: the Kijin's, or the human's | allowed to the body, each one journaled |
| An act the classifier names only `other`: a socket, a remote shell, `systemctl`, `crontab`, a `curl` POST, a push it could not resolve | outward | **still refused** |
| Pushing, pull requests, comments, labels, tags, publishing, deploys, secret stores | refused or the human's | **still refused** |
| The file tools (`write`, `edit`) | inside the world | inside the world: a body writes `/etc` through `bash` |
| The kernel guard | required, or `allow_unguarded` | not required: the container is the boundary |
| Mnemorph's own journals and state, the end gate, the ledger, the receipts | as ever | as ever |

The first build also let `other` through, and its own test ran a `git push`, so `other` is never
contained. The cost is real: a task that starts a service is refused. Measure first, then decide
whether a narrower `service` kind is worth its own rule.

## The adapter

`mnemorph_agent.py` subclasses `BaseInstalledAgent`:

1. **install:** uploads the three binaries (`mnemorph`, `mnemorphd`, `mnemorph-guard`) from
   `$MNEMORPH_DIST` to `/usr/local/bin`. The task images go back to Debian bullseye (glibc 2.31),
   so the binaries are built for it: `cargo zigbuild --release --target
   x86_64-unknown-linux-gnu.2.31`. Once 1.0 is out, `install.sh` replaces the upload.
2. **run:** writes the global configuration to a private `XDG_CONFIG_HOME` (the provider, the
   model, `disposable`, the steps and limits), snapshots the workspace in git if it is not a
   repository, as Harbor's Aider adapter does, then runs
   `mnemorph run "<task>" --world . --json`. The events go to `/logs/agent/mnemorph.jsonl`, and
   the loop's journals, the usage ledger and `log.md` to `/logs/agent/`, every 30 seconds and at
   the end, so a run killed at its timeout still leaves its trace and its spend.
   With `--ak mode=hierarchy`, it first writes a team over the task's folder and adopts it: an Elf,
   an Orc and `slime-workspace`, each over every top-level directory, the workspace Slime holding
   the root files too. The ask then goes to Summoner with the large bench's words: hand it to
   `elf-e` as a brief.
3. **populate_context_post_run:** reads the `result` line's `usage` into Harbor's token and cost
   fields.

## On a server

    harbor run -p <terminal-bench-2.0 dir> -i <task> ... -m openrouter/<model> \
      --agent-import-path bench.harbor.mnemorph_agent:Mnemorph --ak mode=solo -n 2 --jobs-dir jobs

with `PYTHONPATH` at the repository root, `MNEMORPH_DIST` and `OPENROUTER_API_KEY` set, and Docker.
Each task asks 1 to 4 CPUs and 2 to 8 GB.

## In CI

`terminal-bench.yml` is a manual workflow. The bot cannot write `.github/workflows/`, so the owner
copies it there. It takes a model and a task limit, builds the binaries, installs Harbor with `uv`,
runs `harbor run -d terminal-bench@2.0 --agent-import-path`, and uploads the jobs directory. It
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
2. ~~Build the profile, test-first~~ (#365).
3. The adapter, with the workflow in `.github/workflows/`. One smoke run on three tasks.
4. The comparison in step 1 above.
