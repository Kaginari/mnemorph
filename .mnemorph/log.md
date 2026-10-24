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

---

### [2026-10-24T01:06:52+02:00] rimuru — new race icons and a new banner
- **Task:** Veldora generated a cuter race-icon sheet (ten hexagons) and a new banner (the cast in
  a digital city, the emblem and MNEMORPH in the corner) and asked to use them.
- **Files:** assets/portraits/ (ten races and ascended forms, cut with a hexagonal mask, 256/64/32),
  assets/brand/{banner,social-preview}.jpg, assets/source/{banner,race-icons}.jpg,
  assets/README.md.
- **Gate:** n/a. Instruments: every icon previewed on dark and light, at 64 and 32 px.
- **Result:** in review as a pull request.
- **Learned:** the sheet has no Oni — Oni and Veldora keep their portraits from the first grid
  until a matching hexagon exists.

---

### [2026-10-24T01:13:09+02:00] rimuru — the race cards, drawn from the law
- **Task:** Veldora asked for race cards — a Gemini prompt, or made here from the new icons.
  Made here: an image model cannot hold the frame's geometry or spell the words, and the brief
  already said the frame and text come from the race law.
- **Files:** tools/cards.py (draws 13 cards at 2x, saves 600x840: frame in the race's colour, tier
  gems, name, epithet, duty, the six tool classes with the held ones lit, soil, the rise or
  command line, the meter and its ok line), assets/cards/ (Veldora, the six races, the five
  ascended forms, the back), assets/fonts/ (Cinzel, Rajdhani, OFL), docs/race-cards-prompt.md,
  assets/README.md.
- **Gate:** n/a. Instruments: the whole set previewed side by side; `just check`.
- **Result:** in review as a pull request.
- **Learned:** Pillow here is 7.0 — no `rounded_rectangle`, no `getbbox`: rounded shapes are
  masks, text is measured with `getsize`. Oni's art is still the old portrait; a hexagon Oni
  would make the set uniform.

---

### [2026-10-24T01:19:48+02:00] rimuru — the Mnemorph theme, as a package and a skill
- **Task:** Veldora pasted a design spec (Gemini's, from the banner and the icon sheet) and asked
  to set it up as a skill for the theme.
- **Files:** packages/theme/ (`tokens.css` for the web, `index.ts` for the CLI, a test that holds
  them equal and every text colour at WCAG AA on every surface), site/.vitepress/theme/ (the book
  in the theme, dark first, the toggle mirrored onto `data-theme`), site/package.json,
  site/.vitepress/config.ts (fonts, appearance), .claude/skills/mnemorph-theme/SKILL.md.
- **Gate:** n/a. Instruments: theme tests (33), typecheck, site build, a headless screenshot of
  the races page in dark.
- **Result:** in review as a pull request.
- **Learned:**
  - The spec's pink `#ff0066` fails AA on a raised panel (4.24:1) — the contrast test caught it;
    `#ff2a7d` passes. Light mode was not in the spec; it has its own darker accents.
  - Dark first broke the hand-drawn diagrams: neutral-theme hachure on a dark page reads as
    black and white stripes. They keep a light paper sheet in both modes.
  - Kill a server by its port (`fuser -k`), never `pkill -f` on a pattern the killing command's
    own line contains — that killed the shell twice.

---

### [2026-10-24T01:30:01+02:00] rimuru — Oni and the hex emblem; one sheet for every icon
- **Task:** Veldora regenerated the icon sheet with the two it lacked: Oni and Mnemorph.
- **Files:** assets/source/race-icons.jpg (the twelve-hexagon sheet, replacing the ten), every race
  portrait recut from it (256/64/32), assets/brand/mnemorph-hex{,@64,@32}.png, assets/cards/
  (redrawn: Oni on its new art), assets/README.md, the theme skill's asset table.
- **Gate:** n/a. Instruments: the twelve previewed together; the Oni card beside Kijin and Rimuru.
- **Result:** in review as a pull request.
- **Learned:** measure each sheet's grid — this one is 6 columns, hexagons 186 px wide, and the
  last column sits 5 px left of an even pitch. Oni's horns break out of its frame on purpose; the
  hex mask trims them, which reads fine at icon size.

---

### [2026-10-24T01:54:57+02:00] rimuru — the repository goes public; solo mode stays
- **Task:** Veldora decided #34 (public) and #44 (keep the solo path, escalating on the stress
  metrics).
- **Files:** docs/02-races.md (solo mode and its three escalation signals), docs/roadmap.md (M5
  builds solo with its stress reading, M7 builds the escalation and measures it, nothing removed),
  docs/06-consumption.md, docs/study-maki.md (#44 under Decided).
- **Gate:** n/a. Instruments: every commit of the history scanned for credentials before the
  visibility change — none; `just check`.
- **Result:** the repository is public; the book change in review as a pull request.
- **Learned:** escalation reads only code-held signals — occupancy from the ledger, a soil
  refusal, a shelf refusal — so solo never decides by itself that it is "stressed".

---

### [2026-10-24T02:03:52+02:00] rimuru — Veldora wears the emblem
- **Task:** Veldora asked for the hex emblem as their own icon.
- **Files:** assets/portraits/veldora{,@64,@32}.png (from assets/brand/mnemorph-hex), assets/cards/veldora.png (redrawn), assets/README.md.
- **Gate:** n/a. Instruments: the redrawn card viewed.
- **Result:** in review as a pull request.
- **Learned:** every portrait now comes from one Gemini sheet — the set is uniform.

---

### [2026-10-24T02:14:37+02:00] rimuru — M4 rung 1: the shell classifier
- **Task:** M4 began ("go"). First rung: every shell command a body runs is classified read,
  write, outward or destructive — or refused by the guard — from a parsed AST.
- **Files:** crates/mnemorph-tools/src/classify/ (mod.rs: the walker over brush-parser's AST,
  prefix-max, pipelines, subshell cwd; words.rs: braces, quotes, `~`/`$HOME`, substitutions;
  paths.rs: whole-segment placement after `..` and symlinks, devices, credentials, dangerous
  roots; commands.rs: wrappers, shells, git, gh, registries, network, secret stores, disks,
  file commands), tests/corpus.txt (257 rows) + corpus.rs, tests/properties.rs.
- **Gate:** n/a. Instruments: corpus 257/257; proptest "arguments only tighten" (4000 cases);
  never panics on arbitrary input; a symlink out of the world judged outside; `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - brush files `x=y` after the command name as an assignment word: `dd of=/dev/disk2` read as
    nothing until suffix assignments became arguments. A corpus row caught it.
  - Prefix-max makes lesson 29 true by construction, and costs one row of precision: bare
    `git stash` writes, so `git stash list` classifies as write too.
  - A glob has two meanings: a bare `*` is all of its directory (`rm -rf ~/*` is the home);
    `*.log` is a child. `find -delete` acts on what it finds, never its start.
  - The disk is at its last few hundred MB: `target/` is 3.6 GB of it.

---

### [2026-10-24T02:18:38+02:00] rimuru — the maki study stays local
- **Task:** Veldora asked to keep docs/study-maki.md out of git, local only.
- **Files:** .gitignore, docs/README.md and site/.vitepress/config.ts (link and sidebar entry gone; `srcExclude` keeps a local build from rendering it); docs/study-maki.md untracked, kept on disk.
- **Gate:** n/a. Instruments: a clean site build has no maki page; diagrams parse.
- **Result:** in review as a pull request.
- **Learned:** the repository went public before this, so the file remains in history; removing it there would need a history rewrite and a force push — Veldora's call.

---

### [2026-10-24T02:32:38+02:00] rimuru — the classifier on macOS: aliases of system directories
- **Task:** CI on macOS failed four rows of the classifier corpus that pass on Linux.
- **Files:** crates/mnemorph-tools/src/classify/paths.rs (danger and credentials judged on the written and the resolved path, with `/private` and `/System/Volumes/Data` removed), tests/corpus.txt (three `/private` rows).
- **Gate:** n/a. Instruments: corpus on Linux; CI on macOS.
- **Result:** pushed to the classifier PR.
- **Learned:** resolving symlinks before judging turned `/etc` into `/private/etc` and `/home` into `/System/Volumes/Data/home` on macOS — no longer top-level, no longer a credential path. Containment needs both spellings, and the corpus runs on every OS for exactly this.

---

### [2026-10-24T02:41:18+02:00] rimuru — M4 rung 2: the file tools
- **Task:** read, write, edit, glob and grep, behind the world's border and the body's soil.
- **Files:** crates/mnemorph-tools/src/tool.rs (the `Tool` trait, `registry`, `call`, `Outcome`
  and `Meta`, `Context` with `inside`/`writable`), src/files.rs (the five tools),
  tests/files.rs (12 tests against a real world on disk); the classifier's path resolver is
  shared, so the border means the same in both.
- **Gate:** n/a. Instruments: 15 tool tests; `just check` (136 tests).
- **Result:** in review as a pull request.
- **Learned:**
  - The lessons map one to one onto tests: bad JSON and unknown fields are named errors and
    nothing runs; a symlink out of the world is refused by every file tool; edit names the
    closest line or every matching line; a re-read of an unchanged window is a pointer with
    `meta.unchanged`, and a write resets it.
  - Writes go to a temporary sibling and a rename: a file is replaced whole or not at all.

---

### [2026-10-24T02:50:28+02:00] rimuru — M4 rung 3: the human gate and bash
- **Task:** the gate as an interface, and a bash tool that is classified, shelved, gated and
  contained.
- **Files:** crates/mnemorph-tools/src/gate.rs (`Gate`, `Ask`, `Answer`, `Pending`: first answer
  wins, silence denies; `Terminal`, `NoHuman`, `Scripted`), src/shell.rs (bash: classify →
  shelf → gate → a process group of its own, killed whole at the deadline and after the shell
  exits; a scrubbed environment), src/tool.rs (the context carries the race, the gate and the
  home; `call` checks the shelf before any tool runs), tests/shell.rs, tests/gate.rs.
- **Gate:** n/a. Instruments: 27 tool tests; `just check` (148 tests).
- **Result:** in review as a pull request.
- **Learned:**
  - An `Ask` has no recommendation field, so an approval card cannot nudge (lesson 59) — by
    construction rather than by rule.
  - Killing the group after a normal exit is what un-hangs `(sleep 30) & echo done`: the
    background child holds stdout, and only its death closes the pipe.
  - Scope held back on purpose: one shell per call (a `cd` does not carry over) until the loop
    needs more; a dedicated git tool waits for the landings in M7.

---

### [2026-10-24T03:06:48+02:00] rimuru — M4 rung 4: the sandbox and its escape suite
- **Task:** contain every command: landlock + seccomp on Linux, Seatbelt on macOS, the escape
  suite as M4's exit; then, at Veldora's word, let an approval open what it names.
- **Files:** crates/mnemorph-sandbox/ (lib.rs: `Policy`, `Sandbox` (detect, command, status),
  the credential lists, `harden_self`; linux.rs: landlock rules + a seccomp denylist, sockets
  denied without network; macos.rs: the Seatbelt profile; src/bin/mnemorph-jail.rs: restrict
  self, then exec; tests/escape.rs: 9 escapes), mnemorph-tools (bash runs in the context's
  sandbox; the classifier's `outside` paths reach the card and, approved, the policy; the
  credential list comes from the sandbox crate), docs/05-architecture.md, docs/roadmap.md.
- **Gate:** n/a. Instruments: the escape suite on this 5.15 kernel (landlock ABI 1); a jailed
  bash test in the tools crate; `just check` (158 tests); macOS in CI.
- **Result:** in review as a pull request.
- **Learned:**
  - A helper that restricts itself and execs needs no unsafe code and no namespaces — it works
    where Ubuntu restricts unprivileged user namespaces, and a seccomp `socket()` denial closes
    the network for every program, Unix sockets included.
  - Landlock already keeps a sandboxed process out of `/proc` of processes outside its domain:
    the harness's environment was closed before `harden_self` was added.
  - On ABI 1 every cross-directory rename is refused; `mv` and git fall back to copying and
    still work.
  - The sandbox must not be stricter than the human: an approval opens exactly the outside
    paths on the card, nothing hidden behind a variable.

---

### [2026-10-24T03:32:20+02:00] rimuru — M4 rung 5: hardening
- **Task:** close the gaps the threat model ranked: named guard rules a configuration may turn
  into questions (the floor never), a tripwire over git's executable configuration and the
  world's records, the jail's limits and namespace denial, macOS reads from an allowlist and
  without LaunchServices. Veldora added: `.mnemorph/shared/` and `tmp/` stay writable.
- **Files:** mnemorph-tools: classify/mod.rs (`Rule`, the floor, `Verdict::Refuse { rule,
  reason, class }`), every refusal site named, src/tripwire.rs, shell.rs (asked rules go to the
  gate; snapshot → run → restore), tool.rs (`with_asked_rules`, protected paths refused);
  mnemorph-sandbox: linux.rs (limits; `clone` with namespace flags, `clone3` → ENOSYS),
  macos.rs (allowlist reads, a short list of services); tests in both crates;
  docs/05-architecture.md; issue #67 (openat2 for the file tools).
- **Gate:** n/a. Instruments: `just check` (168 tests); macOS in CI.
- **Result:** in review as a pull request.
- **Learned:**
  - Landlock only adds access: a read-only corner inside a writable tree is impossible. A
    snapshot taken before and restored after — when the process group is already dead — covers
    it on every platform, and nothing can race the restore.
  - A tripwire must watch only what existed: `git init` inside the call creates a repository
    that belongs to the command, and `.mnemorph/tmp` must exist before the first snapshot.
  - Twice an edit silently missed rustfmt's layout and a test caught it (the macOS unused
    import; the uncalled limits). Edits now assert the anchor matched.

---

### [2026-10-24T03:35:49+02:00] rimuru — an approved new file opens alone
- **Task:** a side review found that approving a new file outside the world opened its whole folder (the sandbox can only name what exists): approving `~/notes.txt` opened the home, so a hidden second write to `~/.bashrc` would pass. Veldora asked for a smart fix.
- **Files:** crates/mnemorph-tools/src/shell.rs (`make_approved`: the harness makes the approved file, empty, then only that file opens; a failed command leaves no empty file), tests/shell.rs (a sibling write in the same command is refused), docs/05-architecture.md; `LaunchServices` in backticks for clippy on macOS.
- **Gate:** n/a. Instruments: `just check` (168 tests).
- **Result:** pushed to the hardening pull request.
- **Learned:** never widen to a parent to name a child: make the child, then name it. An approved delete outside the world stays refused, since the kernel ties deletion to the folder.

---

### [2026-10-24T03:46:58+02:00] rimuru — isekai's self-heal, carried into the book
- **Task:** Veldora asked to bring isekai's self-healing work ("wave 4": the council, relief, the
  doom checkpoint, salvage, evolution; the roster self-heal and stall guards on the
  `guards-h1-h9` branch) into Mnemorph.
- **Files:** docs/03-loop.md (the stall instrument; the council's keys and forced escalation;
  a Self-heal section — relief rows, the clock-death memo, roster self-heal, `housekeep --heal`,
  one live court per seat), docs/02-races.md (evolution's trigger, arm order, overlay proof,
  cooldown, propose mode), docs/roadmap.md (M5, M7, M11), docs/lessons.md (five rows).
- **Gate:** n/a. Instruments: diagrams parse; the site builds.
- **Result:** in review as a pull request.
- **Learned:** isekai never named it "wave 4": the group is v0.3.0's "the world measures, answers
  and heals itself", spec in its races canon. Roster self-heal exists only on an unmerged branch
  there — it is Mnemorph's from the first world rung (M7).

---

### [2026-10-24T03:56:35+02:00] rimuru — M4a: configuration
- **Task:** the config rung: TOML layers with origins, `explain`, holes, writes that keep
  comments, the tracked file only tightening the gate, models, budgets, providers, Oni
  envelopes; and the fallback chain the M3 review left to this rung (#7).
- **Files:** crates/mnemorph-config/ (lib.rs: `Paths`, `Layer`, `Origin`, `Config::load`,
  `explain`, holes, the loosening keys, `model_for`, `fallback_chain`, `providers`, `envelopes`
  vetted by the classifier; write.rs: `set` through toml_edit, parsed before it replaces),
  tests/config.rs (12), tools/check-layers.mjs (config: races, tools; loop: config),
  docs/05-architecture.md (crate graph, tree, a Configuration section), .gitignore.
- **Gate:** n/a. Instruments: `just check` (180 tests); the layer check counts 12 crates.
- **Result:** in review as a pull request.
- **Learned:**
  - Refusing loosening keys from the tracked file is only half; the other half is that no body
    can write `.mnemorph/` (the tripwire), so "never from text a body wrote" holds by
    construction.
  - Envelopes are vetted by the same classifier the gate trusts: a pattern the guard calls
    destructive or refuses can never become a standing permission.
  - Veldora: a part ships with its docs in the same pull request — now a standing rule.

---

### [2026-10-24T04:01:30+02:00] rimuru — the file tools open beneath the world
- **Task:** #67 — the file tools checked a resolved path, then opened it by name: a symlink
  swapped in between would have led them out of the world.
- **Files:** crates/mnemorph-tools/src/beneath.rs (open the checked path again from the world's
  root, one component at a time, following no symlink; writes renamed inside the same directory
  handle), src/files.rs (read, write, edit and grep through it), docs/05-architecture.md.
- **Gate:** n/a. Instruments: a swapped directory and a swapped last component refused by read
  and write; `just check`.
- **Result:** in review as a pull request.
- **Learned:** no `openat2` is needed: the check resolves the path, and a walk with `O_NOFOLLOW`
  re-checks it by construction, on Linux and macOS alike.

---

### [2026-10-24T04:22:11+02:00] rimuru — M4 and M4a close after a fresh Fable review
- **Task:** close M4 (tools, sandbox, human gate) and M4a (config) with an independent review
  of everything since #61, and fix what it found.
- **Findings, each reproduced before fixing:**
  - high: a substitution inside a parameter or arithmetic expansion (`${z:-$(curl … | sh)}`,
    `$(( $(…) ))`) was never classified — brush keeps that text raw. The expansion's own source
    is now read again as words and every substitution in it classified; unreadable text is
    refused. Every word piece is named, so a new one is a compile error.
  - medium-high: envelope vetting classified only the text before `*`, so `gh *` and
    `git push *` passed. A bare `*` word is now judged at its worst (each destructive
    argument tried); a `*` inside a word cannot become a flag.
  - medium: the first refusal won, so an askable rule could mask a floor rule found later in
    the same command (`sed -i … /dev/sda`). A floor refusal now displaces an askable one.
  - medium: the tracked file could define providers (an attacker `base_url` with a real
    `key_env`). Providers are honoured only from the local or global file.
  - low: `AUTH` scrubbed `AUTHOR`; it now counts only as a whole word of the name.
- **Files:** classify/{words,mod}.rs, shell.rs, mnemorph-config/src/lib.rs, corpus rows and
  tests for each, docs/05-architecture.md.
- **Gate:** n/a. Instruments: `just check` (184 tests); the review's coverage list (wrappers,
  tables, paths, sandbox, gate, tripwire, config) found sound.
- **Result:** in review as a pull request; M4 and M4a close when it merges.
- **Learned:** each finding passed every existing test, because each test proved a narrower
  claim than its name: pure-floor commands only, envelopes with `*` after the dangerous word,
  substitutions only at word level. A test's name is a claim; its inputs must reach the claim's
  edges.

---

### [2026-10-24T04:31:14+02:00] rimuru — a kernel guard, not a jail; no guard, no command
- **Task:** Veldora: "i dont want a jail more like a helper a kernel guard"; and a side review:
  with the helper missing, commands ran unprotected and only the status line said so.
- **Files:** crates/mnemorph-sandbox (the helper is `mnemorph-guard`, `Sandbox::Guard`,
  `MNEMORPH_GUARD`), crates/mnemorph-tools (bash refuses when no guard is in force unless the
  context allows it; `Context::with_unguarded`), crates/mnemorph-config
  (`sandbox.allow_unguarded`, a loosening key: local or global file only), tests in all three,
  docs/05-architecture.md (what the guard is and is not; why once per command).
- **Gate:** n/a. Instruments: 200 commands timed bare and through the guard — about 1 ms each;
  `just check` (185 tests).
- **Result:** in review as a pull request.
- **Learned:** the helper never was a jail — no other world, the real files and tools — only a
  kernel-enforced permission list, set by the helper on itself before it becomes the command.
  It runs per command because those restrictions are one-way; a resident helper would still
  have to restrict a fresh process each time. Naming it for what it does settled the question.

---

### [2026-10-24T04:41:22+02:00] rimuru — M5 rung 1: the loop core
- **Task:** the six beats for one body over the parts M2–M4 left ready.
- **Files:** crates/mnemorph-loop/src/model.rs (`Model`: Anthropic, OpenAI-compatible, a
  scripted model for tests), beats.rs (the beat journal; `conversation` rebuilds a run; a call
  cut mid-flight is answered with an error), stress.rs (guts, doom, escalation, `repin`,
  `actBudget`; a landing resets them), run.rs (perceive · plan · act · verify · record; the
  steps, clock, context and cost meters; salvage on a clock death; priced ledger rows; resume),
  tests/loop.rs (14); mnemorph-tools re-exports `Sandbox`; docs/03-loop.md (the journal,
  salvage, guts scoring).
- **Gate:** n/a. Instruments: 14 loop tests against the scripted model; `just check` (199).
- **Result:** in review as a pull request.
- **Learned:**
  - The layer check caught a test reaching past its layer (loop → sandbox); the type belongs
    with the API that takes it, so tools re-exports it.
  - A guts window restarts after each warning, so a test's pattern must restart with it: my
    first doom script drifted half a window and never reached three warnings.

---

### [2026-10-24T04:48:24+02:00] rimuru — M5 rung 2: solo mode and its end gate
- **Task:** solo mode — Rimuru's voice, a Slime's hands, the whole repository as soil — and the
  first end gate.
- **Files:** crates/mnemorph-loop/src/solo.rs (`SOLO_PROMPT`, `solo`), src/gate.rs (`snapshot`,
  `EndGate::judge`: traits hold, duties done, tests intact), src/run.rs (`with_gate`; one chance
  to fix, then FAIL; each verdict a `gate` beat), src/beats.rs, tests/solo.rs (7), docs
  (03-loop end gate in solo, 02-races solo's shelf).
- **Gate:** n/a. Instruments: 21 loop tests; `just check` (206 tests).
- **Result:** in review as a pull request.
- **Learned:** the gate finds writes by snapshotting the tree, not by trusting the write tool's
  metadata: a `rm` through bash is caught the same as an edit — the first test that would have
  slipped past metadata alone.

---

### [2026-10-24T04:54:44+02:00] rimuru — M5 rung 3: the session, its queue and Esc
- **Task:** the session's queue lane and interrupt, and the message-sequence validator.
- **Files:** crates/mnemorph-loop/src/session.rs (`Session`: a run and its queue; `drive` asks
  each queued line in turn), src/run.rs (`Interrupt`: fired from any thread, seen during the
  model call through `select!` and between tool calls; `Ending::Interrupted`; validation before
  each request; the step budget per ask), src/sequence.rs (`validate`), tests/session.rs (4 and
  a 200-case proptest), docs/03-loop.md (the session section).
- **Gate:** n/a. Instruments: 26 loop tests; `just check` (211).
- **Result:** in review as a pull request.
- **Learned:**
  - A validator inside the run makes a proptest over requests vacuous: a broken conversation
    never reaches the model, it ends the run. The property must also say no run ended that way.
  - The step budget belongs to a commission, not to the run: a session of many asks would
    otherwise run out of steps across them.

---

### [2026-10-24T04:58:09+02:00] rimuru — Esc and an answer arriving together
- **Task:** CI on macOS failed `esc_before_the_acts_runs_none_of_them`; Linux passed.
- **Files:** crates/mnemorph-loop/src/run.rs (`select!` is `biased`, the call first), docs/03-loop.md.
- **Gate:** n/a. Instruments: the loop tests five times running.
- **Result:** pushed to the session pull request.
- **Learned:** `tokio::select!` breaks ties between ready branches at random — the two OSes simply drew differently. A tie needs a rule: an answer already arrived is kept, Esc is honoured before its acts.

---

### [2026-10-24T05:03:49+02:00] rimuru — M5's exit: one real task, solo, on a live model
- **Task:** the exit criterion — a real task end to end against a live model, its spend
  matching the ledger.
- **Files:** crates/mnemorph-loop/tests/live.rs (gated: `MNEMORPH_LIVE=1` and
  `OPENROUTER_API_KEY`; the key only through the environment).
- **Gate:** the end gate's verify (`grep -q 'harness for agents' notes.md`) passed.
- **Instruments:** OpenRouter, `nvidia/nemotron-3.5-lightning:free`, 8.8 s: 5 steps (find,
  read, edit, answer), 12 beats; usage 7712 input · 2096 cache read · 461 output over 5 calls —
  the ledger's 5 rows equal the provider's 5 reported usages, call for call. $0 (a free model).
  `just check` (211).
- **Result:** in review as a pull request; M5 closes after its review.
- **Learned:** the scripted model and the real one agree on the shape of a turn: nothing in the
  loop needed to change for the live run.

---

### [2026-10-24T05:19:59+02:00] rimuru — M5 closes after a fresh Fable review
- **Task:** close M5 with an independent review of the loop (#74–#77), and fix what it found.
- **Findings, each reproduced by the reviewer and now a test that fails on the old code:**
  - high: an empty model reply entered the conversation as an empty turn — every later ask
    failed as malformed, and the journal carried it into resume. Replies are now cleaned
    (empty text dropped, repeated ids made unique, an empty reply stated), and an empty, cut or
    paused reply is a nudge, not an answer.
  - high: the stress instruments lived for the run: forty questions, or one read asked four
    times, stalled a healthy session. They now start again with each ask.
  - medium: resume forgot spend and occupancy (it read them from nowhere), so a budget
    checkpoint resumed into one more call. It now reads the ledger.
  - medium: a reply cut by `max_tokens` was taken as the answer; a stale Esc interrupted the
    next ask; tests intact missed `.only(`, `t.Skipf(`, a padded test file, and a test removed
    from inside a file.
  - low: pause_turn could not be continued; duplicate tool ids ended the run; the lessons table
    promised compaction, the stress threshold and the bench tally to M5 — they are M5b now,
    with blocking tools (#78).
- **Files:** crates/mnemorph-loop/src/{beats,run,gate}.rs; tests (11 new); docs/03-loop.md,
  docs/roadmap.md (M5b), docs/lessons.md.
- **Gate:** n/a. Instruments: the new tests run against the old `src/` — ten fail, the
  opaque-journal test passes on both (it guards what held); `just check` (221).
- **Result:** in review; M5 closes when it merges.
- **Learned:** every finding survived the 200-case proptest because its generator only ever
  answered with text. A generator must produce what the world produces — empty replies, cut
  replies — or it proves the easy path many times.

---

### [2026-10-24T10:08:16+02:00] rimuru — the attic: what commands take away comes back
- **Task:** a side review: deletes inside the project ran without a question and with no undo;
  the book promised an attic that did not exist. Checked: true — and worse, `rm -rf .git` was a
  plain write, and the tripwire put back only `.git/config`, `hooks` and `info`: a command could
  erase the repository's history. The book also placed the attic in `.mnemorph/attic/`, inside
  the world a command can write.
- **Files:** crates/mnemorph-tools/src/attic.rs (a git object store outside the world —
  snapshots before every command and file write, `deleted_since`, `restore`; `.git` objects
  hard-linked, refs copied, `repair_git`), shell.rs (`admit` split out; the attic's report),
  files.rs (write and edit snapshot first), tool.rs (`with_attic`), classify/paths.rs
  (deleting `.git` or the records is destructive), mnemorph-loop solo.rs (solo keeps an attic);
  tests/attic.rs (5), corpus rows; docs (05-architecture attic row and section, 02-races write,
  roadmap).
- **Gate:** n/a. Instruments: `rm -rf src` reported and restored; a script's `rm -rf .git`
  undone with history intact; an overwritten file restored; `just check` (226).
- **Result:** in review as a pull request.
- **Learned:**
  - A safety net inside the space it protects is not one: the attic lives in the state
    directory, where the guard admits no command.
  - Hard links are the cheap way to keep immutable files: `rm -rf .git` unlinks names, not the
    objects the attic also names.
  - `solo()` attached the real attic in tests and left eight attics in `~/.local/state`; every
    suite now gives its runs a private one, and the leak was removed.

---

### [2026-10-24T10:14:15+02:00] rimuru — the attic prunes itself
- **Task:** a side review: the attic only ever grew. Checked: true — snapshots were loose,
  unreferenced trees, and the kept `.git` objects outlived the world's own pruning.
- **Files:** crates/mnemorph-tools/src/attic.rs (a ref per snapshot; `prune`: the keep window,
  then the size cap, `git gc`, old `.git` copies forgotten; at most once a day; `with_limits`),
  mnemorph-config (`attic.keep_days`, `attic.max_mb`, `attic_limits`), tests (4),
  docs/05-architecture.md.
- **Gate:** n/a. Instruments: on this repository the first snapshot took 1.8 s and ~13 MB
  (9 MB packed), an unchanged one 24 ms and nothing; `just check` (230).
- **Result:** in review as a pull request.
- **Learned:** the measurement found the cap's own bug — `du` counts hard links, so a cap over
  the whole attic would have charged the world's repository to it and dropped snapshots for
  nothing. The cap measures the snapshot store alone.

---

### [2026-10-24T10:24:34+02:00] rimuru — M5b: Esc and the clock reach a running command
- **Task:** #78 — a long command (or a gate's verify lines) blocked Esc and the clock until it
  finished.
- **Files:** crates/mnemorph-tools/src/tool.rs (`with_cancel`, `set_deadline`), shell.rs (the
  wait loop stops the group on the cancel flag or the run's deadline; output so far kept),
  crates/mnemorph-loop/src/run.rs (the interrupt's flag is the tools' cancel flag; the clock is
  set before each call; an Esc that stopped a call stops the rest), tests (4),
  docs/03-loop.md.
- **Gate:** n/a. Instruments: `sleep 30` stopped by Esc and by the clock in well under a second;
  `just check` (234).
- **Result:** in review as a pull request.
- **Learned:** no thread rework was needed: bash already polled its child every 10 ms, so the
  same atomic the interface fires on Esc, read in that poll, is the whole mechanism.

---

### [2026-10-24T10:30:37+02:00] rimuru — M5b: replay compaction and the absolute stress line
- **Task:** long runs stop re-sending their history; stress becomes an absolute, configurable
  token line with its acts.
- **Files:** crates/mnemorph-loop/src/compact.rs (`fold`, `fold_point`), src/run.rs
  (`stress_tokens`; `ease_context`: fold in one batch at 60% of the line, tell the body to write
  down what is not durable at 75%, checkpoint at the line), mnemorph-config
  (`stress.context_tokens`), tests/compact.rs (the exit run; what folding keeps),
  docs/03-loop.md.
- **Gate:** n/a. Instruments: a 40-step run whose model reports what it was sent: 41 requests,
  the largest 53 KB against an 80 KB line (unfolded, the history would be ~150 KB); finished
  instead of checkpointing. `just check` (237).
- **Result:** in review as a pull request.
- **Learned:**
  - Folding a little each turn would move the cached prefix every turn and cost more than it
    saves; one batch at a threshold keeps the prefix still between folds.
  - The exit run first ended in `stall: actBudget` — forty turns that only read are the thrash
    the guard exists for. A real long run lands work as it goes; the script now does too.
  - The fresh-body act needs dispatch (M7); until then the line's act is to tell the human.

---

### [2026-10-24T10:36:03+02:00] rimuru — M5b: the loop breaker, the bench tally, the prefix ceiling
- **Task:** M5b's last rung: the identical-call loop breaker and the per-bench tally (#40, the
  lessons), and a CI ceiling on the cached prefix (#43).
- **Files:** crates/mnemorph-loop/src/stress.rs (`act(call, command, outcome)`: the loop
  warning at 3 and stall at 5; `Bench`, `bench_of`, the tally and its stall at 12 visits with no
  green), src/run.rs (passes the call and its bash command), tests (4), docs/03-loop.md.
- **Gate:** n/a. Instruments: the solo prefix measures ≈1137 tokens (ceiling 4000);
  `just check` (241).
- **Result:** in review as a pull request.
- **Learned:** the bench's key must be the bench, not the command line: `pytest -q` and
  `pytest -q | tail -1` first counted as two benches, each under the limit — a pipe split the
  tally it was meant to make suspicious.

---

### [2026-10-24T10:59:25+02:00] rimuru — M5b review fixes
- **Task:** the fresh Fable review that closes M5b — 13 findings, all addressed.
- **Files:** crates/mnemorph-tools/src/attic.rs (own excludes and attributes, an 8 MiB file
  ceiling named in the result, what became ignored leaves the index, `-z` paths, forget only
  what the world dropped, the snapshot log trimmed), classify/paths.rs (writing `.git` or the
  records in place asks; `intel` stays writable), shell.rs (an approved destructive act is not
  undone; the true kill time), tripwire.rs (a deleted repository is released), loop run.rs (Esc
  during verify is not a verdict; the clock set per ask), stress.rs (the bench key and muting),
  solo.rs (`configure`), tests, docs 03 and 05.
- **Gate:** n/a. Instruments: `just check` (246).
- **Result:** in review as a pull request.
- **Learned:**
  - A hard link keeps a file through a deletion, not through a write: "immutable" was a
    property of git's habits, not of the inode.
  - A safety net that ignores the human's approval is a second, louder refusal: the attic and
    the tripwire both had to learn to let an approved act stand.
  - The tripwire's tests went through the classifier's front door; once that door asks, they
    belong in a script, the only way a write reaches the tripwire in earnest.

---

### [2026-10-24T11:48:21+02:00] rimuru — Oni envelopes bounded: signed, expiring, capped, revocable (#88)
- **Task:** Veldora: "start on #88". The book promised envelopes "signed, scoped, expiring
  (default 30 days)"; `Envelope` held only a name and patterns. Nothing honours envelopes yet
  (M11), so this lands before the first reader.
- **Files:** crates/mnemorph-config/src/lib.rs (`signed`, `expires`, `uses`, `revoked`;
  `ENVELOPE_DAYS`; `Lapse`; `Envelope::admits(today, used)`; `revoke_envelope`), src/write.rs
  (`revoke`, once per envelope, one checked save shared with `set`), tests/config.rs (5 new), Cargo.toml (reads
  the ledger); crates/mnemorph-ledger/src/clock.rs (`Day::new`, `Day::plus`);
  tools/check-layers.mjs and docs/05-architecture.md (the config → ledger edge, bounded
  envelopes), docs/02-races.md.
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review as a pull request. Still open on #88: the ledger's use rows and their
  count, the gate reading `admits` (M11), `status` listing envelopes (M6), the revoke command,
  suspension on an observed mismatch (#93).
- **Learned:**
  - A promise in the book with no field in the struct is a hole only a reader finds; the
    research pass found this one by reading the struct against the page.
  - The calendar already lived in the ledger; drawing one edge kept a single calendar instead of
    a second copy of Hinnant's conversion.
  - An envelope is checked at the moment of use, with the ledger's count, so a cap or an expiry
    passed between plan and act still stops it.
  - The fresh Fable review found what the tests did not: a revoke the loader would then refuse
    (an envelope signed ahead, withdrawn early) bricked every later load, and two envelopes under
    one name left the second unrevocable. A write must leave a file its own reader accepts, not
    only one TOML parses.

---

### [2026-10-24T12:48:19+02:00] rimuru — the conversation is cached
- **Task:** a side note found long runs paying full price for their whole history every turn:
  only the system prompt and the tools were marked for the cache. Make it smarter.
- **Files:** crates/mnemorph-loop/src/compact.rs (`mark_cache`: the conversation's tail and the
  previous request's tail, in the slots the stable tiers leave; `foldable`), src/run.rs (marks
  every request; folds only when a fold takes out a fifth of the line), mnemorph-provider
  openai.rs (Claude behind a gateway gets the automatic breakpoint), tests (a model that caches
  as Anthropic does; the gateway mark; Esc tests that fire once the command runs), docs/03-loop.md,
  docs/06-consumption.md (the layout gives breakpoint 4 to the conversation). `docs/study-maki.md`
  untracked again (a merge of an old branch had brought it back; still gitignored, kept local).
- **Gate:** n/a. Instruments: the 40-step run, billed as the cache would bill it: 32% of its
  uncached input. `just check` (253).
- **Result:** in review as a pull request.
- **Learned:**
  - Compaction and caching pull against each other: after 60% the fold point moved every turn,
    so the run rewrote its history each turn — harmless while nothing was cached, a full-price
    write per turn once it is. A fold has to be worth the rewrite it causes.
  - The planned four static breakpoints would have left the conversation, most of a long run's
    input, with none.
  - Two Esc tests fired on a timer; under load the timer beat the ask's stale-Esc reset and the
    Esc was swallowed. A test should fire on the event it means, not on a guess at when it comes.

---

### [2026-10-24T15:40:00+02:00] rimuru — the book: memory (page 8), M8 replanned around it
- **Task:** Veldora: use DIKWP MNEMORPH-85 as Mnemorph's memory system — is it possible, and the
  plan, in the book.
- **Files:** docs/08-memory.md (new: archive and traces, recall with priors, a dissent seat and
  declared omissions, the write rule, reconsolidation, dissent-exempt distillation, three
  pathologies), docs/roadmap.md (M8 · L, seven rungs, a measurable exit), docs/README.md,
  docs/03-loop.md, docs/05-architecture.md, site (math on, page in the sidebar,
  markdown-it-mathjax3).
- **Gate:** n/a. A fresh Fable review: 16 findings, all fixed here (a no-write pass is not a
  check; one revise event per run-ending failure; 04's `@U` landing; fold order and precedence;
  proofs the fold can resolve; saturating BM25; stated defaults; a measurable exit). Instruments:
  diagram check 17 parse; site build green; 0 MathJax errors; `--frozen-lockfile` install clean;
  xmldom pinned past its deprecated releases.
- **Result:** in review as a pull request.
- **Learned:**
  - The rules port; the code does not: MNEMORPH-85 is Python with token-overlap recall and
    concatenation as synthesis. Its rules (model output never writes memory, a seat for dissent,
    correction priors, replay detection, dissent-exempt summaries) fit M8 with BM25 underneath.
  - Mnemorph already owns the hard part MNEMORPH-85 leaves abstract: the "observed world effect"
    is the verify line and the end gate.
  - "Proposed until checked" alone would have kept research and failed runs' lessons private
    forever (a side agent's catch); scope (personal · shared) is now its own axis, with four
    routes to shared: checked, accepted, corroborated, confirmed.
  - Open (@?): the weights are untuned until the recall fixtures exist; the corroboration
    match (what makes two claims "the same") needs its own fixture.

---

### [2026-10-24T17:10:00+02:00] rimuru — the book: the ontology (page 9) and its research page; M7's ontology rungs
- **Task:** Veldora: plan the DIKWP-MESH integration into onto; a research doc with the math and
  graphs; the book page; older pages updated.
- **Files:** docs/09-ontology.md (new: the decisive core — graph, inference to a fixed point,
  shapes, ownership as one ordered function; the plural layer — territory facts with `by`,
  source, counters and settles, gluing across a dimension, relations learned by sightings;
  coded findings with severities), docs/research/dikwp-mesh.md (new), docs/roadmap.md (M7: five
  ontology rungs and exit; M8's intro aligned with the promotion routes), docs/02-races.md,
  docs/04-communication.md (`@U territory <about>` and its counter form), docs/05-architecture.md,
  docs/08-memory.md, docs/lessons.md (one ownership function), docs/README.md, site sidebar,
  tools/check-diagrams.mjs (now reads docs/ recursively).
- **Gate:** n/a. A fresh Fable review: 17 findings, all fixed here (rule 6 was rule 5 again —
  now the Orc's domain, as guards resolves it; fact statuses rebuilt so more counters never mean
  less contested; a synonym check that can fire; obstruction defined on inverse-read sets; the
  ontology rule placed inside *right author*; seat, tie and retirement rules; a false router claim
  and a misplaced bracket in the research math). Instruments: diagram check 21 parse (18 before
  the check read research/); site build green; 0 MathJax errors on pages 8, 9 and the research
  page.
- **Result:** in review as a pull request, stacked on the memory one.
- **Learned:**
  - DIKWP-MESH cannot own ownership: it keeps many answers on purpose, and ownership must have
    one. Its rules fit the plural layer, which had no conflict model at all.
  - Its "gluing" is a per-tag counting rule with sheaf vocabulary; the thresholds (one dissenter
    in three tolerated, two against two never) carry over to creatures as they are.
  - The diagram check skipped subfolders: a book that grows a folder must widen its checks.
  - Open (@?): claims from the structured Territory field only (prose never claims) differs from
    isekai's prose lane — a decision for the human.

---

### [2026-10-24T19:20:00+02:00] rimuru — the book: formulas GitHub can read; facts that contradict without anyone flagging them
- **Task:** Veldora: "issues on some md files" (a formula pasted back broken from GitHub); a side
  agent's catch that two facts contradicted only when one body named the other's id.
- **Files:** docs/08-memory.md, docs/09-ontology.md, docs/research/dikwp-mesh.md (every display
  formula a ```math fence; inline `\{ \} \, \|` rewritten as `\lbrace \rbrace \ \Vert`),
  site/.vitepress/config.ts (math fences rendered through MathJax's block renderer);
  docs/09-ontology.md (*Catching what nobody flags*: keyed facts as functional properties, anchors
  whose digest drifts, the parent's triage on collision; `stale-fact`, `untriaged-fact`),
  docs/04-communication.md (`@U territory <about> <key>=<value>`, the `@V` triage labels),
  docs/research/dikwp-mesh.md, docs/roadmap.md (M7 rung 4 and exit).
- **Gate:** n/a. Instruments: diagram check 21 parse; site build green; every math span on pages
  8, 9, the research page and the roadmap converted by MathJax with 0 failures; GitHub's markdown
  API keeps `\,` and the `+` lines inside the fences.
- **Result:** in review on both pull requests.
- **Learned:**
  - GitHub parses a `$$` block as Markdown before the math renderer sees it: `\,` loses its
    backslash and a line starting with `+` becomes a list. A ```math fence is left alone.
  - Edits made through ordinary Python strings turned `\t`, `\b` and `\r` into a tab, a backspace
    and a line break inside formulas; MathJax read the first two as spaces and rendered nonsense
    without an error. Raw strings, and a per-span MathJax check, catch it.
  - DIKWP-MESH could count stances because its demo data had them filled in by hand; agents do
    not volunteer disagreement, so the ontology derives counters itself.
### [2026-10-24T12:40:00+02:00] rimuru — an envelope's longest life is the human's to set (#88)
- **Task:** Veldora: a cap of 30 days, "i want a cap configurable". After #108 an explicit
  `expires` could be any date after `signed`, so `expires = 2099-01-01` lasted for decades.
- **Files:** crates/mnemorph-config/src/lib.rs (`oni.envelope_days`, 1–365, loosening, so only
  the local or global file sets it; `ENVELOPE_DAYS_CEILING`; `Config::envelope_days`; an
  envelope past its limit is an error naming the file), tests/config.rs (1 new);
  docs/05-architecture.md, docs/02-races.md, docs/roadmap.md (what M11 still owes envelopes).
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - A default without a ceiling bounds only the envelopes nobody thought about; the one written
    by hand was the one that could last forever.
  - The limit is a loosening key: raising it is the same act as signing a longer envelope, so it
    lives where envelopes live. A compiled ceiling of a year keeps any setting from bringing back
    an envelope that never ends.

---

### [2026-10-24T13:06:19+02:00] rimuru — System One decisions, with an LLM fallback
- **Task:** Veldora's ask, from the research: make Jev-like models (TypeSafe's Jev, the open
  jeff) a decision source switched on by config, falling back to the LLM when none is set. A
  side note sharpened it: an LLM's stated probability is not a calibrated one, so the answer must
  say who gave it and cut-offs must differ by backend.
- **Files:** crates/mnemorph-provider/src/systemone.rs (the shared `/v1/systemone` wire: noul,
  choice, score; order-preserving encoding; reading against the ask; the ladder on a decision's
  scale), src/decide.rs (`Decider`: System One first, the LLM through an `answer` tool; `Decision`
  with its `Source` and `Basis`; per-basis `Cutoffs`; the journal row), mnemorph-ledger row.rs
  (`DecisionRow`), mnemorph-config (`decide.via`, `decide.model`, `decide.llm`,
  `decide.cutoff.measured`, `decide.cutoff.stated`; `Config::decide`; `kind = "systemone"`),
  tests (TypeSafe's recorded exchange, jeff's 529, the fallback and its reason, the same 85% sure
  as measured and not as stated, a tracked file that cannot add a provider), docs/05-architecture.md
  (*System One decisions*), clippy.toml (TypeSafe is a name).
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review as a pull request. No decision point calls it yet: each attaches with its
  milestone.
- **Learned:**
  - Jeff answers Jev's protocol on the same path with the same shapes, so one client serves both;
    jeff adds `orders`, drops `box`, rejects unknown fields and answers 529 when busy.
  - A JSON map sorts option labels, and a System One model may read options by position: the
    request is written in the caller's order by hand.
  - One answer shape for two backends hides the switch. The basis, not the shape, says what a
    probability is worth.
  - A fresh Fable review, ten findings, all fixed: a negative cut-off made every answer sure; a
    tracked file could still send the state to the human's own provider, or lower the bar
    (`decide.via` and the cut-offs are now the human's alone); a measured answer of the wrong
    shape was repaired into one that passed for measured (now refused, and the LLM answers);
    the text parser took the first `{` to the last `}`; escaping the state changed it (now it
    goes as it is, between markers made for the call); no id joined a decision to its outcome
    (now `DecisionRow.id` and `OutcomeRow`); `jev-latest` was assumed for jeff (`decide.model` is
    required with `via`).

---

### [2026-10-24T14:10:00+02:00] rimuru — the first decision point: a piped test run, read
- **Task:** Veldora, after a side note that `decide.via` changed nothing: "can you implement it?"
  The piped bench was the point already in the loop's code. The same turn sent a Jev key, so the
  wire was also proven live.
- **Files:** crates/mnemorph-loop/src/decisions.rs (`decisions` from the config, the fallback
  through the run's own model; `bench_ask`, the output's tail; `bench_read`), src/run.rs
  (`with_decisions`; the act step waits on a decision for a piped bench, Esc cancels it; a note
  in the beat journal; the decision row), src/stress.rs (`act_read`, `Read`, `Bench.read`;
  `muted` public), mnemorph-config (`decide.bench`, on by default), tests/decisions.rs (a sure
  green unstalls the run that stalled; an unsure stated read stays muted; a seen status asks
  nothing; the config builds it), mnemorph-provider tests/live_systemone.rs (one real decision,
  opt-in), docs/03-loop.md, docs/05-architecture.md.
- **Gate:** n/a. Instruments: `just check`; the live test once against TypeSafe: `jev-1.14.0`
  read a two-failure pytest log red at 1.00 and "every test passed" at 0.01, measured, 350 ms,
  392 input tokens, the strict reading satisfied.
- **Result:** in review as a pull request.
- **Learned:**
  - Jev's real answers sum to exactly 1 over exactly the asked labels: the strict reading the
    review asked for holds against the live service.
  - A decision point belongs where the instrument already counts: the bench tally takes a read
    as if the exit status had been seen, so no new stall rule was needed.
  - A fresh Fable review, nine findings. Fixed: an Esc that stopped the command was followed by
    a decision it could not cancel (the flag was already taken); a decision ran outside the
    run's clock (now the least of 30 s and what the run has left); the shell keeps the first
    30 000 bytes, so a long log's "tail" was its middle, and a read green there would reset the
    one counter made for thrash (now cut output is never read); default-on asked an LLM on every
    piped run, unpriced (now on only with `via` or `decide.bench = true`, and priced); the docs
    called the point live, which no binary yet makes true. Kept as a warning, not a note: a
    `Beat::Note` is replayed to the model on resume.

---

### [2026-10-24T13:28:29+02:00] rimuru — no merge left half-done
- **Task:** the review of #112 found raw conflict markers in `docs/05-architecture.md` on main
  (the `[oni]` block of #110 against the System One config blocks), merged through a green CI.
- **Files:** docs/05-architecture.md (both sides kept, `[oni]` beside its envelopes), justfile
  (`docs` fails on a conflict marker in any tracked file), docs/lessons.md (Process row).
- **Gate:** n/a. Instruments: `just docs` passes on the fixed tree and fails on a planted marker.
- **Result:** in review as a pull request.
- **Learned:** a green check is only as wide as what it reads: the site build parses markdown
  and checks links, and a conflict marker is valid markdown.

---

### [2026-10-24T13:44:06+02:00] rimuru — a seen failure is never read green
- **Task:** the review of #113 (merged): a piped bench run was read by the decider even when the
  shell saw it fail, so a timed-out or non-zero run could count green; the cut-output check missed
  stderr's marker.
- **Files:** crates/mnemorph-loop/src/run.rs (no read when the outcome is an error; both dropped
  markers), src/decisions.rs (the green label says the run finished), tests/decisions.rs
  (`a_failed_run_is_never_read_green`), docs/03-loop.md, docs/06-consumption.md (decision rows
  in the usage ledger), docs/lessons.md (the bench row's guarantee, as it now holds).
- **Gate:** n/a. Instruments: the new test fails with the fix removed and passes with it;
  `just check`.
- **Result:** in review as a pull request.
- **Learned:** the #113 entry's Files line calls `decide.bench` on by default; it is off unless
  the human names a System One provider or sets it (its own Learned bullet says so).

---

### [2026-10-24T14:55:00+02:00] rimuru — the offices named, and Metis routes within their bands
- **Task:** Veldora: "a jev usecase: which model to choose based on complexity, the courts, the
  offices (ciel, raphael, great-sage); find other names meaningful for mnemorph", then "go on" to
  the proposal: the three first Muses for the offices, Metis for the router, routing off by
  default.
- **Files:** mnemorph-wire commission.rs (`Office::name` mneme/melete/aoede, `Office::function`
  read/judge/write, `Office::ALL`), mnemorph-config (an office key names a Muse or a function,
  anything else is an error; `model_pinned`, `tier_model`; `decide.route`), mnemorph-loop
  route.rs (`band`, `route_ask`, `pick`, `route`; journaled at `model.route`), decisions.rs
  (`bench` and `route` switches; `decision_usage`, shared with the bench), tests/route.rs, the
  live test (`two_real_routes`), docs/02-races.md (*The offices*, *Metis routes within a band*),
  docs/05-architecture.md.
- **Gate:** n/a. Instruments: `just check`; live against TypeSafe: a rename read routine at 1.00
  and hard-to-undo at 0.06; a production payments migration hard at 0.98 and hard-to-undo at 0.97.
- **Result:** in review as a pull request. Honoured once dispatch builds bodies from commissions.
- **Learned:**
  - The canon's model table was already a complexity rule in words — Great Sage fast, Raphael
    bumped for what is hard to undo, Ciel never below Raphael — so the router is the canon made
    measurable, bounded by the floors the canon set.
  - The office key never validated its office word: `models.office.orc.verdict` (the `@ASK` word,
    not the office) configured a model nobody would ever ride.
  - A fresh Fable review, ten findings, all fixed. The worst: a sure "routine" beside an
    unresolved risk lowered a body (74% routine and 79% hard-to-undo went to fast); lowering now
    needs both, surely routine and surely safe. The throne was routable; it is not. A cloned
    repository could turn routing on, which moves spend; `decide.route` is now the human's alone.
    A route ignored Esc and was journaled but never priced; it now takes a stop and a meter. One
    office could be named twice with different models; that is an error. A tier with no model
    could be picked; the default stands instead.

---

### [2026-10-24T15:27:51+02:00] rimuru — a revoked envelope does not break the load
- **Task:** #111 (from the review of #110): lowering `oni.envelope_days` failed the whole load
  over a revoked envelope kept on record, advising "sign it again".
- **Files:** crates/mnemorph-config/src/lib.rs (`revoked` read before the cap; a revoked record
  is not judged by it; the error names revoke and delete), tests/config.rs (lowered over a
  revoked record; 365 accepted from the human's file), docs/05-architecture.md (the loosening
  keys in full; revoked records; UTC days; a local list replaces the global one),
  docs/02-races.md (status listing and time-of-use checks are M11's).
- **Gate:** n/a. Instruments: the new case fails with the fix removed; `just check`.
- **Result:** in review as a pull request.
- **Learned:** an expired record still fails the load past a lowered limit: config has no clock,
  and reading one there would make loading depend on the day. The error now says to delete it.

---

### [2026-10-24T15:36:48+02:00] rimuru — page 8, as its review asked
- **Task:** the review of #87: what grounds a note, a relevance floor nothing could undercut,
  machine-local proofs, δ as a joint probability, a gate that never reads a claim, a fold that
  could not demote, `c` defined on fixtures, BM25 "already" there, and the port's details.
- **Files:** docs/08-memory.md (the rule scoped to what stands behind a note; θ on the meaning term;
  tracked proofs, "unresolved here" apart from unproven; latest event wins; δ conditional and per
  route, accepted and corroborated at a lower κ; claims about the diff apart from claims about the
  world; per-link weights; the port's numbers as MNEMORPH-85 has them), docs/roadmap.md (a route's
  proof grounds; the exit as an exact count), crates/mnemorph-memory (personal, not long).
- **Gate:** n/a. Instruments: `just docs`.
- **Result:** in review as a pull request.
- **Learned:** a floor applied to a sum of priors and a match measures the priors; it has to sit
  on the term it means to bound.
### [2026-10-24T15:33:25+02:00] rimuru — decisions bounded, and confident the provider's way
- **Task:** #115 (from the review of #112): the decider sent any size of state, ignored the
  provider's own confidence, and its score confidence was blind to adjacency; a dead model-id
  constant was left behind.
- **Files:** crates/mnemorph-provider/src/systemone.rs (`MAX_STATE_BYTES`; `Answer.ordered` and
  `.measured`; `confidence()` prefers the provider's measure, else TypeSafe's choice and score
  definitions; `DEFAULT_MODEL` gone), src/decide.rs (LLM score answers are ordered), tests (the
  score's adjacency, the provider's measure winning, an oversized state refused before sending),
  docs/05-architecture.md.
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - Both decision points already bound and priced what they send (12 000 and 8 000 bytes); the
    cap in `Ask::check` is the floor every later point stands on.
  - A reviewer's worktree built `origin/main` into the shared target dir: cargo leaves a workspace
    crate's path out of its hash, so its artifacts replaced this branch's and looked fresh, and
    tests compiled against the old library. Touching the sources rebuilt them. Worktrees that
    build need their own target dir, or a rebuild after.

---

### [2026-10-24T15:39:09+02:00] rimuru — page 9, as its review asked
- **Task:** the review of #89: ownership read as the right to write, a finding that blocked its own
  fix, an undefined Domain field, a policy that cannot call `own`, a threshold borrowed as evidence,
  `@ONTO` with no budget or ids, root files that could not be a fact's subject, a rung with no
  consumer yet.
- **Files:** docs/09-ontology.md (right author needs the race to author; the policy gets the soil
  as prefixes with exclusions; findings on the tree the landing leaves; 0.35 as Mnemorph's own,
  untuned; countered's consequence; `./README.md`; the synonym check from three pairs),
  docs/02-races.md (the Orc's Domain field), docs/04-communication.md (`@ONTO`'s budget and ids),
  docs/roadmap.md (relations between worlds move to M13, where a second world is in reach),
  docs/lessons.md (the ownership row as isekai's code shows it), docs/research/dikwp-mesh.md.
- **Gate:** n/a. Instruments: `just docs`.
- **Result:** in review as a pull request.
- **Learned:** the thresholds do not carry over to creatures as they are: with three voices a
  ratio is a count, and the number is a choice to tune, not a finding to inherit.

---

### [2026-10-24T15:48:56+02:00] rimuru — Metis, after its review
- **Task:** the review of #117 (merged): sound; its follow-ups.
- **Files:** docs/02-races.md and mnemorph-wire's `Office` (the Muses as Pausanias names them;
  Metis, wise counsel), docs/03-loop.md (bench reads need `decide.bench`), docs/05-architecture.md
  (the tracked file may name models, never providers, and why), docs/roadmap.md (M7 books a
  route's cost against its commission).
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - Metis read a split score (0.74 routine, 0.26 hard) as surely routine; the adjacency-aware
    score confidence of the decisions fix reads it 0.22, under the cut-off, so it stays at the
    default with no change here.
  - An Elf may be routed from frontier to fast for a routine, safe `findings` commission; that is
    the band table as written, left for the human to keep or close.

---

### [2026-10-24T16:01:50+02:00] rimuru — backends in config; decisions only with a classifier
- **Task:** the human: models through OpenRouter or a vLLM gateway with its key, the backend in
  config; one model for everything, changed by race and office; decisions a feature switched on
  and off, active only when a classifier such as Jev is configured, disabled otherwise.
- **Files:** crates/mnemorph-config/src/lib.rs (`models.provider`, `models.default`,
  `models.race.*`, `Config::backend`, `Backend`; `decide.enabled`, decisions off without a
  classifier with a hole saying why; both switches the human's files only), tests (the switch and
  the classifier; backends, one model, race and office, `provider:id`), mnemorph-loop's
  `decisions()` doc and test, docs/02-races.md, 03-loop.md, 05-architecture.md (backends, the
  example config, the decisions switch, the diagram).
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - The config had providers and models but nothing joining them: a model had no backend. A
    model now names its backend only by a provider's own name before a `:`, so ids that hold
    a `:` (`qwen3:32b`) stay whole.
  - `decide.bench = true` used to switch the LLM on as the decider; the LLM now only stands in
    for a classifier that failed.

---

### [2026-10-24T16:13:18+02:00] rimuru — backends, after their review
- **Task:** the review of #122: the decisions' fallback ignored the one model; a race's model was
  a pin that silenced Metis; a tracked model could name a backend; the docs claimed requests
  already flow; a repository's tier beat the human's one model; an empty id passed.
- **Files:** crates/mnemorph-config/src/lib.rs (`decide.llm` from `tier_model(Fast)`;
  `model_at`, a race's model as its default; the human's providers known before layering, a
  tracked `<provider>:<id>` a hole; human tiers over the one model over tracked tiers; empty ids
  refused), crates/mnemorph-loop/src/route.rs (`model_at`; a band with no room asks nothing),
  tests, docs/02-races.md, docs/05-architecture.md.
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review as a pull request.
- **Learned:** a protection on one key is only as wide as every other way to say the same thing:
  `models.provider` was the human's, and `models.default = "vllm:…"` said it again from the
  tracked file.

---

### [2026-10-24T16:59:14+02:00] rimuru — M6: the protocol served over stdio
- **Task:** M6's first rung: `mnemorph-server` speaking the client protocol over stdio, driving
  one solo session, and `mnemorphd` serving it.
- **Files:** crates/mnemorph-loop (the run's observer, `Seen`, one `spend` for a turn's and a
  decision's cost; `open` and `open_with`/`Bound`: a session from the world's configuration
  through the human's backend; `ScriptedModel::text`/`tool`), crates/mnemorph-server (the
  protocol's types; `serve`: one writer thread, one reader that answers the gate, one driver
  task; `gate/ask` with a 10-minute deny; replay since a `seq`), crates/mnemorphd (`--world`,
  help and version before any world), tests/stdio.rs (4, through real pipes),
  docs/03-loop.md, docs/04-communication.md.
- **Gate:** n/a. Instruments: `just check`; the pipe tests three times in a row.
- **Result:** in review as a pull request.
- **Learned:**
  - The beats were already the event stream: one observer at the journal's door gives a client
    everything the protocol lists, with no second account of what happened to drift.
  - `open_with` attached the human's attic through `solo()`; tests reached it only by luck, so the
    attic is now an explicit part of what a test binds.

---

### [2026-10-24T17:14:07+02:00] rimuru — the server, after its review
- **Task:** the review of #123: a waiting gate held the process after the client left, queued
  lines ran after `shutdown`, the process outlived `shutdown`, `seq` could reach the wire out of
  order, the `queued` flag raced the turn's end, notifications were answered and batches
  swallowed, a failed `initialize` used up the opener, and the diagram disagreed with the text.
- **Files:** crates/mnemorph-server/src/lib.rs (`close` and `release`: leaving or Esc denies
  every waiting act; `closed` checked under the lock `close` drains; the driver stops on close;
  the writer ends on a `None`; `seq`, replay and send under one lock; a count of outstanding
  lines; no answer without an id; batches and non-objects refused; the opener kept until it
  succeeds; replay says `oldest` and `complete`), tests/stdio.rs (6: leaving by `shutdown` and
  by EOF, Esc at the gate, queued replies and order, a retried `initialize`, the server ending
  with its input open), docs/04-communication.md.
- **Gate:** n/a. Instruments: the six tests three times in a row; `just check`.
- **Result:** in review as a pull request.
- **Learned:** a gate that blocks is a promise to answer it on every way out; the tests never
  left, so the leaving was never tested — every way a client goes is now a test.

---

### [2026-10-24T17:23:12+02:00] rimuru — M6: the protocol generated, the SDK
- **Task:** M6's second rung: `@mnemorph/protocol` generated from the Rust, `@mnemorph/sdk` over
  stdio.
- **Files:** crates/mnemorph-server/src/protocol.rs (`ts-rs` derives; counters as `number`;
  `Envelope` and the requests' params and results; a test holding the TS `PROTOCOL` to the
  core's), .cargo/config.toml (`TS_RS_EXPORT_DIR`, `.js` imports), packages/protocol
  (the generated types, `Arrived`, `eventType`), packages/sdk (`stdio`, `Client`, `RpcError`,
  tests in memory and against the real `mnemorphd`), justfile (`ts` builds the core first),
  biome.json (the generated files are not linted), docs/04-communication.md.
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - `ts-rs` writes `u64` as `bigint`, which JSON never delivers to JavaScript: every counter on
    the wire is typed `number`.
  - pnpm rewrote 4 975 lockfile lines — its quoting, not its content — to add one workspace
    link; the link went in by hand, six lines, and the frozen install accepts it.
  - A test that runs only when the binary exists runs nowhere unless something builds it.

---

### [2026-10-24T17:35:18+02:00] rimuru — the SDK, after its review
- **Task:** the review of #133: a write on a dead or ended stdin crashed the host; `'exit'`
  dropped the core's last lines; `reason` required in TS; an event with no `seq` disarmed the
  dedup; a removed Rust type left its TS behind.
- **Files:** packages/sdk/src/index.ts (stdin errors close the connection; `'close'` not
  `'exit'`; nothing written after `close` or `shutdown`; the first close reason kept; `lastSeq`
  reset for a new session; events with no `seq` ignored), protocol.rs (`reason` optional),
  justfile (the generated TS removed before it is regenerated), tests (a core that answers and
  exits, 50 times; a core with no input; a request after shutdown).
- **Gate:** n/a. Instruments: the SDK suite three times; the last-line test fails with `'exit'`
  put back; `just check`.
- **Result:** in review as a pull request.
- **Learned:** a child's `'exit'` is when it died, `'close'` is when it finished speaking; a
  client reads speech.

---

### [2026-10-24T17:49:51+02:00] rimuru — M6: the terminal client
- **Task:** M6's third rung: `mnemorph`, the terminal client — a session in Ink and `run`.
- **Files:** packages/cli (`args` — help and version first, arguments over stdin; `core` — which
  mnemorphd; `transcript` — the events folded into what the view shows; `editor` — the input,
  pure; `run` — one ask; `app.tsx` — the view; `cli.tsx`; tests for each, the view through a
  real `Client` over a fake core), packages/sdk (`onClosed`), tsup.config.ts (the workspace's
  packages bundled), docs/04-communication.md.
- **Gate:** n/a. Instruments: `just check`; a live `mnemorph run` through OpenRouter
  (`openai/gpt-4o-mini`, two turns, ~900 input tokens each) read a file with the `read` tool and
  answered; a live session driven through a pseudo-terminal streamed its answer, showed the
  status line and quit on Ctrl+C.
- **Result:** in review as a pull request.
- **Learned:**
  - The first live session lost its Enter: keys typed fast arrive as one input (`".\r"`), and
    the editor took the carriage return for a newline. Every test had sent Enter alone. A burst
    is now taken key by key, against an editor no render can make stale.
  - The first build crashed before `--version` could answer: the workspace's packages are
    TypeScript sources and were left outside the bundle.

---

### [2026-10-24T18:33:29+02:00] rimuru — the terminal client, after its review
- **Task:** the review of #134, demonstrated in a pty with a VT emulator: a tall streamed answer
  copied into scrollback 26 times, long lines wrapping with blank rows, every resize erasing the
  scrollback (`ESC[3J`), a core's death silent with exit 0, quitting held by a core that never
  answers, an abandoned approval trapping the input, emoji split by the cursor.
- **Files:** packages/cli/src/transcript.ts (finished paragraphs leave the live text; the turn's
  end clears an open approval), app.tsx (a prefix column beside wrapping text; the last column
  free; the live text cut to the rows left; the reprint after Ink takes the new width, through
  its writer, `2J` only; an abandoned approval denied; Ctrl+C at an approval denies and stops;
  quitting within a second; the core's closing reported), editor.ts (graphemes), cli.tsx (the
  reason on stderr, exit 1), run.ts (no doubled newline), core.ts (the checkout's build only
  from the checkout), packages/sdk (`Client.close`; a core that will not leave is killed),
  tests, docs/04-communication.md.
- **Gate:** n/a. Instruments: the reviewer's own scenarios rerun — one copy of a 40-line answer
  at 20 rows and no full clears, wrap correct at 80 and after a resize to 50, no `3J`, no broken
  emoji, exit 1 on a core's death, exit 0 within a second on a silent core; the CLI suite (34)
  three times; `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - A line exactly as wide as the terminal wraps its trailing space onto a row of its own;
    leaving the last column free is cheaper than measuring every line.
  - A reprint laid out in the same tick as the resize used the old width for its first item;
    waiting until Ink has taken the new width in fixed it.
  - Quitting is not done until the child is gone: the UI ended, the process stayed, held by a
    core that never left.

---

### [2026-10-24T18:41:22+02:00] rimuru — the caps enforced
- **Task:** checked how spend works rather than supposed: the price table knows only Anthropic's
  ids, so OpenRouter calls are `usd: null`; `budget.*_usd` were read by nothing; the run's
  weighted-token budget existed but nothing set it — a session had no limit at all, against
  06-consumption's caps.
- **Files:** crates/mnemorph-config (`budget.session|day|month` in wt, the human's files only,
  at least 1; `budget_wt`), crates/mnemorph-loop/src/budget.rs (`left`: the tightest of the
  session cap and what the world's usage ledger leaves of the UTC day and month), open.rs (the
  run opens with it), mnemorph-server (the hello's `budget`), packages/protocol (`Budget`),
  packages/cli (`left … wt (cap)` in the status line; a spent cap said at the start),
  docs/06-consumption.md, docs/04-communication.md; tests in config, loop, server and CLI.
- **Gate:** n/a. Instruments: a session with `budget.session = 1` stops at a checkpoint after
  its first call; `just check`. Live tests now run on OpenRouter's free models
  (`poolside/laguna-s-2.1:free` read a file and answered, $0).
- **Result:** in review as a pull request.
- **Learned:** the book promised caps, the config parsed dollar keys, and the run had a budget
  field — three pieces of a limit, none connected. Weighted tokens are the only unit every call
  reports, so the caps are in it.

---

### [2026-10-24T18:57:44+02:00] rimuru — the caps, after their review
- **Task:** the review of #135: two sessions on one world spent twice the day cap (the cap was
  read once, at open); an unreadable ledger read as nothing spent; the salvage call went
  unmetered; the docs promised a per-turn fold, an estimate and an 80% warning nothing did.
- **Files:** crates/mnemorph-loop/src/run.rs (`with_caps`; before every call the ledger is
  reread and `check_caps` — the ledger crate's own, dead until now — stops at a spent cap or
  warns once at 80%; an unreadable ledger stops; the salvage call metered; spend saturates),
  budget.rs (`left` per session id, saturating), open.rs (a capped session reads the ledger or
  does not open; unreadable lines reported), mnemorph-ledger caps.rs (saturating), config
  (a cloned repository neither raises nor sets a cap), packages/cli (`--theme` refuses words it
  does not know), docs/06-consumption.md, docs/04-communication.md, tests.
- **Gate:** n/a. Instruments: a second session on a world whose day is spent makes no call;
  the warning once; an unreadable ledger stops before any call; `just check`.
- **Result:** in review as a pull request.
- **Learned:** a cap read once is a quota for one session; a cap across sessions is a fold read
  again before each call. The ledger crate had the right fold all along, with nothing calling it.

---

### [2026-10-24T19:08:52+02:00] rimuru — M6: /send and /handoff
- **Task:** the polish rung's last pieces: the send lane and the handoff.
- **Files:** crates/mnemorph-loop/src/run.rs (`steer`: a line sent reaches the body after its
  next tool step, marked as the human's; one a turn never reached is said, never carried into the
  next; `handoff`: the template filled from the facts the core gathers, written to
  `.mnemorph/handoffs/<time>.md`; `latest_handoff`), session.rs, mnemorph-server (`session/send`
  — refused when nothing runs; `session/handoff {focus | read}` through the session's lane, in
  order; the `handoff` event; the hello's waiting handoff), packages/protocol (`Waiting`),
  packages/sdk (`send`, `handoff`, `readHandoff`), packages/cli (`/send`, `/handoff [focus]`,
  `/handoff read`; a waiting handoff said at the start; a checkpoint offers one; the core's own
  asks shown as the commands that asked them), docs/03-loop.md, docs/04-communication.md, tests.
- **Gate:** n/a. Instruments: a live `/handoff` on a free model (`poolside/laguna-s-2.1:free`)
  wrote the seven sections, and recorded that `git` failed in a world with no repository;
  `just check`.
- **Result:** in review as a pull request.
- **Learned:** the core's own asks are long templates; shown as the human's line, they buried
  the transcript. The transcript shows the command that asked.

---

### [2026-10-24T19:24:46+02:00] rimuru — /send and /handoff, after their review
- **Task:** the review of #136: `/handoff` refused after a checkpoint, where the CLI offers it;
  a late `/send` reported only at the next turn; `/handoff read` with nothing to read left the
  CLI busy; two handoffs in one second overwrote each other; the template quoted as the first
  ask; unbounded facts; age by mtime; the marker undefined to the body; the docs' `/send` form.
- **Files:** crates/mnemorph-loop/src/run.rs (a handoff passes the context line and a carved
  budget, never the caps, in at most 8 calls, after folding; the core's asks never the first;
  a late line said with its turn's end; names that never collide; age from the name; facts
  capped at 16 KiB; `ask_for_core`), solo.rs (the marker is the human's only as its own text),
  mnemorph-server (read refuses past 64 KiB and says what is unreadable; a queued handoff is
  `queued`), packages/cli (`/handoff read` not busy before it starts; the exact template
  sentence), docs/03-loop.md, docs/04-communication.md, tests.
- **Gate:** n/a. Instruments: a handoff written after a context checkpoint; a late line warned
  before its turn's end; two handoffs, two files; `just check`.
- **Result:** in review as a pull request.
- **Learned:** the save operation is used when the meters are full; a save gated by the same
  meters saves nothing. It passes the soft lines and stops at the human's.

---

### [2026-10-24T19:38:43+02:00] rimuru — the handoff removed
- **Task:** the human: "delete it, don't put handoff". Handoff notes were written into the
  world's tracked `.mnemorph/handoffs/`, carrying the session's first ask word for word, in a
  public repository.
- **Files:** crates/mnemorph-loop (the handoff, its templates, `latest_handoff`, the ask kinds and
  the first ask gone), mnemorph-server (`session/handoff`, the `handoff` event, the hello's
  waiting handoff gone), packages/protocol, packages/sdk, packages/cli (`/handoff` gone), the
  tests that held them, docs/03-loop.md (the section), 04, 05 (the storage row), 08, roadmap
  (M6 and M8's rung), lessons (the row becomes session start through the harness).
- **Gate:** n/a. Instruments: no handoff file was ever committed (`git log --all --
  .mnemorph/handoffs` is empty); `just check`.
- **Result:** in review as a pull request.
- **Learned:** a note meant for the next session, written into a tracked tree, is a note for
  every reader of the repository; what a session must carry over belongs where only the harness
  reads it. `/send` stays: it never touches a file.
