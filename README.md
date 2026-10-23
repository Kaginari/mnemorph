# Tempest

An agent harness grown from [isekai](https://github.com/Kaginari/isekai): the nine natures and
the wire, rebuilt with a Rust core and a TypeScript terminal client and web board. Races are
compiled in — what each one may do, whom it answers to, how it is scored and how it ascends is
code, not configuration.

**Status:** M0 — foundations. Start with [the book](docs/README.md); the plan is the
[roadmap](docs/roadmap.md) and its [milestones](https://github.com/Kaginari/tempest/milestones).

| Read | For |
|---|---|
| [Overview](docs/01-overview.md) | the world in one picture |
| [Races](docs/02-races.md) | the six races, five ascensions, Kijin posts, the Dark Elf's witness rule |
| [Roadmap](docs/roadmap.md) | the book, then M0–M14, rung by rung |

## Set up a machine

| Tool | Version | Install |
|---|---|---|
| Rust | pinned in `rust-toolchain.toml` | [rustup](https://rustup.rs) — the pin installs itself |
| cargo-nextest, cargo-deny | latest | prebuilt: [nexte.st](https://nexte.st), [cargo-deny releases](https://github.com/EmbarkStudios/cargo-deny/releases) |
| just | latest | [just.systems](https://just.systems) |
| Node | 22 (`.nvmrc`) | your usual Node manager |
| pnpm | pinned in `package.json` | `corepack enable` |

```sh
just setup   # install the TypeScript workspace
just         # the full check: Rust, TypeScript, the layering, a clean tree
```

## Layout

| Path | Holds |
|---|---|
| `crates/` | the Rust core — one crate per concern, layered ([architecture](docs/05-architecture.md)) |
| `packages/` | the TypeScript clients: protocol, sdk, cli, board |
| `docs/` | the book |
| `assets/` | race portraits and cards |
| `.tempest/` | Tempest's own world: law, creatures, journal |
| `tools/` | repository checks |
