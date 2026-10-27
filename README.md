<p align="center"><img src="assets/brand/banner.jpg" alt="Mnemorph — the throne, the races, and the storm dragon over the city" width="100%"></p>

# Mnemorph

<p><img src="assets/portraits/rimuru@64.png" width="48" alt="Rimuru"> <img src="assets/portraits/elf@64.png" width="48" alt="Elf"> <img src="assets/portraits/orc@64.png" width="48" alt="Orc"> <img src="assets/portraits/slime@64.png" width="48" alt="Slime"> <img src="assets/portraits/kijin@64.png" width="48" alt="Kijin"> <img src="assets/portraits/dark-elf@64.png" width="48" alt="Dark Elf"> <img src="assets/portraits/high-elf@64.png" width="48" alt="High Elf"> <img src="assets/portraits/high-orc@64.png" width="48" alt="High Orc"> <img src="assets/portraits/named-slime@64.png" width="48" alt="Named Slime"> <img src="assets/portraits/oni@64.png" width="48" alt="Oni"> <img src="assets/portraits/arch-dark-elf@64.png" width="48" alt="Arch Dark Elf"></p>

An agent harness grown from [isekai](https://github.com/Kaginari/isekai): the nine natures and
the wire, rebuilt as a Rust core with a Rust terminal app — `mnemorph` — plus a TypeScript SDK
for the web board to come. Races are
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

### Install `mnemorph`

`mnemorph` is one Rust binary: the terminal app, and `mnemorph run` for scripts. It hosts the
session in its own process. `mnemorphd` is the core as a server — the client protocol for the
SDK and the board — and holds the world's admin commands.

```sh
cargo install --path crates/mnemorph-tui      # mnemorph, into ~/.cargo/bin
cargo install --path crates/mnemorphd         # mnemorphd: the server and the admin commands
```

Name a provider and a model once, for every world, in `~/.config/mnemorph/config.toml`
(`$XDG_CONFIG_HOME/mnemorph/config.toml`); a world's `.mnemorph/config.local.toml` overrides it
([configuration](docs/05-architecture.md#configuration)):

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
```

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
