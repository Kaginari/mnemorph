<p align="center"><img src="assets/brand/banner.jpg" alt="Mnemorph — the throne, the races, and the storm dragon over the city" width="100%"></p>

# Mnemorph

<p><img src="assets/portraits/rimuru@64.png" width="48" alt="Rimuru"> <img src="assets/portraits/elf@64.png" width="48" alt="Elf"> <img src="assets/portraits/orc@64.png" width="48" alt="Orc"> <img src="assets/portraits/slime@64.png" width="48" alt="Slime"> <img src="assets/portraits/kijin@64.png" width="48" alt="Kijin"> <img src="assets/portraits/dark-elf@64.png" width="48" alt="Dark Elf"> <img src="assets/portraits/high-elf@64.png" width="48" alt="High Elf"> <img src="assets/portraits/high-orc@64.png" width="48" alt="High Orc"> <img src="assets/portraits/named-slime@64.png" width="48" alt="Named Slime"> <img src="assets/portraits/oni@64.png" width="48" alt="Oni"> <img src="assets/portraits/arch-dark-elf@64.png" width="48" alt="Arch Dark Elf"></p>

An agent harness grown from [isekai](https://github.com/Kaginari/isekai): the nine natures and
the wire, rebuilt as a Rust core with a Rust terminal app — `mnemorph` — plus a TypeScript SDK
and a web board. Races are compiled in — what each one may do, whom it answers to, how
it is scored and how it ascends is code, not configuration.

**Status:** M0–M6 and M8 are built — the wire and the races, the ledger, the providers, the tools,
sandbox and human gate, configuration, the loop with its long runs and resume, the protocol and
the terminal app, memory and the toolbox. M7, the world, runs with two items open (the Jev
decisions and the paired solo-versus-hierarchy measurement). M9, the Kijin, has its typed outward
acts, fingerprints, receipts, the act runner, closure and the typed pull request — a branch and
its pull request under one approval; its exit is met against a stub forge, a live run remains. M10's Dark Elf specialities, their routing and the wire's finding fields
are compiled. M12's board serves every world on loopback — chats, approvals under the terminal's
protections, memory, ontology, bodies and metrics, live. Start with [the book](docs/README.md); the plan is the
[roadmap](docs/roadmap.md) and its [milestones](https://github.com/Kaginari/mnemorph/milestones).

| Read | For |
|---|---|
| [Overview](docs/01-overview.md) | the world in one picture |
| [Races](docs/02-races.md) | the six races, five ascensions, Kijin posts and outward acts, the Dark Elf's specialities and witness rule |
| [The loop](docs/03-loop.md) | the six beats of one turn, budgets, stress, resume |
| [Communication](docs/04-communication.md) | the wire between bodies, the lanes, the client protocol |
| [Architecture](docs/05-architecture.md) | crates, packages, the terminal UI, storage, sandbox, configuration |
| [Consumption](docs/06-consumption.md) | how every token is budgeted, metered and stopped |
| [MCP](docs/07-mcp.md) | the MCP client, agent-mcp and toolbox-mcp (M13) |
| [Memory](docs/08-memory.md) | the archive, notes as traces, recall, the write rule |
| [Ontology](docs/09-ontology.md) | who owns each path, facts with who said them |
| [Lessons](docs/lessons.md) | what isekai taught, each one a test or a rule |
| [Roadmap](docs/roadmap.md) | the book, then M0–M14, rung by rung |
| [Releasing](docs/releasing.md) | how a version is tagged, built, smoke-tested, drafted and published |

## Set up a machine

| Tool | Version | Install |
|---|---|---|
| Rust | pinned in `rust-toolchain.toml` | [rustup](https://rustup.rs) — the pin installs itself |
| cargo-nextest, cargo-deny, cargo-llvm-cov | latest | prebuilt: [nexte.st](https://nexte.st), [cargo-deny releases](https://github.com/EmbarkStudios/cargo-deny/releases) |
| just | latest | [just.systems](https://just.systems) |
| actionlint | latest | [releases](https://github.com/rhysd/actionlint/releases) |
| Node | 22 (`.nvmrc`) | your usual Node manager |
| pnpm | pinned in `package.json` | `corepack enable` |

```sh
just setup   # install the TypeScript workspace
just         # the full check: Rust, TypeScript, the layering, the book, the workflows
just docs-dev  # read the book as a website while writing it
```

### Install `mnemorph`

Mnemorph ships two Rust binaries over the same core — and, on Linux, `mnemorph-guard`, the kernel
guard every command runs through, found beside `mnemorph`. Install them together.

| Binary | For | Use it to |
|---|---|---|
| `mnemorph` | you, at a terminal, and your scripts | talk to Rimuru in the terminal app (`mnemorph`), ask once without it (`mnemorph run "<ask>"`), pick up a run that stopped (`mnemorph resume <run|last>`), check the setup (`mnemorph status`) |
| `mnemorphd` | other programs, and the world's upkeep | serve the client protocol (JSON-RPC over stdio) to the TypeScript SDK, and the web board on loopback (`mnemorphd board`); run the admin commands: `runs`, `memory`, `onto`, `q`, `incident`, `housekeep`, `relief` |

They are two binaries because they answer two kinds of caller. `mnemorph` holds the session in
its own process, so nothing sits between the screen and the core. `mnemorphd` is the core
**served**: a program that is not Rust — the SDK, an editor plugin — starts it as a child and speaks
the protocol to it, and the web board (from M12) is served by it, so every client sees the same
sessions, gate and events. The world's
upkeep commands live with the server because they read and change the world's records directly,
without a session. Both share the same crates, so a rule enforced in one is enforced in the other.

**With the installer** (Linux x86_64 and arm64, macOS Apple Silicon and Intel):

```sh
curl -fsSL https://raw.githubusercontent.com/Kaginari/mnemorph/main/install.sh | sh
sh install.sh --version 1.0.0 --dir ~/bin    # a given release, somewhere else
```

It downloads the release's archive for this machine, refuses it unless it matches the release's
`SHA256SUMS`, and puts the binaries in `~/.local/bin` (`MNEMORPH_INSTALL_DIR` or `--dir` to
choose). It needs no sudo and edits no shell profile: when the directory is not on PATH, it
prints the line to add.

Once a release carries build provenance ([releasing](docs/releasing.md#provenance-signing)), an
archive can also be checked against the workflow that built it:
`gh attestation verify mnemorph-<target>.tar.gz --repo Kaginari/mnemorph`.

**By hand**, from [the releases](https://github.com/Kaginari/mnemorph/releases): download
`mnemorph-<target>.tar.gz` and `SHA256SUMS`, check, unpack, and put the binaries side by side on
PATH:

```sh
sha256sum -c --ignore-missing SHA256SUMS     # macOS: shasum -a 256 -c --ignore-missing SHA256SUMS
tar -xzf mnemorph-x86_64-unknown-linux-gnu.tar.gz
cp mnemorph-*/mnemorph mnemorph-*/mnemorphd mnemorph-*/mnemorph-guard ~/.local/bin/   # no guard on macOS
```

**From source**, in a checkout:

```sh
cargo install --path crates/mnemorph-tui      # mnemorph, into ~/.cargo/bin
cargo install --path crates/mnemorph-sandbox  # mnemorph-guard, beside it (Linux)
pnpm install --frozen-lockfile && pnpm --filter @mnemorph/board build   # the board, built into…
cargo install --path crates/mnemorphd         # mnemorphd: the server, the board and the admin commands
```

`mnemorphd` embeds the board as it finds it built in `packages/board/dist` when it is compiled;
compiled without it, it serves a page saying so, and its API. On Linux, `mnemorph` without
`mnemorph-guard` beside it runs no command, and `mnemorph status` says so. How a release is cut,
tested and published: [releasing](docs/releasing.md); what changed: [the changelog](CHANGELOG.md).

Name a provider and a model once, for every world, in `~/.config/mnemorph/config.toml`
(`$XDG_CONFIG_HOME/mnemorph/config.toml`). A world's `.mnemorph/config.local.toml` overrides it,
except for providers, caps, MCP servers and anything that loosens the gate or the sandbox, which
only this global file sets ([configuration](docs/05-architecture.md#configuration)):

```toml
[providers.openrouter]
kind = "openai"
base_url = "https://openrouter.ai/api"
key_env = "OPENROUTER_API_KEY"     # the variable that holds the key (never write the key here)
key_file = "~/.config/mnemorph/openrouter.key"  # used when the variable is unset; chmod 600

[models]
provider = "openrouter"
default = "anthropic/claude-sonnet-5.5"
```

`key_file` is read only from this global file, never from a world's config. The file must be
yours, mode `600`, and outside every world.

### Run it

```sh
export OPENROUTER_API_KEY=…         # or keep it in the key_file, which every shell and the board read
mnemorph                            # a session in this world, in the terminal app
mnemorph --world ~/code/project     # …in another
mnemorph status                     # the model, the key's origin, the sandbox, the config's holes
mnemorph run "<ask>"                # one ask without the app: the answer on stdout
echo "<ask>" | mnemorph run --json  # every event as a JSON line, then a result line
mnemorph resume last                # pick up the newest run that stopped
mnemorphd runs                      # the runs that may be resumed
mnemorphd board                     # the board on 127.0.0.1:7878: open the URL it prints
```

`mnemorphd board` prints `http://127.0.0.1:7878/#k=<key>`; the key works once, traded by the page
for a session cookie. It listens on loopback only — from another machine, forward the same port:
`ssh -L 7878:127.0.0.1:7878 <host>`, then open the printed URL. A second `mnemorphd board` prints
the running board's URL, with the key waiting now ([the board server](docs/05-architecture.md#the-board-server)).
Each link works once, in one browser: a link VS Code's browser opened first is spent for Chrome.
The board answers at `127.0.0.1`, `[::1]` and `localhost`, and refuses every other host name, to
block DNS rebinding.

To have `mnemorph` start the board (or reuse the running one) and open it as a session opens, say
so in your global file, `~/.config/mnemorph/config.toml` — a world's file never can:

```toml
[board]
autostart = true
open = true
```

The board outlives the app; `mnemorphd board stop` stops it.

The board is every world on the machine in one page: its worlds, chats in the terminal's grammar,
the approval card with the terminal's protections, memory, ontology, bodies and metrics, live.
⌘K (Ctrl+K) jumps anywhere; `?` lists every shortcut ([the board](docs/05-architecture.md#the-board)).

| | Dark | Light |
|---|---|---|
| Worlds | <img src="docs/img/board/worlds-dark.png" width="400" alt="The worlds page, dark: each world's status, memory standing, gate and spend"> | <img src="docs/img/board/worlds-light.png" width="400" alt="The worlds page, light"> |
| Chat | <img src="docs/img/board/chat-dark.png" width="400" alt="A chat, dark: tool cards, a numbered diff, the end gate's verdict, the session pane"> | <img src="docs/img/board/chat-light.png" width="400" alt="A chat, light"> |
| Approval | <img src="docs/img/board/approval-dark.png" width="400" alt="An approval card, dark: the act whole with every hidden character escaped, Yes disabled until read"> | <img src="docs/img/board/approval-light.png" width="400" alt="An approval card, light"> |
| Metrics | <img src="docs/img/board/metrics-dark.png" width="400" alt="The metrics page, dark: totals, spend per day by race, cache hit, burn"> | <img src="docs/img/board/metrics-light.png" width="400" alt="The metrics page, light"> |
| Phone | <img src="docs/img/board/phone-chat-dark.png" width="160" alt="A chat at phone width, dark: the rail behind the menu"> | <img src="docs/img/board/phone-chat-light.png" width="160" alt="A chat at phone width, light"> |

`mnemorph run` goes through the same session and end gate as the app, and exits 0 only when the
answer landed; it denies every act that needs your approval unless you pass `--gate-tty`
([`mnemorph run`](docs/05-architecture.md#mnemorph-run)). From a checkout, `cargo run -p
mnemorph-tui --` runs the same binary.

## Layout

| Path | Holds |
|---|---|
| `crates/` | the Rust core — one crate per concern, layered ([architecture](docs/05-architecture.md)) |
| `packages/` | the TypeScript side: protocol, sdk, theme, board |
| `docs/` | the book |
| `site/` | the book as a website (VitePress) |
| `assets/` | race portraits and cards |
| `.mnemorph/` | Mnemorph's own world: law, creatures, journal |
| `tools/` | repository checks |
