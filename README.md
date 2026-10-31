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

Mnemorph ships two Rust binaries over the same core. Install both.

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

```sh
cargo install --path crates/mnemorph-tui      # mnemorph, into ~/.cargo/bin
pnpm install --frozen-lockfile && pnpm --filter @mnemorph/board build   # the board, built into…
cargo install --path crates/mnemorphd         # mnemorphd: the server, the board and the admin commands
```

`mnemorphd` embeds the board as it finds it built in `packages/board/dist` when it is compiled;
compiled without it, it serves a page saying so, and its API.

Name a provider and a model once, for every world, in `~/.config/mnemorph/config.toml`
(`$XDG_CONFIG_HOME/mnemorph/config.toml`). A world's `.mnemorph/config.local.toml` overrides it,
except for providers, caps, MCP servers and anything that loosens the gate or the sandbox, which
only this global file sets ([configuration](docs/05-architecture.md#configuration)):

```toml
[providers.openrouter]
kind = "openai"
base_url = "https://openrouter.ai/api"
key_env = "OPENROUTER_API_KEY"     # the key is read from this variable, never from the file

[models]
provider = "openrouter"
default = "anthropic/claude-sonnet-5.5"
```

### Run it

```sh
export OPENROUTER_API_KEY=…
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
