---
name: mnemorph-theme
description: Mnemorph's visual theme — the holographic HUD look. Use when building or styling any Mnemorph surface: the board, the docs site, the CLI's colours, cards, decks, banners or other images.
---

# The Mnemorph theme

**Techno-arcane**: a fantasy world seen through a holographic HUD. Deep night surfaces, one cyan
**holo** accent that glows, gold kept for the throne and titles, every race in its **hex**. The
banner (`assets/brand/banner.jpg`) and the icon sheet (`assets/source/race-icons.jpg`) are the
reference look; when in doubt, match them.

## The source of truth

- `packages/theme/src/tokens.css` — the web tokens (`--mn-*`), dark by default, light under
  `:root[data-theme="light"]`, and the components below. Import it as `@mnemorph/theme/tokens.css`.
- `packages/theme/src/index.ts` — the same palette for TypeScript (`dark`, `light`, `races`) and
  `contrast()`. The CLI's Ink colours come from here.
- Its test holds the two files equal and every text colour at WCAG AA on every surface. A new
  colour goes into both files and the test's lists.

Read colours from tokens, always: a hex literal in a component is a second source of truth.

## The vocabulary

| Word | Class / token | Meaning |
|---|---|---|
| surface | `--mn-bg`, `--mn-panel`, `--mn-panel-raised` | page, container, a container on a container |
| **holo** | `--mn-holo`, `--mn-glow`, `--mn-glow-soft` | the interface: borders, links, focus, the live thing |
| alarm | `--mn-alarm` | errors, refusals, the adversary's findings |
| gold | `--mn-gold` | the throne, titles, an *active* flash — sparingly |
| race colour | `--mn-race-<race>` / `races.<race>` | anything that belongs to one race: its lane, prefix, card |
| panel | `.mn-panel` | rounded, thin holo border, inner glow |
| **hex** | `.mn-hex` | the frame of every race icon |
| title | `.mn-title` | Cinzel in the banner's gold ramp — the name and page titles only |
| data | `.mn-mono` | ids, logs, numbers, wire lines |
| interactive | `.mn-interactive` | hover: border brightens and pulses, one scan line; active: gold flash |
| stream | `.mn-stream` | faint scrolling code behind a page — decoration, never content |

Type: Cinzel for titles, Rajdhani for UI labels and headings, JetBrains Mono for data, the system
sans for prose — long reading stays in a proportional face.

## The assets

| What | Where |
|---|---|
| the emblem (dragon and crowned slime) | `assets/brand/mnemorph-icon{,@512,@256,@128,@64,@32}.png`, `favicon.ico` |
| the banner, the link card | `assets/brand/banner.jpg`, `assets/brand/social-preview.jpg` |
| race icons, already cut to hexagons | `assets/portraits/<race>{,@64,@32}.png` |
| race cards | `assets/cards/<race>.png` — drawn by `python3 tools/cards.py` |
| fonts for images | `assets/fonts/` (Cinzel, Rajdhani; OFL) |

## A surface is done when

1. Every colour, font and radius is a token.
2. It reads in **both** modes: switch `data-theme` and look at each.
3. Motion stops under `prefers-reduced-motion` — the tokens' components already do; new animation
   joins that block.
4. Focus is visible on every interactive element (`.mn-interactive` gives the holo ring).
5. Diagrams and other paper-drawn images keep their light sheet in dark mode (the site's
   `.mermaid` rule).
6. You have **seen** it: a screenshot of each mode, not only a passing build.
