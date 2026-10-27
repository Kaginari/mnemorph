<p align="center"><img src="assets/brand/banner.jpg" alt="Mnemorph — the throne, the races, and the storm dragon over the city" width="100%"></p>

# Mnemorph

<p><img src="assets/portraits/rimuru@64.png" width="48" alt="Rimuru"> <img src="assets/portraits/elf@64.png" width="48" alt="Elf"> <img src="assets/portraits/orc@64.png" width="48" alt="Orc"> <img src="assets/portraits/slime@64.png" width="48" alt="Slime"> <img src="assets/portraits/kijin@64.png" width="48" alt="Kijin"> <img src="assets/portraits/dark-elf@64.png" width="48" alt="Dark Elf"> <img src="assets/portraits/high-elf@64.png" width="48" alt="High Elf"> <img src="assets/portraits/high-orc@64.png" width="48" alt="High Orc"> <img src="assets/portraits/named-slime@64.png" width="48" alt="Named Slime"> <img src="assets/portraits/oni@64.png" width="48" alt="Oni"> <img src="assets/portraits/arch-dark-elf@64.png" width="48" alt="Arch Dark Elf"></p>

An agent harness grown from [isekai](https://github.com/Kaginari/isekai): the nine natures and
the wire, rebuilt with a Rust core and a TypeScript terminal client and web board. Races are
compiled in — what each one may do, whom it answers to, how it is scored and how it ascends is
code, not configuration.

**Status:** M0–M6 and M8 are built — the wire and the races, the ledger, the providers, the tools,
sandbox and human gate, configuration, the loop with its long runs and resume, the protocol and
the terminal clients, memory and the toolbox. M7, the world, runs with two items open (the Jev
decisions and the paired solo-versus-hierarchy measurement). M9, the Kijin, has its typed outward
acts, fingerprints, receipts, the act runner and closure; the typed pull-request act its exit
needs is in progress. M10's Dark Elf specialities and their routing are compiled; the board is
M12. Start with [the book](docs/README.md); the plan is the
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

Run Mnemorph from the checkout once `just` has built it — the CLI starts the core built there:

```sh
pnpm --filter @mnemorph/cli build
node packages/cli/dist/cli.js status          # the build, the key's origin, the sandbox in force
node packages/cli/dist/cli.js                 # a session in this world
node packages/cli/dist/cli.js run "<ask>"     # one ask, the answer on stdout
```

Or the terminal UI in Rust, the core in its own process
([the terminal UI](docs/05-architecture.md#the-terminal-ui)):

```sh
cargo run -p mnemorph-tui -- --world .
```

## Layout

| Path | Holds |
|---|---|
| `crates/` | the Rust core — one crate per concern, layered ([architecture](docs/05-architecture.md)) |
| `packages/` | the TypeScript clients: protocol, sdk, cli, board |
| `docs/` | the book |
| `site/` | the book as a website (VitePress) |
| `assets/` | race portraits and cards |
| `.mnemorph/` | Mnemorph's own world: law, creatures, journal |
| `tools/` | repository checks |
