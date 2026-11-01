# Mnemorph — the book

Mnemorph is the agent harness that grows out of isekai: the same nine natures, the same wire,
rebuilt as a Rust core with a Rust terminal app, `mnemorph`, plus a TypeScript SDK for the web
board. The races are **code**, not
configuration: what each one may do, whom it answers to, how it is scored and how it ascends is
compiled into the binary.

We start by writing, then we code. Every page reads the same way:

1. **In one picture** — a diagram you can understand in a minute.
2. **In plain words** — the idea, without jargon.
3. **Technical detail** — the exact rules, types and numbers the code implements.

## Diagrams

Two layers, one picture each:

- **Canvases** — the overview, races and ascensions, the loop, one order down and back, the
  client protocol, the architecture, the sandbox and consumption are drawn on tldraw's canvas:
  one document per page, `docs/canvas/<page>.tldr` (a tldraw page per sketch), and each sketch
  exported as `docs/canvas/<sketch>-light.svg` and `-dark.svg`, which the page shows in the
  reader's theme. A `.tldr` opens in [tldraw offline](https://github.com/tldraw/tldraw-offline),
  tldraw.com and tldraw's VS Code extension.
- **Mermaid sketches** — every page also keeps its diagrams as hand-drawn Mermaid in the text.
  They render on GitHub, diff in review, and are the source a canvas is drawn from: under a
  canvas, its sketch sits folded in a "The sketch in Mermaid" section.

A canvas is drawn, never edited by hand: `pnpm --filter @mnemorph/canvas draw` lays each sketch
out with tldraw's own Mermaid converter (`@tldraw/mermaid`) in headless Chromium and writes the
`.tldr` and both SVGs; the same sketches always make the same files. Its labels are written as
plain SVG text in tldraw's hand-drawn font, cut down to the characters they use, so an SVG holds
no HTML and fetches nothing. A canvas must read in the page's column: no wider than 2:1, its
labels at 10 px or more at 900 px wide, or the draw fails; a long sketch is re-laid for its canvas
only (the loop and the sandbox top-down, the architecture by ELK, one order's messages wrapped and
its rows spaced), its Mermaid text kept as written. A node its sketch dashes keeps the dashed
border. Each canvas links to its SVG at full size. The list of sketches and their layouts is
`tools/canvas/draw.mjs`'s; it needs the Chromium `@playwright/test` pins once (`pnpm --filter
@mnemorph/canvas exec playwright install chromium`).

When the code changes a flow, the sketch and its canvas change in the same change (Vitality).
Canvases are reviewed like code: a tldraw document or an SVG can carry script, and tldraw
offline's own `.tldraw` archive can embed a document script that runs when it is opened, so the
book keeps `.tldr` and SVG only. `node tools/check-canvas.mjs` (in `just docs` and CI) fails on
anything else in `docs/canvas/`, and on any script, event handler, `javascript:` URL, link or
source outside the file, `<foreignObject>`, embed or media there.

## Reading order

| # | Page | Read it to learn |
|---|---|---|
| 1 | [Overview](01-overview.md) | what Mnemorph is, the world, the processes |
| 2 | [Races](02-races.md) | every race and ascension: duty, powers, limits, score, fall |
| 3 | [The loop](03-loop.md) | the six beats of one turn, budgets, stress |
| 4 | [Communication](04-communication.md) | the wire between bodies, dispatch down and back, the client protocol |
| 5 | [Architecture](05-architecture.md) | crates, packages, processes, storage, sandbox |
| 6 | [Consumption](06-consumption.md) | how every token is budgeted, metered and stopped |
| 7 | [MCP](07-mcp.md) | the client: servers from your config, scoped by race, ranked, priced and learned; only the needed specs out |
| 8 | [Memory](08-memory.md) | the archive and the traces, recall with a seat for dissent, why only the gate writes memory |
| 9 | [Ontology](09-ontology.md) | who owns each path, facts with who said them, relations between worlds |
| 10 | [Roadmap](roadmap.md) | the order we build it in, rung by rung |
| — | [Lessons](lessons.md) | what isekai taught us, each one a test or a rule |
| — | [Releasing](releasing.md) | how a version is tagged, built, smoke-tested, drafted and published |
| — | [Race cards prompt](race-cards-prompt.md) | the brief for the card artwork |
| — | [Research: DIKWP-MESH](research/dikwp-mesh.md) | the mathematics behind the ontology's outer layer |

## The three words that matter

- **Race** — a kind of agent, compiled in: six races and five ascended forms. No config adds
  another.
- **Creature** — one living instance of a race, with a name and a territory: `slime-wire`,
  `orc-engine`. Creatures are data in `.mnemorph/`.
- **Body** — one running context that a creature rides for one commission. It dies with the
  task. Only what it wrote to the world survives it.
