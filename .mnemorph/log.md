# Tempest — journal

Append-only. One entry per landed change: task, files, gate, result, learned.

---

### [2026-10-23T21:30:00+02:00] rimuru — the book, before the code
- **Task:** Veldora: plan a new harness from isekai — Rust core, TypeScript CLI and board, races and
  their behaviour hardcoded, Kijin as the outward race, Dark Elf as the code's adversary, ascensions
  reviewed, consumption checked step by step; docs first with hand-drawn diagrams (tldraw offline),
  then code; a prompt for race-card artwork.
- **Files:** README.md, docs/ (README, 01–06, roadmap, lessons, race-cards-prompt).
- **Gate:** n/a (no code). Instruments: isekai read at ~/kaginari/isekai main 5399d96 (v1.0.0),
  docs/canon/races.md, both log.md journals; a fresh Fable review of the book.
- **Result:** the book drafted — awaiting Veldora's review of the races and the card art.
- **Learned:**
  - isekai's config/ (8k lines) is mostly the rank table, bands and formulas that Tempest compiles in.
  - This machine has no Rust toolchain and no Bun: M0 starts with rustup; Node 22 and gh are present.

---

### [2026-10-23T23:10:00+02:00] rimuru — the book, corrected by a fresh Fable review
- **Task:** validate the book (Veldora's standing rule: big work is reviewed by a fresh Fable body);
  Veldora also asked for Leonardo.ai prompts for the race icons (docs/race-icons-prompt.md).
- **Files:** docs/02-races.md, 01, 03, 04, 05, 06, roadmap.md, lessons.md, race-cards-prompt.md, README.md.
- **Gate:** n/a (no code). Instruments: Fable review, 26 findings (19 confirmed, 7 plausible) — all
  addressed in the book.
- **Result:** the book is ready for Veldora's review of the races.
- **Learned:**
  - An envelope that may push a branch may push a workflow, and a workflow runs with the repo's
    secrets: workflow-touching pushes, tag pushes and registry publishes are never enveloped.
  - A witness the accuser can still edit is no witness: seals freeze it by hash, the human
    confirms, the fix copies it into the zone's tests, and a seal can be contested and expires.
  - Carving steps down the chain starves the only author (40 → 6): only cost is carved, by plan
    weight; steps and clock pass whole.
  - Budgets in raw tokens punish cached context; the unit is the weighted token.
  - Every path needs exactly one lawful author: the workspace soil (manifests, lockfiles) is a
    compiled zone, or no one can land the CVE fix the Dark Elf demands.
  - Image models cannot hold card geometry or blank plates: they paint the art; the board draws
    the frame and every word.

---

### [2026-10-23T21:07:00+02:00] rimuru — M0 foundations: the workspaces, the check, the world seeded
- **Task:** Veldora: crop the generated cards and icons, put the project on GitHub under Kaginari
  with milestones, and start building step by step.
- **Files:** assets/ (portraits 256/64/32 transparent, cards, source grids); GitHub Kaginari/tempest
  (transferred from a personal account): 17 milestones, labels, 17 tracking issues carrying each
  rung's lessons, D issues #18–23, M0 issues #24–30. Branch `m0-foundations`: Cargo workspace (11
  crates, layered, pinned 1.99.1), pnpm workspace (protocol, sdk, cli, board), justfile, CI
  (Linux + macOS), cargo-deny, templates, `.tempest/` (law, 30 creatures, this journal).
- **Gate:** n/a (no world yet). Instruments: `just rust ts layers` green — fmt, clippy -D warnings,
  nextest (no tests yet), deny ok, biome, typecheck, vitest 4/4, layer check 11 crates.
- **Usage:** ccusage for the whole day on this machine (all sessions, the parallel isekai session
  included — not separable): 77,989,603 tokens (76,246,735 cache read, 1,291,181 cache write,
  449,741 output), fable-5-1 and opus-5-5; $37.64 API-equivalent, not the plan's cost.
- **Result:** M0 part 1 in review as a pull request; the docs site (#29) follows as part 2.
- **Learned:**
  - ccusage counts per machine and day, not per session: a rung's own usage needs the session's
    transcript (or Tempest's own ledger from M6) — record the day total and say what it includes.
  - The cards grid drew its third row larger than the first two; crops keep each card as drawn
    rather than stretch it.
- **Correction:** the two entries above this one carry guessed clock stamps (21:30 and 23:10);
  both were written earlier on 2026-10-23, before 21:07. Stamps are read from the clock from here on.

---

### [2026-10-23T21:25:00+02:00] rimuru — M0 part 2: the book as a website
- **Task:** close M0 with the docs site (#29).
- **Files:** site/ (VitePress over docs/, Mermaid hand-drawn, local search), docs/02-races.md (the
  portraits of every race and ascended form), justfile (`docs`, `docs-dev`), CI job `docs`
  (build, upload the site as an artifact), .gitignore, pnpm-workspace.yaml.
- **Gate:** n/a. Instruments: `just docs` builds in ~24 s with no dead link; the races and
  communication pages screenshotted in headless Chromium — diagrams hand-drawn, portraits shown.
- **Result:** in review as a pull request.
- **Learned:**
  - VitePress over Starlight: it reads docs/ as it stands (no `title:` frontmatter, no content dir)
    and fails the build on a dead link — the book's links become a check.
  - Pages outside the site package cannot resolve `vue` under pnpm's strict node_modules: the
    site aliases it to its own copy.
  - A preview server started before a rebuild serves the deleted dist/: broken images and no
    diagrams. Restart it before judging a page.

---

### [2026-10-23T21:31:59+02:00] rimuru — M0 closed after a fresh Fable review; a stamp guessed twice
- **Task:** close M0: the milestone's Fable review (18 findings) and its fixes.
- **Files:** justfile (`check` snapshots the tree before and after — git failing now fails the
  check; `docs` and `workflows` join it), tools/check-layers.mjs (the architecture's exact edges,
  stale entries, the sandbox lint mirror), docs/05-architecture.md (the diagram draws the edges the
  manifests hold), crates/tempest-sandbox/Cargo.toml (`missing_docs`, `module_name_repetitions`
  restored), .tempest creatures (`site/` owned by slime-book; real verify lines: workspace
  `just check`, book `just docs`, kijin-ci `actionlint`; the workspace zone defined once — root
  files and tools/), docs/02-races.md (that definition; 64 px portraits), CI (`rustup toolchain
  install` first, actionlint, full history for the book), `.nvmrc` 22.23.2, roadmap and README
  (VitePress, `site/`, `just docs`), lessons (protection pending #34; `status` moves to M6),
  `vue` at the root instead of an alias, the task template's label. Decision issue #34 opened.
- **Gate:** n/a. Instruments: `just check` (with docs and workflows), CI on the pull request.
- **Usage:** ccusage, the whole day on this machine, all sessions: 110,556,427 tokens
  (108,019,604 cache read, 1,898,976 cache write, 634,863 output); $53.48 API-equivalent.
- **Correction:** the entry above, "M0 part 2", is stamped 21:25 but its commit is 21:18:20 — a
  guessed stamp, the same mistake as before the last correction, and it carries no usage line.
- **Learned:**
  - A promise to read the clock is not a mechanism; the same mistake came back within an hour
    (Nature 3). The stamp comes from `date -Iseconds` in the same command that appends the entry.
  - `test -z "$(git …)"` passes when git fails; compare snapshots under `set -e` instead.
  - A layer number admits edges a diagram forbids; hold the edges themselves.
  - Branch protection and Pages are paid features on a private repository: decided in #34.

---

### [2026-10-23T21:37:40+02:00] rimuru — M1 part 1: tempest-wire
- **Task:** the wire crate (M1, #3): the envelope parsed and emitted, isekai's tests carried over.
- **Files:** crates/tempest-wire (commission, report, forge, tag, fnv; tests/isekai_fixtures.rs,
  tests/properties.rs), docs/04-communication.md (the cut order, the never-cut set, three readings).
- **Gate:** n/a. Instruments: 19 tests (four property tests, 256 cases each), clippy pedantic
  `-D warnings`, `just check` green.
- **Result:** in review as #33.
- **Learned:**
  - `@$` orders join the never-cut set: isekai lost a relay order once and the chain stopped.
  - An order's body is one word; `@$ note this: …` is prose. An untyped `@U` opening with a
    kind's word reads as typed — the grammar, stated in the book.
  - clippy 1.99 flags `assert!(x.is_empty())` in tests and exact float comparison; budgets parse
    with an integer multiplier.

---

### [2026-10-23T22:00:53+02:00] rimuru — M1 part 2: tempest-races, the law compiled
- **Task:** the race law (M1, #3): races, shelves, soil, gates, the dispatch decision table, scores,
  meters, rises and falls, covered whole.
- **Files:** crates/tempest-races (race, shelf, dispatch, value; tests/law.rs), justfile (`law`:
  100% region coverage or the check fails), CI (cargo-llvm-cov), rust-toolchain.toml (llvm-tools),
  docs/02-races.md (how falls are read, default tiers), docs/04-communication.md (the High Elf's
  lane first; sideways dispatch meets the budget).
- **Gate:** n/a. Instruments: 18 races tests, `just law` at 100% regions / functions / lines
  (330 lines), proven to fail at 99.38% with one uncovered function; `just check` green.
- **Result:** in review as a pull request.
- **Learned:**
  - The table test caught a real ordering bug: a High Elf commands Orcs by race, so the lineage
    rule refused its other-lane route before the route was read. Special powers are checked first.
  - `unreachable!()` arms make 100% coverage impossible; list the ascended forms in the match
    instead, and give every refusal a method a test can call.
  - Stable Rust measures regions, not branches: "covered whole" means regions, said plainly.
  - #33's macOS job was cancelled before reaching a runner (no steps); a re-run, not a fix.

---

### [2026-10-23T22:11:56+02:00] rimuru — a diagram GitHub could not parse
- **Task:** Veldora: a Mermaid parse error on GitHub ("Parse error on line 22 … wire/parse.rs").
- **Files:** docs/04-communication.md (the message text), tools/check-diagrams.mjs (Mermaid parses
  every diagram of the book under jsdom), justfile (`docs` runs it), CI (the docs job runs it),
  package.json (jsdom, mermaid at the root).
- **Gate:** n/a. Instruments: the check passes on the fix (13 diagrams) and fails on the old text
  at the line GitHub named.
- **Result:** in review as a pull request.
- **Learned:**
  - In a sequence diagram `;` ends the statement, so `…Report.Wrote; the test…` broke the
    message. VitePress's newer Mermaid rendered it anyway: a page that renders on the site proves
    nothing about GitHub. The book's diagrams are now parsed, not only rendered.
- **Then:** Veldora: "you should use the icons we created in the docs". The portraits now stand
  wherever a race is named — each race's section on its page, the races and ascension tables, a
  legend under the overview's picture, the README, galleries on both prompt pages — and Rimuru is
  the site's logo and favicon (`assets/portraits/` is the site's public dir). The Mermaid diagrams
  keep their emoji: GitHub's Mermaid shows no images inside nodes.
- **Then:** Veldora: remove the Leonardo.ai prompt page. docs/race-icons-prompt.md is gone (its
  gallery of the generated icons moved to assets/README.md); the cards brief stays.

---

### [2026-10-23T22:22:40+02:00] rimuru — M1 closed after a fresh Fable review
- **Task:** close M1: the milestone's Fable review (20 findings) and its fixes.
- **Files:** tempest-wire — `hole.rs` (typed holes: cut, contest, budget), report.rs (fields folded
  to one line; a linear cut that folds earlier cuts into one hole; `parse` strict, `parse_raw` only
  for `wire:raw`; fenced claims and `Finding::address`; `Word`; `envelope(path, filed)`,
  `Courier::matches`, `Report::is_whole`), tag.rs (each raw tag joins its fields its own way),
  commission.rs (`@CAP 0` refused, fields folded); tests/regressions.rs pins every confirmed defect,
  properties reach claims that look like parts and any-string fields. tempest-races —
  `ShelfRefusal`, `Post`, `RECOVER_REPORTS`, `DECLINE_COOLDOWN_RUNS`, `Recent`, `recovers`,
  `rises(risen, …)`, the lane route needs its plan item, a literal 11×11 dispatch table. The book:
  02 (the law's real shape, `write` for every race in its soil, own docs in the soil table,
  posts compiled), 04 (`@SIZE`, typed holes, every reading of the grammar, the lane route's plan
  check), 06 (the budget hole), 05, roadmap (coverage in regions, said plainly), lessons (the
  lexicon row moves to M6).
- **Gate:** n/a. Instruments: 52 tests (wire 31, races 21), a 20,000-finding cut inside the suite,
  `just law` 100% regions / functions / lines, clippy pedantic clean, `just check`.
- **Result:** in review as #35.
- **Learned:**
  - A property test guards only its alphabet: every confirmed wire defect lived outside
    `[a-z0-9 ]`. Generators now carry words that look like the grammar's own parts.
  - The test oracle must not be the code under test: the dispatch table is written out by hand.
  - Accepting the raw form everywhere let prose overwrite a status; a mouth pair's agreement is
    now a parser the world chooses, not a habit of the parser.

---

### [2026-10-23T22:31:17+02:00] rimuru — M2: the usage ledger and the budget
- **Task:** M2 (#4): the ledger before the first model call — count, carve and stop.
- **Files:** crates/tempest-ledger (usage: weighted tokens, occupancy and spend; prices: TOML,
  unpriced is null; clock: UTC timestamps and days without a date crate; journal: append-only
  JSONL, one synced write per row, tolerant reader; row: the usage row with its prefix tokens;
  envelope: carve by plan weight under race ceilings, parent and challenge reserves, pre-flight,
  recorded spend, settle; caps: month, day, session; fold: by body, race, model, day, cache-hit
  and prefix shares), tests/budget.rs (512 random commission trees; folds by hand), the book
  (06, roadmap: the value fold moves to M11).
- **Gate:** n/a. Instruments: 23 ledger tests, 75 in the workspace; clippy pedantic; cargo-deny
  with the first real dependencies (serde, serde_json, toml); `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - Pre-flight and spend are two acts: the guard can refuse a call, nothing can refuse a cost
    that happened. An envelope that refused to record would lie about the session.
  - The binary ships no guessed prices: a wrong price is worse than *unpriced*. M3 fills the
    table from the providers' published pages.
  - A dollar cap is the wrong limit while unpriced models exist; caps count weighted tokens.

---

### [2026-10-23T22:43:07+02:00] rimuru — M2 closed after a fresh Fable review
- **Task:** close M2: the milestone's Fable review (13 findings) and its fixes; Veldora also asked
  for a study of the maki harness (filed as #38–#45: six ideas on their milestones, two decisions).
- **Files:** envelope.rs (slices move-only, stamped with their issuer and number, settled once —
  a replay or a stray slice is refused and the books untouched; `top_up`/`grant`; saturating
  arithmetic; envelopes and slices serialise for resume), usage.rs (integer weighted tokens,
  `estimate` for pre-flight), prices.rs (all four prices required; free output still weighs),
  clock.rs (digits only), journal.rs (one handle, the directory synced on creation, a BOM
  ignored; concurrent appenders and error paths tested), tests/budget.rs (no cloned envelope:
  every recorded token is counted beside the books and must reach the session once; overspends;
  races drawn from what each parent commands; weight 0; a test that the generator reaches every
  ceiling), the book (06, 02), race.rs doc. M3 issue #5 notes the pricing duties.
- **Gate:** n/a. Instruments: 30 ledger tests, 82 in the workspace; clippy pedantic; `just check`.
- **Result:** in review as #37.
- **Learned:**
  - A `Copy` handle to something that must come back once is a double-return waiting for a
    replayed report; make it move-only and stamp it with its issuer.
  - A test that clones state to please the borrow checker loses every invariant that spans the
    two copies — the property "spend never passes the session" was never checked.
  - Cache prices differ by model (a twentieth, a fortieth on the newest); a derived default is a
    guess, so the table states them.

---

### [2026-10-23T22:44:55+02:00] rimuru — a study of maki, written into the book
- **Task:** Veldora: study the maki harness against Tempest's philosophy, then write
  `study-maki.md`: where Tempest is stronger, and what it takes.
- **Files:** docs/study-maki.md (the picture, maki's benchmark and how to read it, a comparison
  table, what is taken — #38–#43 — what waits for a decision — #44, #45 — and what is not taken),
  docs/README.md and the site's sidebar.
- **Gate:** n/a. Instruments: an Opus 5.5 body read maki's code and its v0.5.5 benchmark report;
  `just docs` (14 diagrams parse).
- **Result:** in review as a pull request.
- **Learned:**
  - maki is Rust, not Go; its edge is token discipline (a 2.4 KB prompt, a prefix that never
    moves), not architecture — and it has no OS sandbox.

---

### [2026-10-23T22:57:19+02:00] rimuru — the lanes, the shared work queue and the handoff, from isekai
- **Task:** Veldora: isekai's shared queue (`isekai q`) and its handoff are missing from the Rust
  version. Read isekai's queue (082cd50, 588d847, 4878c4d, 7bfec94, fb26ce8) and app/handoff.go.
- **Files:** docs/04-communication.md (the session's lanes — queue, send, wake — and the shared
  work queue: revisioned snapshots, the rank law, relay-hold, `tempest q`, the queue tool,
  `merge=union`; protocol calls and events), docs/03-loop.md (the handoff: gathered facts, the
  template, offered at the stress zone and checkpoints, read with verification), 05 (storage),
  roadmap (M5–M8), lessons (six rows), .gitattributes.
- **Gate:** n/a. Instruments: `just docs` (15 diagrams parse).
- **Result:** in review as a pull request.
- **Learned:**
  - isekai's order book lived in one process's memory until the shared queue: a restart, a second
    session or the terminal lost orders. Tempest starts with the durable form.
  - A revision per snapshot line gives a race-free claim with no lock and no database.
- **Then:** Veldora: keep the MCP comparison with maki as part of the first simple MCP benchmarks
  — M13 now carries the MCP client (#48) and a repeatable benchmark against maki in its exit.

---

### [2026-10-23T23:23:27+02:00] rimuru — MCP: every spec in, only the needed ones out
- **Task:** Veldora drew the MCP path (server → client → agent-mcp → toolbox-mcp → agents of every
  rank) and asked for code wherever code is efficient.
- **Files:** docs/07-mcp.md (the drawing as canvas `docs/canvas/mcp-toolbox.png`, its Mermaid
  sketch, the roles: the client caches and starts lazily, agent-mcp is a Kijin post — a curator
  that indexes once per server version on the fast tier and a code-only conduit for every call —
  toolbox-mcp selects per commission, a model only to rerank a close shortlist), docs/README.md,
  the site's sidebar, 02-races (kijin-scout is agent-mcp), `Post::Scout`'s doc.
- **Gate:** n/a. Instruments: `just check` (16 diagrams parse).
- **Result:** in review as a pull request.
- **Learned:**
  - Veldora's agent-mcp layer keeps the whole catalog on Kijin soil: no rank's context ever holds
    it. The call path runs no model; models only index (rarely) and rerank (when close).
- **Then:** `just law` failed at 97% on a doc-comment change — stale profile data, not missing
  coverage (100% once cleaned). The recipe now cleans the coverage data first; a lessons row.

---

### [2026-10-23T23:55:48+02:00] rimuru — M3 part 1: the Anthropic client
- **Task:** Veldora: start M3. The Anthropic Messages client, from the Claude API reference (raw
  HTTP: Rust has no official SDK).
- **Files:** crates/tempest-provider — model (provider-neutral request and response; thinking
  blocks carried as opaque JSON, never rebuilt; mid-conversation system messages for append-only
  updates), sse (an incremental reader), transport (reqwest over rustls with the platform's trust
  store and the system proxy; a scripted transport that keeps every encoded request), retry (the
  ladder: 408/409/429/5xx/529, `retry-after-ms`/`retry-after`, full jitter, a cap; three waits:
  connect, first byte 10 min, stall 2 min), anthropic (encode: API version 2026-10-21, no
  sampling parameters, at most four breakpoints, no system message first; the assembler: text,
  tool calls — `{}` for none — opaque blocks by delta, usage from start and delta, a warning on
  1-hour writes, `message_stop` required; no retry once content has started), prices/anthropic.toml
  (Opus 5.5, Sonnet 5.5, Fable 5.1), `usage_row` (the junction with the ledger); tests/anthropic.rs
  (16, on a paused clock), tests/live.rs (gated: TEMPEST_LIVE=1 and a key). deny.toml allows
  CDLA-Permissive-2.0 (Mozilla's root certificates as data). The book (06).
- **Gate:** n/a. Instruments: 16 tests; two mutations (the started-guard, the empty-input rule)
  each turned tests red; clippy pedantic; cargo-deny; `just check`. Not run: the live test — this
  machine has no API key.
- **Result:** in review as a pull request; part 2 (OpenAI-compatible) follows.
- **Learned:**
  - The API moved under the training prior: `anthropic-version: 2026-10-21` rejects
    `temperature`/`top_p`/`top_k` at any value, cache reads are 0.05× on Opus 5.5 and 0.025× on
    Fable 5.1, and thinking blocks are bound to the conversation — the reference, not memory.

---

### [2026-10-23T23:58:48+02:00] rimuru — M3 part 2: the OpenAI-compatible client, one driver for both
- **Task:** M3 (#5): vLLM, OpenRouter, Ollama — chat completions — sharing the Anthropic client's
  ladder and waits.
- **Files:** crates/tempest-provider — drive.rs (one attempt loop: the ladder, the three waits, no
  retry once content has started; each provider brings an `Assembler`), anthropic.rs (now a thin
  client over the driver; its 16 tests unchanged and green), openai.rs (encode: system blocks
  joined, `tool` messages per result, assistant `tool_calls` with JSON-string arguments, opaque
  blocks and cache marks dropped, `stream_options.include_usage`; assemble: text, tool-call
  fragments merged by index, `{}` for empty arguments, cached tokens split from input, finish
  reasons mapped — `content_filter` is a refusal — `[DONE]` required, in-stream errors; a bearer
  key only when one is given), tests/openai.rs (3), clippy.toml (product names in docs).
- **Gate:** n/a. Instruments: 19 provider tests, clippy pedantic, `just check`.
- **Result:** committed; its pull request opens after part 1 (#50) lands.

---

### [2026-10-24T00:16:48+02:00] rimuru — M3 closed after a fresh Fable review
- **Task:** close M3: the milestone's Fable review (20 findings) and its fixes.
- **Files:** drive.rs (a timeout on the response head, the call's deadline `Policy::turn`,
  `x-should-retry` obeyed, empty chunks are not the first byte, error bodies in every shape),
  retry.rs (three retries, the lesson's count; a server's `retry-after` honoured whole, the deadline
  decides), error.rs (`Wait::Turn`, numeric codes retried, `model_not_found`), anthropic.rs
  (system-message placement checked; `eager_input_streaming` on every tool; "started" only once
  something is shown — thinking is not; opaque blocks assemble `input` and object deltas, an
  unknown text delta is a warning; `stop_details` kept; a refusal's partial content discarded; a
  tool call cut by `max_tokens` dropped; another provider's opaque blocks left out), openai.rs
  (`finish_reason: error` is a retryable failure; reasoning and tool-call extras kept and replayed
  as `openai.*` opaque blocks; index-less fragments; one ToolStart per call; a mid-conversation
  note joins the user's turn), transport.rs (a redacted `Debug`; test doubles hidden from the
  docs; a silent-head reply), tests/review.rs (19), issues #52 (real cassettes, needing a key) and
  a note on M4a (#7: the fallback chain).
- **Gate:** n/a. Instruments: 35 provider tests, clippy pedantic, `just check`.
- **Result:** in review as #51.
- **Learned:**
  - "Started" must mean "shown": a turn that thinks for minutes and then hits an overload lost
    everything because a thinking block counted as content.
  - A review export that rebuilds from scratch needs gigabytes: the disk filled. Check `df` first
    and reuse the checkout's target directory.
  - A test with `|| true` in an assertion proves nothing; caught before landing, removed.

---

### [2026-10-24T00:25:15+02:00] rimuru — colour in every diagram; the MCP drawing keeps one picture
- **Task:** Veldora: remove the drawing they sent from the MCP page (keep the diagram), and use
  colours in the diagrams.
- **Files:** docs/07-mcp.md and docs/canvas/mcp-toolbox.png (removed); all 16 diagrams: a palette
  shared by every page — each race in its own colour (Rimuru azure, Elf emerald, Orc amber, Slime
  teal, Kijin crimson, Dark Elf violet, Veldora gold, ascended forms dashed gold), steps blue,
  gates amber, stores indigo, the outside grey, outcomes green and red, TypeScript pink and Rust
  orange; subgraphs tinted; sequence diagrams coloured by phase (`rect` bands: orders down, the
  gate, reports back) — a coloured box per participant widened them past reading. Every diagram's
  config wraps labels at 360 px: Mermaid sized boxes for fewer lines than it wrapped, and cut the
  last one (already so on main).
- **Gate:** n/a. Instruments: 16 diagrams parse; the site built and screenshotted (races,
  communication, overview, architecture).
- **Result:** in review as a pull request.

---

### [2026-10-24T00:29:03+02:00] rimuru — the OpenAI-compatible client meets a real server
- **Task:** Veldora gave an OpenRouter key for live tests (passed only through the test's
  environment; never written to a file or a commit).
- **Files:** crates/tempest-provider/tests/live_openrouter.rs (gated: a text answer and a full tool
  round trip on a free model; a recording transport, `TEMPEST_RECORD=<dir>`, saves each response
  body with generation ids blanked), tests/cassettes/openrouter-{text-0,tool-0,tool-1}.sse (real
  streams), tests/cassettes.rs (replayed every run, no key), openai.rs (`cache_write_tokens` is
  its own class), tests/review.rs (its test).
- **Gate:** n/a. Instruments: both live tests green on `nvidia/nemotron-3.5-lightning:free`
  (30/111 tokens; 299/42 then 375/22); 39 provider tests; clippy; no key in any cassette (a test
  guards it).
- **Result:** in review as a pull request.
- **Learned:**
  - The free model isekai used (`nemotron-3-super-120b-a12b:free`) is gone: OpenRouter answers 404
    "Provider returned error" — the client read it right, non-retryable. Free models churn: the
    live default must be easy to change (`TEMPEST_LIVE_OR_MODEL`).
  - A real stream carries both `reasoning` and `reasoning_details`, `cache_write_tokens`, and the
    provider's own `cost`; the hand-written fixtures had none of them.

---

### [2026-10-24T00:51:56+02:00] rimuru — the harness is renamed Mnemorph
- **Task:** Veldora wanted a name from myth about the digital world, evolution and memory, and
  chose **Mnemorph** — Mnemosyne, memory, + morph, change of form: memory that evolves. Free on
  npm and crates.io. Then the brand: the dragon-and-crowned-slime emblem as the icon, the banner
  with its old corner wordmark replaced by the emblem and the new name.
- **Files:** every crate (`mnemorph-*`, the daemon `mnemorphd`), the packages (`@mnemorph/*`),
  the world (`.tempest/` → `.mnemorph/`), the env vars (`MNEMORPH_LIVE`, `MNEMORPH_RECORD`,
  `MNEMORPH_LIVE_OR_MODEL`), the book, the site (logo and favicon from `assets/brand/`),
  `assets/brand/` (icon at 32–512, favicon.ico, banner.jpg, social-preview.jpg),
  `assets/source/{banner,emblem}.jpg`; the GitHub repo is `Kaginari/mnemorph`.
- **Gate:** n/a. Instruments: `just check` green after the rename.
- **Result:** in review as a pull request.
- **Learned:**
  - The journal's past entries keep the old name — it is history; the recorded cassettes keep
    it too: the server read "Tempest is a harness." and answered so, and a recording is not
    edited.
  - Replacing text baked into generated art works: inpaint the gold strokes (OpenCV Telea),
    blur the patch, a dark wash, then set the new mark in Cinzel with a gold ramp. Cleaner than
    asking the image model again, and the name can change once more for free.
