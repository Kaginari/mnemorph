# Tempest — the book

Tempest is the agent harness that grows out of isekai: the same nine natures, the same wire,
rebuilt with a Rust core and a TypeScript CLI and board. The races are **code**, not
configuration: what each one may do, whom it answers to, how it is scored and how it ascends is
compiled into the binary.

We start by writing, then we code. Every page reads the same way:

1. **In one picture** — a diagram you can understand in a minute.
2. **In plain words** — the idea, without jargon.
3. **Technical detail** — the exact rules, types and numbers the code implements.

## Diagrams

Two layers, one picture each:

- **Canvases** — the architecture, the races, the loop and the communication flows are drawn in
  [tldraw offline](https://github.com/tldraw/tldraw-offline) as `docs/canvas/<page>.tldraw`, and
  exported next to them as `docs/canvas/<page>.svg`, which the pages embed. An agent connected to
  the open canvas draws and updates them; you rearrange by hand.
- **Mermaid sketches** — every page also keeps its diagrams as hand-drawn Mermaid in the text.
  They render on GitHub, diff in review, and are the layout a canvas is drawn from.

When the code changes a flow, the sketch and the canvas change in the same change (Vitality).
tldraw offline does not merge external edits to an open file: close a canvas before a pull,
checkout or rebase touches it.

## Reading order

| # | Page | Read it to learn |
|---|---|---|
| 1 | [Overview](01-overview.md) | what Tempest is, the world, the processes |
| 2 | [Races](02-races.md) | every race and ascension: duty, powers, limits, score, fall |
| 3 | [The loop](03-loop.md) | the six beats of one turn, budgets, stress |
| 4 | [Communication](04-communication.md) | the wire between bodies, dispatch down and back, the client protocol |
| 5 | [Architecture](05-architecture.md) | crates, packages, processes, storage, sandbox |
| 6 | [Consumption](06-consumption.md) | how every token is budgeted, metered and stopped |
| 7 | [Roadmap](roadmap.md) | the order we build it in, rung by rung |
| — | [Lessons](lessons.md) | what isekai taught us, each one a test or a rule |
| — | [Race cards prompt](race-cards-prompt.md) | the brief for the card artwork |

## The three words that matter

- **Race** — a kind of agent, compiled in: six races and five ascended forms. No config adds
  another.
- **Creature** — one living instance of a race, with a name and a territory: `slime-wire`,
  `orc-engine`. Creatures are data in `.tempest/`.
- **Body** — one running context that a creature rides for one commission. It dies with the
  task. Only what it wrote to the world survives it.
