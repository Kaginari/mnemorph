# Architecture

## In one picture

```mermaid
---
config:
  look: handDrawn
  theme: neutral
  flowchart:
    wrappingWidth: 360
---
flowchart TB
    subgraph ts["TypeScript — packages/"]
      CLI["cli<br/>Ink terminal UI"]
      BOARD["board<br/>React web cockpit"]
      SDK["sdk<br/>client, reconnect, typed calls"]
      PROTO["protocol<br/>generated types"]
      CLI --> SDK
      BOARD --> SDK
      SDK --> PROTO
    end
    subgraph rs["Rust — crates/"]
      TUI["tui<br/>Ratatui terminal UI, the session in-process"]
      SERVER["server<br/>JSON-RPC · stdio · WebSocket"]
      LOOP["loop<br/>six beats"]
      WORLD["world<br/>creatures · territory · dispatch · gate"]
      RACES["races<br/>the compiled law"]
      WIRE["wire<br/>envelope"]
      TOOLS["tools<br/>read · write · bash · git · gh"]
      SANDBOX["sandbox<br/>landlock · seccomp · Seatbelt"]
      PROV["provider<br/>Anthropic · OpenAI-compatible"]
      LEDGER["ledger<br/>usage · value · budgets"]
      MEM["memory<br/>tiers · recall · write rule"]
      CONFIG["config<br/>layers · origins · loosening"]
      SERVER --> LOOP
      TUI --> SERVER
      TUI --> LOOP
      LOOP --> CONFIG --> TOOLS
      CONFIG --> RACES
      CONFIG --> LEDGER
      LOOP --> WORLD --> RACES
      LOOP --> PROV --> LEDGER
      LOOP --> TOOLS --> SANDBOX
      TOOLS --> RACES
      LOOP --> LEDGER --> RACES
      LOOP --> MEM
      LOOP --> RACES
      LOOP --> WIRE
      WORLD --> WIRE
      WORLD --> LEDGER
      WORLD --> MEM
      MEM --> LEDGER
    end
    PROTO -. generated from .-> SERVER
    SDK <==>|JSON-RPC| SERVER
    classDef ts fill:#fde7f3,stroke:#db2777,stroke-width:1.5px,color:#500724
    class CLI,BOARD,SDK,PROTO ts
    classDef rs fill:#ffedd5,stroke:#ea580c,stroke-width:1.5px,color:#431407
    class TUI,SERVER,LOOP,WORLD,RACES,WIRE,TOOLS,PROV,LEDGER,MEM,CONFIG rs
    classDef gate fill:#fef3c7,stroke:#d97706,stroke-width:2px,color:#451a03
    class SANDBOX gate
    style ts fill:#fdf2f8,stroke:#db2777
    style rs fill:#fff7ed,stroke:#ea580c
```

## In plain words

All the decisions live in Rust, in small crates that each do one thing and can be tested alone.
The clients only show things and ask you questions: the TypeScript terminal client and the web
board speak to the core over its protocol, and the Rust terminal UI hosts the core in its own
process — three windows onto the same core. The arrows only point downward — `races` knows nothing about
networks, `wire` knows nothing about models — so each piece can be proven before the next is
built on it. `tools/check-layers.mjs` holds the same graph and fails on any edge it does not
draw.

## Technical detail

### The repository

```
mnemorph/
├─ crates/
│  ├─ mnemorph-wire       envelope types, parser, encoder            (no I/O)
│  ├─ mnemorph-races      Race enum, shelves, dispatch, scores, falls (no I/O)
│  ├─ mnemorph-ledger     usage + value ledgers, budgets, prices      (files)
│  ├─ mnemorph-provider   streaming clients, retry ladder, caching,   (network)
│  │                      System One decisions with an LLM fallback
│  ├─ mnemorph-sandbox    landlock + seccomp, Seatbelt, net policy    (OS)
│  ├─ mnemorph-tools      tool impls, classifier, secret injection    (OS)
│  ├─ mnemorph-config     TOML layers, origins, the gate's loosening  (files)
│  ├─ mnemorph-memory     tiers, BM25 recall, the write rule          (files)
│  ├─ mnemorph-world      creatures, onto, dispatch, gate, Minds      (files)
│  ├─ mnemorph-loop       the six beats, journal, resume              (all above)
│  ├─ mnemorph-server     JSON-RPC, WebSocket, board assets           (network: loopback)
│  ├─ mnemorph-tui        the terminal UI, the session in-process     (terminal)
│  └─ mnemorphd           the binary
├─ packages/
│  ├─ protocol           generated from crates via ts-rs; checked in
│  ├─ sdk                transport (stdio | ws), reconnect by seq
│  ├─ cli                `mnemorph` — Ink + React
│  └─ board              Vite + React; built into mnemorphd
├─ docs/                 this book
├─ .mnemorph/             Mnemorph builds itself as a world
└─ justfile              one entry point for every task
```

### Choices, and why

| Concern | Choice | Why |
|---|---|---|
| async runtime | tokio | the ecosystem default; cancellation via `CancellationToken` everywhere |
| HTTP | reqwest + rustls | no OpenSSL; private CA bundles loadable |
| server | axum | WebSocket + static assets + JSON-RPC in one router |
| types → TS | ts-rs | one source of truth for the protocol |
| config | TOML via `toml_edit` | edits preserve comments; isekai's YAML-append bug cannot exist |
| recall | tantivy (BM25) | local, model-free by default (Nature 7) |
| errors | `thiserror` in crates, `anyhow` only in `mnemorphd` | typed failures cross crate borders |
| logs | `tracing` → JSON file under the world | the journal is an instrument |
| tests | nextest, insta snapshots, proptest; vitest + ink-testing-library; Playwright for the board | every rung testable alone |
| TS tooling | Node 22, pnpm workspaces, tsup, Biome | Node is already the platform Claude Code runs on |
| distribution | an npm package with per-platform optional deps carrying `mnemorphd`; plus a curl installer and GitHub release binaries | one `npm i -g` for users, a plain binary for servers. The names `mnemorph` on npm and crates.io are taken: the published names are settled in phase D |

### The terminal UI

`mnemorph-tui` is a terminal UI in Rust, drawn with [Ratatui](https://ratatui.rs). It hosts the
session in its own process: `mnemorph_loop::open` gives it the session, an observer turns what the
run does into the same events the server sends (`mnemorph_server::events`), and the model's stream
arrives as text deltas. No protocol sits between the screen and the core.

```sh
cargo run -p mnemorph-tui -- --world <dir>            # a session in that world
cargo run -p mnemorph-tui -- --theme light            # dark, light or auto
cargo run -p mnemorph-tui -- --probe                  # what the terminal draws, then exit
```

`--theme auto` (the default) reads `MNEMORPH_THEME` (`dark` or `light`), else the terminal's
background as `COLORFGBG` reports it, else dark.

```
╭─ ◈ MNEMORPH · rimuru · deepseek-v4-flash ───────────────────────────────────────────────── ● thinking 0s ╮
│ ᴄᴏɴᴠᴇʀꜱᴀᴛɪᴏɴ ─────────────────────────────────────────────────────────── │ ꜱᴇꜱꜱɪᴏɴ ───────────────────── │
│ ▝▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀ ▘ │ ▾ vitals ──────────────────── │
│                                                                          │ ᴄᴛx  ━━━━────── 41% / 150.0k  │
│ ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄   │ ᴛᴏᴋ  60.9k                    │
│ ▌ ● edit  sqlglot/parser.py  write                                   ▕▄▖ │ ᴄᴏꜱᴛ $0.04                    │
│ ▌   ⎿ ok · 0ms · +1 −1                                               ▕ ▌ │                               │
│ ▌      - return this                                                 ▕ ▌ │ ▾ memory ──────────────────── │
│ ▌    1 + return self._parse_collate(this)                            ▕ ▌ │ recalled 3 this ask           │
│ ▝▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀ ▘ │ grounded 5 · shared 2         │
│                                                                          │ ✓ “collations drop in         │
│   ↳ orc-sql  is working on: judge                                        │   \u{202e}_parse_types\u{1b}… │
│                                                                          │                               │
│   ↳ slime-parser  bash pytest tests/test_parser.py                       │ ▾ files · 1 ───────────────── │
│  ▄▀▀▀▀▄  ⣾ Thinking… (0s · 61k tokens · esc to interrupt)                │ sqlglot/                      │
│ ▀▀▀▀▀▀▀▀   slime-parser · running bash                                   │ └ M parser.py                 │
│ ▀▀▀▀▀▀▀▀                                                                 │                               │
│  ▀▀▀▀▀▀                                                                  │ ▾ bodies · 2 · 2 live ─────── │
│ ╭─ ᴀꜱᴋ ────────────────────────────────────── enter queues for rimuru ─╮ │ ◆ rimuru            ⣾ working │
│ │ ›  a line now waits its turn                                         │ │ … +2 more                     │
│ ╰──────────────────────────────────────────────────────────────────────╯ │                               │
╰─ $0.04 · 60.9k tok · ctx 41% · esc stop · enter queue · pgup scroll · world ~/sqlglot ───────────────────╯
```

| Part | What it shows |
|---|---|
| header | Mnemorph's mark (the name drawn across the water races' blues), the session's body and its model on the left; the session's id and its state on the right — `○ idle`, `● running bash 3s`, `◆ waiting for you` |
| panes | labelled in small capitals on their edges: ᴄᴏɴᴠᴇʀꜱᴀᴛɪᴏɴ, ꜱᴇꜱꜱɪᴏɴ (the sidebar), ᴀꜱᴋ (the input) |
| welcome | the card the conversation opens on: Rimuru's portrait (still), the name and tagline, the races as chips, the world, the model and the session in small capitals, the first keys |
| transcript | each message and call a raised panel — a spine in its colour, a lit top edge, a shadow one cell right and half a row down: the human's lines in gold; the body's Markdown (headings, emphasis, code highlighted, lists, quotes, links, tables) in cyan with `✦`, one panel however many pieces it streamed in; tool calls as cards in their class's colour (a look blue, a write violet, an outward act orange, a destructive one red) — `● bash  pytest -q  read`, then `⎿ running…`, then `⎿ ok · 2.0s · 3 lines` or `⎿ failed · 800ms`; output folded to three lines, a numbered diff for `edit`, `multiedit` and `patch`, a new file's head for `write`, each expanded whole by a click or a key; consecutive reads, greps and globs as one card (`read 4 files · grep 3 patterns`, its calls ok, failed or stopped counted); the reasoning as one quiet line; the end gate's verdict as one line with its counts |
| activity strip | above the input from the frame an ask is sent until its turn ends: a spinner, the phase as a verb in a moving shimmer, the time the turn has run, its tokens; under it the body and the phase in words, and the context when it nears its stress line |
| sidebar | at its head, on a column 36 rows tall or more, the portrait of Veldora — the human — while no body works, and of the body working now during a turn (Rimuru for the session, a commissioned body's race while it acts), over its state; **vitals**: the context as a gauge against the stress line, the tokens, the spend; **memory**: the notes recall gave this ask, the grounded and shared counts and the claim grounded last, as `mnemorph memory status` reads them; **files**: the paths the session's acts changed, marked `A`, `M` or `D`, as a tree of their directories; **bodies**: the session's body and the bodies it commissioned, nested as they commissioned each other, each in its race's colour with what it is doing. A click on a section's title folds it (`▸`) or opens it (`▾`) |
| input | multi-line; a line sent while a turn runs waits its turn, shown above the input |
| approval | a modal over the dimmed view: a strip naming the gate with its countdown, the act's class in its colour, the act, its reasons and the paths it opens outside the world, the answers numbered; the input's edge reads "answer the card first" |
| toast | the end gate's verdict, `◆ end gate pass`, floating in at the top right and fading out on the spring; never drawn over an open approval |
| status bar | the spend in dollars and tokens, the context against its stress line (gold from 75%, alarm from 90%), the keys that matter now, the world |

| Key | Does |
|---|---|
| Enter | send (queued while a turn runs) |
| Alt+Enter, Shift+Enter, Ctrl+J | break the line |
| ↑ ↓ | move between lines, then through the lines sent before |
| PgUp PgDn, Shift+↑ ↓, Ctrl+Home End, the wheel | scroll the transcript |
| Alt+↑ ↓ | move the focus between cards (calls, groups, thoughts), scrolling each into view |
| Ctrl+T, a click on a card | expand the focused (or clicked) card — a call's folded output, a group's calls, a thought's text — or fold it again; with none focused, the newest |
| Ctrl+O | show or hide the sidebar |
| Esc | stop the turn; back to the end of the transcript |
| Ctrl+C | stop the turn, or leave when none runs |
| 1 or y, 2 or n, 3, then Enter | answer an approval — yes, no, or no with a reason typed for the model; ↑ ↓ move the pick while the whole act shows — from no pick to No, stopping at the ends, never onto Yes unasked — and read a long act otherwise; Esc denies it |

Slash commands: `/send <text>` steers the running turn at its next tool step, `/theme dark|light`,
`/clear`, `/help`, `/quit`.

**It shows its work as it happens.** The status appears in the frame after Enter, before any
token: the phase is read from the model — an approval on the card, a call whose arguments still
stream (`calling grep`), a call running (`running bash: pytest -q`), the answer streaming
(`writing`), the context past its stress line with the next answer not begun (`compacting`), else
`thinking` — and a commissioned body that acts (its events come through `Seen::From`) puts its
own name and tool there, and its portrait at the sidebar's head, until its turn ends: the body
shown is the newest that still works, so siblings working at once never take the portrait from
each other, and the session's own shows only when none does; the loop restarts only when the
body shown changes. The providers hand the stream reasoning as it comes
and a call's arguments piece by piece (`StreamEvent::Thinking`, `StreamEvent::ToolInput`; the
server's protocol does not carry them): the reasoning shows as one line that updates in place —
`◌ thinking · 12s · …its latest words` — and folds to `✓ thought for 12s` when the answer or a
call begins, its text a key away. A provider that keeps its reasoning in opaque blocks shows it
when the turn's blocks are journaled: the thought is placed where the step began, timed to the
step's first output; it replaces whatever streamed, so a retry that streamed the reasoning again
never leaves it doubled. A call's card appears at its first byte, its summary filling in as its
arguments arrive, its class read by the classifier off the UI thread; its duration is the time
to its result. Time is the model's clock, advanced by its ticks.

The portraits are each race's idle loop, 32 pixels square (32 columns by 16 rows, 48 frames), in
the `.mnanim.json` format of Kaginari/mnemorph-animations, embedded from `assets/anims/` (its
README names the commit and command they came from) and decoded once per race into every
frame's cells by `kit::mnanim`, on a blocking thread at start — a frame only reads the cache and
never decodes, so a race not yet ready is simply not drawn — — the half-block rule of that repository's player; a
malformed file is an error, never a panic. The portrait plays at the loop's own pace, 16 frames a second, advanced by elapsed time under the 30-frame
cap, only while a body works and the portrait is on screen; idle, it rests on its first frame and
the UI schedules no tick for it.

**The portraits in real pixels.** Where the terminal draws images, the portraits are pictures as
sharp as their source — each race's 256 px PNG, embedded from `assets/portraits/` (mnemorph-animations
`df20cfa`) — and the half blocks above are the fallback. At start, before the alternate screen,
`probe` asks the terminal on `/dev/tty` in raw mode: the window's pixels (`TIOCGWINSZ`), a cell's
size (`CSI 16 t`), its name (XTVERSION), Kitty's graphics query, and the device attributes (DA1)
last — every terminal answers DA1, so its reply ends the wait. It waits at most 200 ms with
`poll` on the calling thread, leaves no thread reading the input, and flushes a half-reply when
the time runs out. A reply that comes later still reaches the UI's input as keys — `ESC P` and
`ESC _` read as Alt+P and Alt+_, the rest as characters, Kitty's `1` among them — so for two
seconds after a probe that timed out, keys shaped like a reply — plain characters, and Alt+P,
Alt+_, Alt+[ and Alt+\ — are dropped, and no other key reaches an open approval card. A key with
Control is never a reply: Ctrl+C and Esc always reach the UI, so they stop, quit and deny. `kit::pixels` reads the replies and chooses:

1. **Kitty** when its query answered OK;
2. **iTerm2's inline images** when the terminal is xterm.js (VS Code), iTerm2 or WezTerm, by its
   name or the environment, and images are on — DA1 lists 4, or iTerm2 or WezTerm, which always
   draw them. A PNG in full colour, smaller than Sixel, so it is preferred where both exist;
3. **Sixel** when DA1 lists 4;
4. **half blocks** otherwise — and whenever a cell's size in pixels is unknown, or inside tmux.

`MNEMORPH_TUI_IMAGES` overrides it: `auto` (the default), `kitty`, `iterm`, `sixel`, `blocks`, or
`off` — half blocks, nothing asked of the terminal. `mnemorph-tui --probe` prints what was found,
the replies as they came, and the choice.

A picture is built once per race, box size and background: the PNG laid on the background at
its own size, then Lanczos-resized to a square exactly `cols × cell width` by `rows × cell
height` pixels — 32 columns of 8 × 17 px cells are 256 × 255 px in 15 rows — centred, so the
terminal never rescales it; its encoding is cached. iTerm2's inline images are given their box
in cells, so a font zoom never leaves the picture spilling past its box; on a resize the cell's
size is read again from the window (`TIOCGWINSZ`), and when it changed every picture is built
again at the new size. Sixel draws exactly the pixels it was built for: where the window gives no
pixels, a resize leaves no size to trust, so the portraits fall back to half blocks — the old
picture's box written again — until a size returns. Every byte of an image sequence comes from
those embedded files and from numbers, never from the model or the world.

A terminal image lies over the cells, and text drawn after it does not cover it, so the view
draws plain cells of the picture's background where it goes and names the picture it wants
(`pictures::Want`: the slot, the race, the box, the background). After each frame `Placer`
compares that with what the terminal shows. An unchanged picture is never sent again; one gone
or moved is erased first — Kitty deletes it by id, and under Sixel and iTerm2 the cells under its
old box are written again from the frame just drawn — then the new one is sent at its box with
the cursor saved and restored. While an approval is open no picture is wanted, so the card never
sits under one; under Kitty they are placed below the text (`z=-1`), so text drawn over one
always shows, and they are deleted before the frame that opens the card besides. A panic deletes
Kitty's pictures too, before the terminal is given back. A toast or the scroll note never sits under one either. A resize redraws
the screen, so each picture is sent again; the sidebar's toggle, its fold below 100 columns and
a scroll moves or removes theirs. A box that moved is erased and its picture held
back until the box has stayed put for 150 ms, so a scroll never sends a picture once a frame. In pixels a portrait is still — Veldora at rest, the working body's
race during a turn, swapped when it changes — and asks no ticks. Pictures are the sidebar's alone:
the welcome card draws its portrait in half blocks, so it scrolls with nothing to withdraw.
(A Kitty-only animation would upload each frame once under its own id and switch placements by
id; `pictures::Art` is where it would be named.)

A terminal that gives no cell size gives its text area in pixels (`CSI 14 t`) more often than
not; the cell is that area over the window's cells, in either order (VTE answers width first):
the one the window divides exactly, a text area being whole cells, then the one nearest twice as
tall as wide. VTE lists Sixel in its device attributes when its Sixel is on; a VTE that does not
draws half blocks, and `--probe` names its version (`VTE_VERSION`) and what would give images.

To see the pictures in VS Code's terminal, turn on `terminal.integrated.enableImages` in the
settings and reload the window: xterm.js then lists Sixel in DA1, and Mnemorph draws iTerm2's
inline images. `mnemorph-tui --probe` says `drawing iterm` when it took.

The sidebar takes 36 columns on the right from a 100-column terminal up, and folds away below it.
It shows only what the run or the world gives. The observer sends the chain of bodies each
commissioned body's event came through (`Seen::From`), what recall gave the session's ask, and
each `write`'s path; it never touches the disk on the session's task. The UI looks whether that
path exists on a runtime thread, so a hung mount holds that thread, never the session. The look
may race the write: "not there" proves the write created the file and marks it `A`, whenever the
answer arrives; anything else leaves it `M`. An act's paths count once its result lands without
error. The memory's standing (`mnemorph_memory::standing`, the same reading `memory status`
prints) and the roster's races are read on a runtime thread at start and after each line
finishes, each reading numbered so a late one never overwrites a newer one. The bodies' tree is
indexed as events fold in, so a frame walks only the rows it shows. The UI takes the mouse for the wheel and clicks; hold Shift to select text as usual.

**Nothing a model or a tool writes reaches the terminal raw.** The transcript drops the colour
sequences a program prints and escapes every other control, bidi and zero-width character as
`\u{…}` (`mnemorph_loop::hidden`); so does the sidebar, for every claim, path and body name it
shows. The approval card shows each field through `mnemorph_loop::shown`, the core's own rule
for what a human approves, and takes an answer only from a key: a pick (`1` or `y`, `2` or `n`,
`3`, or ↑ ↓ while the whole act shows), then a pause, then Enter. Every pick passes the same
rules. Keys are ignored for 300 ms after the card is first drawn — a key that came before
the first draw never arms it — and so are keys the terminal read before it; any key that is not a
pick, Enter or a scroll — a paste too — clears the choice and arms the card again, so `okay` or
`deploy` typed into it, or a `y` and a carriage return in one read, approve nothing. An act longer
than the card scrolls inside it (↓ ↑, PgDn, End): its head shows, the cut marked, with the end of
its last line, and yes is refused until every row has been on screen, read in order — a jump past
rows never shown does not count them. The third answer denies, and once confirmed takes a reason
typed for the model; Enter sends the denial with it. Acts that ask while a card is open wait their turn, their number on the card. The
card counts down to its deadline and leaves at it. Esc, Ctrl+C, the turn ending, SIGTERM, SIGHUP,
SIGINT or SIGQUIT, and ten minutes of silence deny.

A panic in the session's task ends its turn as a failure and frees the UI; SIGTERM, SIGHUP,
SIGINT and SIGQUIT leave as a quit does, the terminal given back. However the loop ends, every
approval open or waiting is denied, and a terminal mode that cannot be set at start gives the
terminal back before the error prints. A panic on the UI thread turns the mouse, bracketed paste
and the keyboard flags off before its message prints.

**It stays fast in a long session.** Each entry is laid out once per width and kept — what moves
(a spinner, an elapsed count) once per frame and second — and a frame lays out only the entries
it shows, counted up from the end. Only the tail paragraph of a streaming answer reflows: a
finished paragraph leaves for an entry of its own, joined to the same panel. Stream deltas and tool events that arrive
together become one frame, and frames come at most 30 a second. An idle UI wakes for nothing.
The wheel moves the scroll on the same spring as the keys, and the loop ticks only while it moves.
A test draws a 2,000-message session with the sidebar open — every file it changed, forty
bodies, the activity strip drawn and the portrait playing — and holds every frame under the 33 ms budget in a debug build.

The UI is built in the Elm architecture, on a kit that rebuilds the good parts of Charm's
ecosystem:

| Charm | `mnemorph-tui` | What it gives |
|---|---|---|
| Bubble Tea | `app` | one model, a message enum, a pure `update` returning commands the runtime runs off the UI thread, a `view` |
| Lip Gloss | `kit::style`, `kit::text` | a chainable box style — padding, borders with titles set in, width, alignment, margins — blocks joined across and down, all measured in terminal cells |
| Bubbles | `kit::textarea`, `kit::viewport`, `kit::spinner`, `kit::keyhelp` | components, each with its own `update` and `view` |
| Harmonica | `kit::spring`, `kit::toast` | a critically damped spring: the transcript scrolls smoothly, a toast fades |
| Glamour | `kit::markdown`, `kit::highlight` | Markdown drawn from a stylesheet the theme derives; code highlighted by syntect's grammars, in colours taken from the palette |
| Isekai's depth | `kit::slab`, `kit::shimmer` | raised panels — a spine, a lit top edge, a shaded bevel, a cast shadow, every tone stepped from the palette — and a crest of light sweeping a busy word |
| mnemorph-animations | `kit::mnanim`, `kit::pixels` | the races' portraits: their idle loops decoded once into cells and drawn in half blocks, or their 256 px sources as real pixels — Kitty, iTerm2 or Sixel — resized once to their box |

The colours are `packages/theme`'s, ported to `theme.rs`; a test reads the TypeScript file and
fails when the two differ.

### Storage: files are the truth

| What | Where | Tracked in git |
|---|---|---|
| law, creature docs, Minds | `.mnemorph/law.md`, `.mnemorph/<race>/<name>/README.md`, `.mnemorph/minds/` | yes |
| journal | `.mnemorph/log.md` — append-only writer | yes |
| shared notes | `.mnemorph/memory/shared/notes.jsonl` — append-only note events ([memory](08-memory.md)) | yes |
| personal notes | `.mnemorph/memory/creature/<name>.jsonl` — one creature's note events | yes |
| semantic cache | `.mnemorph/memory/short/` — one body's recent recalls | no — machine-local |
| shared work queue | `.mnemorph/shared/queue.jsonl` — append-only snapshots; its witness under `$XDG_STATE_HOME/mnemorph/queue/` | no — machine-local |
| Kijin post journals | `.mnemorph/kijin/<post>/post.jsonl` | yes |
| Dark Elf proving ground | `.mnemorph/dark-elf/` | yes (frozen witnesses are evidence) |
| plan | `.mnemorph/plan.md` — the throne's | yes |
| dimension manifest | `.mnemorph/dimension.toml` — the neighbouring worlds and their relations ([ontology](09-ontology.md#relations-between-worlds)) | yes |
| ontology facts | `.mnemorph/ontology/facts.jsonl` — territory facts, counters and settles, as events | yes |
| relation sightings | `.mnemorph/ontology/sightings.jsonl` — cross-world references seen, the ledger relations are learned from | yes |
| advisory mirror | `.mnemorph/intel/` — kept by `kijin-scout` | no — hundreds of MB |
| attic | `$XDG_STATE_HOME/mnemorph/attic/<world>/` (else `~/.local/state/…`) — outside the world, out of every command's reach: a git object store of working-tree snapshots, hard links to `.git`'s objects, copies of its refs | no |
| usage + value ledgers | `.mnemorph/instruments/*.jsonl` | no — machine-local |
| derived indexes | `.mnemorph/index/` | no — rebuildable |
| run journals | `.mnemorph/instruments/loop/` | no |

Every index is derived and can be deleted; `mnemorph index` rebuilds it. A stale index is rebuilt,
never routed around.

### Sandbox

```mermaid
---
config:
  look: handDrawn
  theme: neutral
  flowchart:
    wrappingWidth: 360
---
flowchart LR
    T["tool call"] --> P{"policy for<br/>this race + soil"}
    P --> FS["landlock: read system + world,<br/>write only the world"]
    P --> NET{"network?"}
    NET -- "approved outward act" --> ALLOW["sockets open"]
    NET -- "anything else" --> DENY["seccomp: no socket()"]
    P --> SC["seccomp: no ptrace, mount,<br/>unshare, bpf, io_uring, setsid"]
    P --> ENV["env: secrets by name,<br/>outputs redacted"]
    classDef step fill:#e0f2fe,stroke:#0284c7,stroke-width:1.5px,color:#082f49
    class T,P,NET step
    classDef gate fill:#fef3c7,stroke:#d97706,stroke-width:2px,color:#451a03
    class FS,SC,ENV gate
    classDef kijin fill:#fbdde1,stroke:#D7263D,stroke-width:2px,color:#3d0710
    class ALLOW kijin
    classDef bad fill:#fee2e2,stroke:#dc2626,stroke-width:1.5px,color:#450a0a
    class DENY bad
```

- **A kernel guard, not a jail.** A jail gives a command another world — its own filesystem
  view, users and network. The guard does not: the command runs as the human, in the real
  files, with the real tools. It only has the kernel enforce a permission list on that one
  process — which folders it may read, that it may write only the world, whether it may open a
  socket. On Linux every command starts through `mnemorph-guard`: the helper sets the list on
  its own process and then *becomes* the command (`exec`), so it never stays in between and
  everything the command starts inherits the list. It runs once per command because the
  kernel's restrictions are one-way — a process can tighten, never loosen — so each command,
  with its own list, needs a fresh process; it costs about a millisecond. No unsafe code and no
  namespaces are needed, so it works where unprivileged user namespaces are restricted (Ubuntu
  23.10+ through AppArmor).
- **No guard, no command.** When no guard is in force — the helper missing, an unsupported
  system — `bash` refuses to run and says why. The human may choose otherwise with
  `sandbox.allow_unguarded = true` in their local or global file (never the tracked one); the
  status line then reads `none — <reason>`.
  - **landlock** — reads: the system's directories, the world, and the toolchains in the home
    (`.rustup`, `.local/bin`, uv's interpreters and tools, …); never a credential path
    (`.ssh`, `.aws`, `.gnupg`, keyrings, uv's `credentials/`, …), which no rule names, nor a
    folder above one: landlock cannot hide a path inside a tree it reads. So a toolchain tree
    that holds a credential is granted part by part: of `.cargo`, `bin`, `registry`, `git` and
    its config (not `credentials.toml`); of `.cache`, the named caches of uv, pip, pre-commit,
    Go, Node, pnpm, TypeScript, Playwright, Hugging Face's models, the bench's interpreters and a
    few more (not the Hugging Face token or git's credential cache); of `.config/git`, its
    `config`, `ignore` and `attributes` (not `credentials`); `.m2/repository`, `.gradle/caches`
    and `.gradle/wrapper` (not `settings.xml` or `gradle.properties`); and each Node nvm
    installed, its `bin`, `lib` and `include` (not `etc/npmrc`). A cache not named is
    unreadable. A toolchain path that is a link resolving onto, inside or above a credential
    path grants nothing; Landlock resolves links again when a file is opened, so a link inside a
    granted cache leads nowhere either. Of `~/go`, only `bin` and `pkg` (not `src`, the human's
    projects). Some configs stay readable because their tools cannot run without them, and each
    can hold a secret: `.cargo/config.toml` (a `[registry] token`), the remote URL in each cargo
    git dependency's `.cargo/git/db/*/config`, and `.gitconfig` (an `http.extraHeader` bearer
    token) — keep such tokens in the credential files instead. Seatbelt reads the same list and denies the credential paths besides. Writes: the world, the command's temporary directory
    inside it, and a few devices — for solo. A commissioned creature's commands are granted
    exactly what its soil admits, and the rest of the world is read-only to them: landlock can
    allow a folder but never except a spot inside it, so a claim that holds another's spot (a
    seat, the queue, another's scratch) is granted part by part around that spot, existing
    entries each on its own. A claim reached through a link is no claim. Build output is granted
    besides the soil: what git ignores where `own` names the creature or nobody — never another's
    ignored files, the records (`.mnemorph/`) or git's own files, and never through a link. The
    guard resolves every rule to the file it names, so every grant is resolved first and kept
    only if it still resolves inside the world to the creature's own. Past a few hundred rules
    they reach the guard in an owner-only file it reads and removes before restricting itself.
    The temporary directory is the creature's own scratch nest, or, when that nest is a link, a
    fresh owner-only directory outside the world for that creature alone. Below landlock ABI 3,
    which cannot refuse `truncate` on a path, seccomp refuses the call.

    What this costs a commissioned creature, by design: in a folder that holds another's spot it
    cannot create, delete or rename its own files, so `sed -i`, editors' atomic saves and
    formatters that replace a file fail there — the file tools do that work; a build
    whose output directory or root lockfile does not exist yet cannot create it; `.git` is not
    its to write (landing is the harness's); a claimed folder that does not exist yet gets no
    grant until it is made. Landlock never governs a file's mode or times, so the end gate's
    snapshot records each file's mode and inode change time, which no process can set back:
    a `chmod`, a `touch` or a rewrite dressed in its old mtime counts as a change and is judged.
    The file tools run in the harness, not under landlock, each path checked against the soil
    and opened component by component without following a link: `read`, `write` (whole, atomic),
    `edit` (one exact replacement), `multiedit` (several, in order, all or none, and never a
    rewrite that leaves the file as it was), `move` (a file or a directory, both ends and every
    entry in it the body's own, never a folder holding another's spot, never into itself, never
    over an existing file) and `delete` (a file, only once the attic's snapshot holds a copy of it
    — so not what the attic leaves out: ignored files, build output, large files — or an empty
    directory), and `patch` (a unified diff across files: every file's result computed first, a
    hunk landing at its stated line or its one match elsewhere, nothing written when one does not
    apply, a file named twice taking each part in turn, a failed write putting back every file
    already changed; `/dev/null` creates or deletes, a delete under `delete`'s rule; line endings
    kept; a rename refused, for `move`; a binary, mode-only or empty-file `diff --git` block
    refused by name, never skipped). A link is moved or deleted as itself, never its target; git's own files are never
    a tool's; a refusal leaves the tree as it was, the directories a failed move made unmade; a
    write of what a file already holds writes nothing; and `edit` and `multiedit` refuse a file
    that is not UTF-8 text rather than replace its bytes.
    `sed -i` and `perl -i` are refused, pointing to `edit` and `multiedit`: an anchor that misses
    exits 0 and changes nothing.
    Layered, landlock refuses what it can, the tools refuse by name, and the end gate's *right
    author* judges every changed file the snapshot sees; what git ignores is build output and is
    not judged. Landlock also keeps a sandboxed process out of `/proc` of processes outside its
    domain, so the harness's environment is closed to its commands; the harness additionally
    makes itself non-dumpable.
  - **seccomp** — without network, `socket()` fails for every family, Unix sockets included (a
    host daemon's socket is a way out); `socketpair` stays. Always denied: `ptrace`,
    `process_vm_*`, `mount`, `unshare`, `setns`, `pivot_root`, `chroot`, `bpf`,
    `perf_event_open`, the kernel keyring, module loading, `io_uring` (it would bypass the
    socket rule).
  - **The network** opens only for an outward or destructive command the gate approved. An
    approval also opens the paths outside the world that the classifier located in the command
    — listed on the approval card — and nothing else: a path known only at run time stays shut.
    Each path opens alone, never its folder: a new file is first made, empty, by the harness so
    the sandbox can name it (and removed again if the command fails). An approved delete
    outside the world stays refused: the kernel ties deletion to the folder, and the folder is
    not opened.
  - **Limits** — no core dumps, files up to 4 GiB, at most 8192 processes for the user; and no
    new namespaces: `unshare` and a `clone` carrying a namespace flag fail, `clone3` reports
    itself missing so the C library falls back to `clone`.
- **The tripwire** covers what landlock cannot: landlock only adds access, so no read-only corner
  can be carved out of the writable world. Git's executable configuration (`.git/config` —
  `core.fsmonitor`, `core.sshCommand`, filter drivers — `.git/hooks`, `.git/info`, a `.git` file)
  would run code outside any sandbox the next time the human runs git, and `.mnemorph/` holds the
  world's records. Each command runs between two snapshots of them; once its process group is dead,
  every change is put back and the body is told. The file tools refuse those paths outright, but
  one: a body's own `plan.md`, directly in its own seat, which the dispatch law reads (an Orc's
  verify lines on its Slimes' docs are written by the harness, through the `verify` tool) — written
  with the file tools; a command's write there is put back, and a plan that is a link is no plan. On
  Linux no process escapes the snapshot: seccomp refuses `setsid` and `setpgid`, so killing the
  group ends everything the command started. A tool that insists on its own session or group
  (Python's `start_new_session`, Go's `Setsid`, a pty) fails there with `EPERM`, and coreutils
  `timeout` signals the whole command when it fires. Seatbelt has no such rule on macOS. The shared
  work queue is rebuilt from what it held and what its witness gained since — the witness, under the
  human's state directory, keeps every line the harness and the human's `mnemorph q` wrote — so
  nothing the command did to the file counts, a link in its place included, and the human's
  concurrent moves are kept. A body's own `mnemorph q` refuses (every command carries
  `MNEMORPH_BODY`, a courtesy; the tripwire is the border). `tmp/`, `intel/` and `report/` (where a check writes what it leaves) stay writable; a
  repository the command itself creates is its own. The snapshot keeps up to 16 MiB of content; a
  change to a protected file beyond that is reported, not undone.
- **The attic** keeps what a command or a write takes away, outside the world where the guard
  lets no command reach. Before every command and every file write, the working tree is
  snapshotted into the attic's own git object store — only changed files are hashed again.
  What the world ignores is left out, and so, by the attic's own rules whatever the world
  ignores, are build output and dependencies (`target`, `node_modules`, `.venv`, `dist`,
  `build`, …), the command's `.mnemorph/tmp` and the machine-local records (`instruments`,
  `index`, `intel`); a file that becomes ignored leaves the next snapshot. A file over 8 MiB is
  not kept, and the command's result names it. A command that deleted files is told how many
  and which snapshot holds them, so `rm -rf src` and a script that wipes a folder are both
  undone by restoring from it. The world's `.git` is kept another way: its objects are
  hard-linked into the attic (copied, across devices) and its refs copied, so a command that
  deletes or breaks `.git` finds it put back after it ends. A hard link shares the file, so it
  survives a deletion, not a write in place: deleting or changing `.git` or the world's records
  outright is `destructive`, so it asks first — and what the human approved, the attic keeps but
  does not undo. A world whose `.git` is a file — a worktree's, a submodule's — has its
  repository elsewhere: that file is kept, and put back when a command deletes it. A nested
  repository is kept as git keeps one, by its commit's name and none of its files, so the attic
  never says it holds one: a delete of it is told it was not kept. Two runs on one world take the attic in
  turn — a lock around every snapshot and restore — never colliding on its index. The attic's
  git runs deaf to every configuration but its own: no hooks, no filters, no attributes, no
  fsmonitor from the world or the human's files.
  It prunes itself at most once a day: snapshots older than `attic.keep_days` (14) go, then the
  oldest while the snapshot store is over `attic.max_mb` (2048) — the newest always stays — and
  `git gc` packs what remains; a kept `.git` object ages out with the same window only once the
  world itself has dropped it. The cap measures the snapshot store only: the kept `.git` objects are hard links
  and cost no disk beside the world's own. Measured on this repository: the first snapshot
  takes 1.8 s and about 13 MB (9 MB once packed); an unchanged one, 24 ms and nothing.
- **The file tools** run in the harness, not the sandbox, so they guard their own opens: a path
  is resolved and checked inside the world (and the soil, for writes), then opened again one
  component at a time from the world's root following no symlink, and written through a
  temporary sibling renamed inside the same directory handle. A directory swapped for a symlink
  between the check and the open fails the open instead of leading out.
- **The guard's rules have names** (`in-place-edit`, `secret-store`, `credential`,
  `not-literal`, `remote-code`, `unreadable`, …). The human's configuration may turn a rule into
  a question — the command then waits on the gate with the class it would have had — except
  the floor, which stays a refusal whatever any file says: `root-path`, `device`,
  `machine-tool`, `fork-bomb`. A floor refusal displaces any askable one found earlier in the
  same command, so no co-occurring rule can turn it into a question. The sandbox still holds under an approved rule: approving
  `credential` does not open a credential path.
- Landlock's power depends on the kernel's ABI: v1 (5.13–5.18, this machine's 5.15) refuses
  every cross-directory rename or link inside the sandbox (`mv` and git fall back to copying)
  and cannot restrict truncate; v4 (6.7+) adds network rules. The guard asks for the newest rules
  it knows, the kernel enforces what it has, and `status` prints what is actually in force —
  `landlock ABI 1 (of 5 asked) + seccomp, network off`, or `none — <reason>`.
- macOS: a generated Seatbelt profile with the same policies — reads from the same allowlist
  (metadata anywhere, for path lookups), writes only the world, and only the system services
  commands need: not LaunchServices, through which `open -a` would start an application outside
  the sandbox.
- `--container`: the container **is** the sandbox — one container per body, its mounts limited
  to the body's soil and its network to the policy. It does not nest landlock or namespaces
  inside Docker's default profile.
- The sandbox is applied by the core when spawning; a tool cannot opt out.
- The escape suite runs in CI on both a 5.15 kernel (ABI v1) and the current runner's kernel.

### Configuration

Four layers, lowest first; a higher one wins a scalar or a list, tables merge key by key, and
an empty list or table never overrides a lower layer:

| Layer | File | Who writes it |
|---|---|---|
| default | compiled in | the release |
| global | `$XDG_CONFIG_HOME/mnemorph/config.toml` (else `~/.config/…`, home from the platform) | the human |
| tracked | `<world>/.mnemorph/config.toml` | the project, for everyone who clones it |
| local | `<world>/.mnemorph/config.local.toml` | the human, on this machine |

- **Every leaf keeps its origin** — layer and exact file — so `config explain` is right for a
  scalar, a list and each leaf of a merged table.
- **The tracked file only tightens the gate.** `gate.enabled`, `gate.ask_rules`, `gate.approve`,
  `sandbox.allow_unguarded`, `oni.envelopes` and `oni.envelope_days` loosen it; `models.provider`
  decides where every model's requests and key go; and `decide.enabled`, `decide.via`,
  `decide.cutoff.*` and `decide.route` loosen decisions (where their state goes, how sure they must be, which model does the work and what it
  costs), so they are honoured only from the local or global file; from
  the tracked file they become holes saying so. So are `providers.*`: a provider decides where
  a key is sent, and a cloned repository must not point one elsewhere. Nothing else feeds them: no environment
  variable, no flag, and no body — the tripwire keeps every body out of `.mnemorph/`.
  `gate.verify_secs`, how long each of the end gate's verify commands may run, is the project's
  to set: a longer limit loosens no guard, and a command killed at it grounds nothing.
- **Holes and errors.** An unknown key is a hole, reported with its file, and the session goes
  on. A file that does not parse, a value of the wrong type, a floor rule in `ask_rules`, an
  envelope that is a bare `*` or admits a destructive or refused act, an envelope without the
  day it was signed, a fallback cycle, a `models.office` race or office that does not exist or an
  office named twice, a `models.tier` that is not a tier, a `gate.verify_secs` outside 10 to
  600: errors that name the file. An envelope is judged at its worst: a word that is a bare `*` stands for
  any arguments, so it is tried as each destructive one (`--force`, `--delete`, `+ref`,
  `delete`, `publish`, …); a `*` inside a word (`feat/*`) cannot become a flag.
- **Envelopes are bounded.** Each names the day it was `signed` (a bare TOML date) and lasts
  `oni.envelope_days` — 30 unless the human's own file says, from 1 to 365 — unless its own
  `expires` is sooner; never on or before the signing, never past that limit. Raising the limit
  loosens the gate, so the tracked file cannot set it. A revoked envelope covers nothing, so a
  limit lowered after it was signed does not judge it; a live one past the new limit is an error
  that says to sign it again, revoke it or delete its record. Days are UTC calendar days. A list
  of envelopes in the local file replaces the global one whole, as every list does. `uses` caps the acts an envelope covers in
  all, at least one; no two envelopes share a name. `Envelope::admits(today,
  used)` is the check the gate makes at the moment of use, once it honours envelopes (M11), with
  the ledger's count of acts the envelope already covered; it names the lapse that stops one:
  revoked, not yet in force, expired, or spent. Revoking writes `revoked = <day>` into the
  human's file that holds the envelope, through `toml_edit` like any write, and only once: the
  envelope stays on record and covers nothing again. An envelope signed ahead may be revoked
  before its signing day. The dates are the ledger's `Day`, the one calendar in the
  workspace, so config reads the ledger crate.
- **Writes** go through `toml_edit` — comments and layout kept — and the result is parsed again
  before it replaces the file; a failed save is returned, and the file stays as it was.
- **Models**: `models.creature.<name>`, else `models.office.<race>.<office>`, else
  `models.race.<race>` (the race's model at its default tier), else `models.tier.<the race's
  tier>` — and `models.default` is one model for everything: every tier the human's own files did
  not set rides it, so a cloned repository's tiers do not override it.
  `models.fallback.<model>` names the next model to try when a model is not found. The tracked
  file may name models — a team picks the models its world uses — but neither a provider nor a
  model's backend, so a model it names resolves only through the backend the human chose, at the
  human's prices; whether a body is routed, and how sure a decision must be, stay the human's.
- **Backends**: every model goes through `models.provider` (`anthropic` unless the human says),
  any provider that holds conversations: Anthropic's API, or an OpenAI-compatible gateway with
  its key — OpenRouter, a vLLM gateway, a LiteLLM proxy. A model written `<provider>:<id>`
  (`vllm:qwen3-coder`) goes to that provider instead; a `:` that names no provider is part of the
  id (`qwen3:32b`); an empty id is no model. From the tracked file, a model written
  `<provider>:<id>` is a hole, as `models.provider` is. `Config::backend(model)` returns the
  provider, its URL and key variable, and the id it knows the model by; it is honoured once a
  binary builds runs from the configuration (the CLI, M6). **Providers** name the environment
  variable holding their key (`key_env`), never the key. A provider's `kind` is `anthropic`,
  `openai` (any OpenAI-compatible server) or `systemone` (a classifier: TypeSafe's Jev, or a
  jeff server on your own machine).
- **Decisions** (`decide.*`) are a feature the human switches on and off (`decide.enabled`, off by
  default), and only a classifier makes them: `decide.via` names a provider of
  `kind = "systemone"` — TypeSafe's Jev, a jeff server, or any server that speaks the same
  protocol. With none named the feature stays off, whatever else the section says, and the load
  reports why. When the classifier fails, the LLM `decide.llm` (the fast tier's model by default)
  answers in its place. `decide.bench` (on with decisions) and `decide.route` (off until asked)
  switch each point. A decision
  is only acted on above its cut-off, and the cut-off depends on who answered:
  `decide.cutoff.measured` for a System One answer, `decide.cutoff.stated`, stricter, for an
  LLM's, each from 0 to 1. `decide.via` must name a provider of `kind = "systemone"` and come
  with `decide.model`, the name that provider answers to (`jev-latest` at TypeSafe, `jeff-latest`
  at a jeff server). Only the human's own files set `decide.via` or a cut-off: a cloned repository
  can neither send its state to one of the human's providers nor lower the bar (see *System One
  decisions* below).

```toml
# ~/.config/mnemorph/config.toml
[gate]
ask_rules = ["in-place-edit"]      # sed -i asks instead of refusing

[budget]
day_usd = 5

[models.fallback]
"claude-opus-5-5" = "claude-sonnet-5-5"

[providers.openrouter]              # an OpenAI-compatible gateway with its key
kind = "openai"
base_url = "https://openrouter.ai/api"
key_env = "OPENROUTER_API_KEY"

[providers.vllm]                    # or your own vLLM gateway
kind = "openai"
base_url = "https://gateway.example/v1"
key_env = "GATEWAY_API_KEY"

[models]
provider = "openrouter"            # every model goes here unless it names its own
default = "anthropic/claude-sonnet-5.5"   # one model for everything…

[models.race]
dark-elf = "anthropic/claude-opus-5.5"    # …or one per race

[models.office.orc]
judge = "vllm:qwen3-coder"         # …or per race and office, through another backend

[providers.jeff]                   # System One on this machine; or TypeSafe's Jev:
kind = "systemone"                 # base_url = "https://api.typesafe.ai"
base_url = "http://127.0.0.1:8765" # key_env = "TYPESAFE_API_KEY"

[decide]
enabled = true                     # the switch; off by default
via = "jeff"                       # the classifier; without one, decisions stay off
model = "jeff-latest"              # required with via; Jev's is "jev-latest"
llm = "claude-haiku-5-5"           # answers when the classifier fails; the fast tier's model by default
bench = true                       # read a piped test run's output (on with decisions)
route = true                       # Metis: route commissions within their office's band (off until asked)

[decide.cutoff]
measured = 0.6                     # the least confidence to act on a System One answer
stated = 0.9                       # and on an LLM's

[oni]
envelope_days = 30                 # the longest an envelope lasts (1–365); 30 when unset

[[oni.envelopes]]
name = "feature branches"
allow = ["git push origin feat/*", "gh run rerun *"]
signed = 2026-10-24                # expires 2026-11-23, unless `expires` says sooner
uses = 50                          # optional: at most 50 acts in all
```

### System One decisions

Some of Mnemorph's questions are decisions, not conversations: did this piped test run pass, which
body suits this commission, is this finding worth a reviewer's time, how big is this task. Each
has a fixed set of answers, and each is worth acting on only when the answer is sure. A System
One model answers exactly that shape: one call, a probability for every option, in milliseconds.

```mermaid
---
config:
  look: handDrawn
  theme: neutral
---
flowchart LR
    Q["a decision point<br/>state + typed questions"] --> V{"decide.enabled<br/>and a classifier?"}
    V -- yes --> S["the classifier<br/>Jev · jeff · any System One server"]
    V -- no --> OFF["no decision:<br/>the point keeps its default"]
    S -- fails --> L["the LLM decide.llm<br/>answer tool, schema-bound"]
    S --> M["basis: measured"]
    L --> T["basis: stated"]
    M & T --> D["Decision<br/>answers · source · basis · fell_back"]
    D --> C{"confidence ≥ cut-off<br/>for its basis?"}
    C -- yes --> A["the caller acts"]
    C -- no --> K["the caller keeps its default"]
    D --> R["decision row<br/>journaled"]
```

- **One wire for every System One provider.** TypeSafe's Jev and the open jeff share it:
  `POST /v1/systemone` with a state and named questions, each a yes-or-no (`noul`), a `choice`
  between labelled options, or a `score` on an ordered scale of two to ten levels. Options are
  sent in the caller's order, since a model may read them by position. An answer is read
  strictly: exactly the asked labels, summing to 1. A server always returns the whole
  distribution, so any other shape is a fault, refused rather than repaired into something that
  would pass for measured. `SystemOne` keeps the
  provider ladder (the same statuses, waits and `retry-after`, and 529 as busy) on a decision's
  scale: seconds, not minutes, so a fallback can still answer in time. A decision's state holds
  at most 24 000 bytes: a point sends the part that decides (a log's tail, a commission's head),
  and a larger one is refused before anything is sent — past it the System One models refuse,
  and the fallback would pay full price for all of it.
- **The fallback is the LLM, through one tool.** `decide.llm` gets the state exactly as it is,
  between `<document id="…">` tags whose marker is made for that call, so no text written
  beforehand can close the document early. It is told to treat everything inside as data, never
  as instructions, and gets the questions in words. It answers by calling an `answer` tool whose
  schema has a probability for every label. A reply written as text is read from the first JSON
  object that carries `answers`; a refusal or a cut-off reply is an error, never a guess. Its
  probabilities are clamped and rescaled to sum to 1.
- **Every answer says who gave it.** Both backends return the same `Answer`, so its shape cannot
  show that the backend changed. A `Decision` carries its `Source`: the provider, the model, and
  the `Basis`. A System One provider says its models are trained for calibration (*measured* —
  a claim the decision record tests, below); an LLM writes its probabilities about itself
  (*stated*), and a stated 70% is not a measured one.
- **Cut-offs are per basis.** `Decision::sure(question, cutoffs)` returns the answer only when its
  confidence clears the cut-off for its basis; otherwise the caller keeps its default. The
  confidence is the one the provider measured and sent, when it sent one; else TypeSafe's
  definitions: for a choice, `(max p − 1/n) / (1 − 1/n)` (`|2p − 1|` for a yes-or-no); for a
  score, adjacency-aware, `max(0, 1 − Σ pᵢ·|i − m| / MAD)`, so an even split between neighbouring
  levels is surer than one between the ends. The same 85% yes passes as measured and fails as
  stated. The cut-offs (0.6 measured, 0.9 stated) are conservative starting points, untuned: they
  are a margin over an even spread, not a raw probability, and are meant to be set from the
  record.
- **Every decision has an id and a record.** Each decision point journals a `DecisionRow`: the
  id (the provider's request id, else one made for it), the point, provider, model, basis, why
  it fell back when it did, each question's probabilities, the milliseconds and the tokens. When
  the point learns what proved true, an `OutcomeRow` names the decision's id and the label that
  did. The two join into calibration per provider, model and basis, never pooled, so a cut-off is
  set from the record rather than guessed. A decision answered by the LLM is a model call like
  any other: its row reaches the usage ledger priced, and its cost the run's budget. Jev's input
  is priced at $0.042 per million tokens and its output is free, but TypeSafe publishes no cache
  prices, so it stays out of the price tables (never guessed) and its rows count in weighted
  tokens with `usd` null.
- **Containment.** A System One provider is a provider: only the human's own files define one,
  and only they set `decide.enabled` or `decide.via`. Until the human switches decisions on and
  names a classifier, no decision is made at all.
- **Decision points.** Each reads `Decision::sure` and journals its decision and outcome rows.
  **`bench.outcome` is attached to the loop:** a run built with
  `decisions(config, world, model, env)` reads a piped test run's output, so a sure green or red
  counts in the tally as seen (*the bench tally*, [the loop](03-loop.md)). It is on with decisions
  unless `decide.bench = false`; when the classifier fails, the fallback asks `decide.llm` through
  the run's own provider. What leaves for the decider is the command and the last 12 000
  bytes of its output; test output can print anything, so a provider outside the machine sees
  what the tests print. It is honoured once a binary builds runs from the configuration (the CLI,
  M6). **`model.route` is Metis** (`decide.route = true`, off by default, the human's files
  only): `route(config, decisions, caller, work, stop, meter)` returns the model a body rides
  and why, moving it within its office's band on a sure answer (*Metis routes within a band*,
  [the races](02-races.md)). Each route is journaled and priced into the meter's ledger, waits at
  most the least of 30 s and the caller's time left, and ends on an Esc. What leaves for the
  decider is the commission's first 8 000 bytes. It is honoured once dispatch builds bodies from
  commissions (M7). The others attach with
  their milestones:
  dispatch advice (M7), the Dark Elf's pre-screen, judged by its calibration first (#106, M10),
  and a commission's size (M11's value row).

### Where each piece comes from in isekai

Mnemorph is a port of proven behaviour, not a blank page. Each crate's first rung starts by reading
these isekai sources and turning their tests into fixtures.

| Mnemorph crate | isekai sources (Go, `~/kaginari/isekai`) | Size (src lines) |
|---|---|---|
| wire | `wire/` | 433 |
| races | `world/rank.go`, `config/ranks.go`, `rank/`, `value/` formulas | ~1,700 |
| ledger | `value/`, `instrument/`, usage instruments | ~1,500 |
| provider | `provider/` | 1,487 |
| sandbox | `sandbox/`, `guard/` | 858 |
| tools | `tool/`, `shell/` | 4,796 |
| memory | `memory/`, `toolbox/`, `compact/` | 6,924 |
| world | `world/`, `onto/`, `gate/`, `discover/` | 11,682 |
| loop | `loop/` | 2,511 |
| server | `board/` (API half), `mcp/` | ~2,500 |
| cli | `tui/`, `ui/`, `theme/`, `app/` (REPL half) | ~20,000 |
| board | `board/web/` | — |

`config/` (8,134 lines) shrinks the most: the rank table, ascension bands and value formulas move
into compiled code, leaving models, budgets, providers, creatures and envelopes.
