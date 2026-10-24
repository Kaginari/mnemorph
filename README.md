<p align="center"><img src="assets/brand/banner.jpg" alt="Mnemorph — the throne, the races, and the storm dragon over the city" width="100%"></p>

# Mnemorph

<p><img src="assets/portraits/rimuru@64.png" width="48" alt="Rimuru"> <img src="assets/portraits/elf@64.png" width="48" alt="Elf"> <img src="assets/portraits/orc@64.png" width="48" alt="Orc"> <img src="assets/portraits/slime@64.png" width="48" alt="Slime"> <img src="assets/portraits/kijin@64.png" width="48" alt="Kijin"> <img src="assets/portraits/dark-elf@64.png" width="48" alt="Dark Elf"> <img src="assets/portraits/high-elf@64.png" width="48" alt="High Elf"> <img src="assets/portraits/high-orc@64.png" width="48" alt="High Orc"> <img src="assets/portraits/named-slime@64.png" width="48" alt="Named Slime"> <img src="assets/portraits/oni@64.png" width="48" alt="Oni"> <img src="assets/portraits/arch-dark-elf@64.png" width="48" alt="Arch Dark Elf"></p>

An agent harness grown from [isekai](https://github.com/Kaginari/isekai): the nine natures and
the wire, rebuilt with a Rust core and a TypeScript terminal client and web board. Races are
compiled in — what each one may do, whom it answers to, how it is scored and how it ascends is
code, not configuration.

**Status:** M0 — foundations. Start with [the book](docs/README.md); the plan is the
[roadmap](docs/roadmap.md) and its [milestones](https://github.com/Kaginari/mnemorph/milestones).

| Read | For |
|---|---|
| [Overview](docs/01-overview.md) | the world in one picture |
| [Races](docs/02-races.md) | the six races, five ascensions, Kijin posts, the Dark Elf's witness rule |
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
