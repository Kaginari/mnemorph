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

---

### [2026-10-24T19:52:05+02:00] rimuru — M7: creatures as data, and ownership
- **Task:** M7's first rung: the world's creatures read from their docs, the chain of command,
  and ownership as one function (the ontology's rungs 1 and 2, before the gate calls it).
- **Files:** crates/mnemorph-world (doc.rs — the fields read as data, ground fields only lists of
  `/`-written prefixes; roster.rs — every doc, a fingerprint, the chain, `above`, loops; own.rs —
  harness soil, the law, scratch, seats, the compiled soils, the longest zone, the longest
  domain, else unowned), tests/own.rs (this repository's own world, and worlds made to break
  it), .mnemorph/orc/*, .mnemorph/kijin/ci (Domain and Post, as the races say),
  .mnemorph/*/workspace (a sentence for a territory, made a prefix), docs/09-ontology.md.
- **Gate:** n/a. Instruments: the world's 30 docs read with no problem; every owner the tests name
  matches the docs; `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - Two docs gave their ground as a sentence ("every file at the repository root, and tools/");
    a parser that read words would have claimed `every/` and `file/`. A claim is a prefix with
    its `/`, or the doc does not read.
  - `crates/mnemorph-config/` has no Slime and no Orc: unowned soil, found by the first owner query
    — the findings rung will report it.

---

### [2026-10-24T20:35:13+02:00] rimuru — M7 ownership: the review's fixes
- **Task:** fix what the Fable review of the first M7 rung found (verdict: merge after fixes).
- **Files:**
  - crates/mnemorph-world:
    - own.rs — paths read segment by segment; human files; evolution's staging; the plan
      exactly; the throne read from its doc;
    - doc.rs — file claims, a claim inside the world only, the header ending at any heading;
    - roster.rs — unreadable docs and race directories reported and fingerprinted;
    - tests/own.rs.
  - The world: .mnemorph/rimuru/rimuru (the throne's doc); orc-engine and orc-clients (the
    config crate and the theme package); the workspace (`.cargo/`, `.claude/`); kijin-ci
    (`.github/workflows/` only).
  - docs/09, 02, 05.
- **Gate:** n/a. Instruments: the three `..` paths the review used, now owned as written;
  `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - A path is input, not a fact. `own()` trusted `crates/mnemorph-wire/../../.mnemorph/law.md`
    to be the wire's because its first segments said so. Reading segment by segment is the one
    place this can be settled.
  - The throne was an owner by a hard-coded name and no doc. Every Elf reported to a creature
    the world could not read.
### [2026-10-24T20:29:24+02:00] rimuru — M6 closed: the review's fixes, and what the milestone cost
- **Task:** close M6 after its Fable milestone review (verdict: close after fixes).
- **Files:**
  - CLI: cli.tsx, run.ts — a core that will not start is let go;
  - app.tsx — the scrollback keeps the last column free; an approval chosen, then confirmed;
  - core.ts — the newer build wins;
  - transcript.ts — a call shows as it starts; no spend before the first call;
  - status.ts, args.ts, index.ts, tsup.config.ts — `mnemorph status`, the commit stamped.
  - Core:
    - mnemorphd build.rs and main.rs — the commit in the version;
    - loop open.rs and run.rs — a ledger line that cannot be read stops a capped session;
    - the key's origin and the sandbox in the hello;
    - protocol.rs, lib.rs.
  - Docs: 04, 06, roadmap (WebSocket moved to M12; `status` named in M6), lessons (the dispatch
    card's row to M7, the distributions row to M14), README.
- **Gate:** n/a. Instruments: each fix has a test that fails without it — the start failure
  against the real core (one core left running before), the scrollback width (101 cells in 100
  before), the approval typed into (`xyz` approved nothing), the torn ledger at open and before a
  call; `just check`.
- **Result:** in review as a pull request.
- **Usage** (`npx ccusage`, 2026-10-24, the whole day — M5b's end, M6, M7's first rung):
  - 1.11 B tokens, $473 at list prices:
    - opus-5-5: 1.07 B, $368;
    - fable-5-1: 45.7 M, $103;
    - opus-4-8: 0.7 M, $2.
  - The 10:00 and 15:00 windows reached 99.6% and 89.8% of the plan's five-hour limit.
  - Cache reads are 98% of the tokens.
- **Learned:**
  - A test of the width at one terminal size proves nothing: the overflow appears only when a
    line meets the edge exactly. Lines of every length are the test.
  - A test with a fake transport cannot see a process left running. Only the real core showed
    the leak.

---

### [2026-10-24T21:00:49+02:00] rimuru — a torn ledger line counted high, not a wall
- **Task:** the human's call. A damaged usage-ledger line must not block every capped session
  after a crash. Count it as hidden spend instead.
- **Files:**
  - crates/mnemorph-ledger caps.rs — `unseen_wt`, `UNSEEN_FLOOR_WT`, `check_caps` taking
    `unseen`;
  - crates/mnemorph-loop:
    - open.rs — the hole names the lines and their assumed weight;
    - run.rs — counts them before every call;
    - budget.rs;
  - tests in ledger, loop and server;
  - docs/06-consumption.md.
- **Gate:** n/a. Instruments: a torn line opens the session with a hole and 20 000 wt less left; a
  run under a cap at that weight stops before its call; `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - Fail-closed was right for a ledger that cannot be read at all, and wrong for one torn line.
    A crash is the common case, and a wall after every crash makes the cap the thing the human
    turns off. Counting high keeps the cap's promise without the wall.
  - A line that cannot be read cannot say its day or session, so it counts against all three caps.

---

### [2026-10-24T21:18:51+02:00] rimuru — unseen spend drawn from a hidden 15% reserve
- **Task:** the human's call. Counted alone, a torn ledger line could still block a cap under
  20 000 wt and quietly took 20 000 wt off every cap until repaired. Hold back 15% of each cap,
  out of sight, for spend the ledger cannot read.
- **Files:**
  - crates/mnemorph-ledger caps.rs — `UNSEEN_RESERVE_PCT` and `unseen_against`; `check_caps`
    adds unseen spend per cap, clamped;
  - crates/mnemorph-loop budget.rs and open.rs (the hole names the reserve);
  - tests;
  - docs/06-consumption.md.
- **Gate:** n/a. Instruments:
  - nine torn lines under a 100 000 cap still read Ok;
  - 70 000 of real spend plus the reserve warns at 85 000;
  - a 20 000 day cap with one torn line answers;
  - `just check`.
- **Result:** in review as a pull request.
- **Learned:** an assumed cost needs a ceiling as much as a floor. The floor keeps unseen spend
  from being skipped; the ceiling keeps it from becoming the wall it replaced.

---

### [2026-10-24T22:02:40+02:00] rimuru — M7: the closed graph, each body's soil, and onto owner
- **Task:** finish the ontology's rungs 1 and 2:
  - the graph inferred to its fixed point;
  - ownership with the rule that decided it;
  - the soil each body is handed;
  - `onto owner` for the human.
- **Files:**
  - crates/mnemorph-world:
    - graph.rs — `IsA`, `above` with its truth, verdict and reports sub-bonds, `below`, `owns`,
      `routes`, `posts`, `wears`, closed;
    - own.rs — `explain`, `Rule`, `Ownership`;
    - soil.rs — `Soil`, `soil`, `admits`;
    - tests/graph.rs and own.rs;
  - mnemorph-loop and mnemorph-server re-export the world;
  - mnemorphd — `onto owner`, tests/onto.rs;
  - packages/cli — args.ts and cli.tsx, `mnemorph onto`;
  - docs/09 and 04.
- **Gate:** n/a. Instruments:
  - every living creature's soil agrees with `own` on every tracked file, and on the greedy
    fixture;
  - `onto owner` run from the root and from a crate;
  - `just check`.
- **Result:** in review as a pull request.
- **Learned:** a soil's exclusions are found by asking `own` of every claim inside the body's,
  never by restating the rules. The two cannot drift: a test walks every tracked file and checks
  both give the same answer.

---

### [2026-10-24T22:16:47+02:00] rimuru — M7 graph and soil: the review's fixes
- **Task:** fix what the Fable review of the graph-and-soil rung found (verdict: merge after
  fixes).
- **Files:**
  - mnemorphd main.rs:
    - an existing directory is read as one, with or without its `/`;
    - the world's root is named as such;
    - `--version` answers first;
  - world:
    - soil.rs — builds its anchors from own.rs's constants;
    - own.rs — the empty scratch claim;
    - graph.rs — the closure joins by range instead of a full scan;
  - tests: the review's nested fixture, lifted; onto's directory, root and version cases;
  - docs/09.
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - The previous entry said this rung finished rungs 1 and 2. It delivers their functions only.
    The rebuild on the docs' fingerprint, the gate's *right author* and handing `soil` to the
    tool policy all wait for the dispatch rung that calls them.
  - The anchor list restated `own`'s soils. Only the filter asked `own`, so a new harness prefix
    could have been missed. Both now come from one list.

---

### [2026-10-24T22:24:49+02:00] rimuru — M7: the shapes as findings, and onto check
- **Task:** the ontology's rung 3: each shape the law demands, checked as a finding with a code
  and a severity.
- **Files:**
  - crates/mnemorph-world:
    - findings.rs — `overlap`, `above-cycle`, `dead-bond`, `slime-truth`, `unowned-soil`;
    - doc.rs — each field's line, for `file:line`;
    - tests/findings.rs;
  - mnemorphd — `onto check`, tests/onto.rs;
  - docs/09.
- **Gate:** n/a. Instruments:
  - a fixture holding every breach reports each with its code, severity and place;
  - this world passes, with two medium unowned-soil findings: `.github/`'s templates, which wait
    for a forge Kijin;
  - `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - Overlap is between write grounds, not between all claims. In this world every Slime's zone
    equals its Orc's domain and its Elf's lane, which is the hierarchy working. Read across all
    claims, the rule would block every landing.
  - The gate blocking on these findings, and the codes for facts, heirs, Minds and relations,
    come with the rungs that bring what they read.

---

### [2026-10-24T22:33:15+02:00] rimuru — M7 findings: the review's fixes
- **Task:** fix what the Fable review of the findings rung found (verdict: merge after fixes).
- **Files:**
  - findings.rs:
    - `unowned-soil` reads the owner, not the rule;
    - a retired parent is a `dead-bond`;
    - a loop through a retired creature is none;
  - mnemorphd — `git ls-files -z`;
  - tests: keeperless compiled soil, retired parents, a name git quotes;
  - docs/09's table.
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - A finding about ownership must ask who owns, not which rule answered. "No claim" and "a
    compiled soil whose keeper does not live" are the same ⊥. Before genesis, a world's root
    files are all unowned.
  - Findings and the graph must read the same bonds. The graph dropped a retired parent while
    the loop check still walked through it.

---

### [2026-10-24T22:56:24+02:00] rimuru — M7: dispatch, the world's half
- **Task:** the first dispatch rung. The races' law decides on five facts; the world now finds
  them from the creature docs and names the creature a refused order goes to.
- **Files:**
  - crates/mnemorph-world dispatch.rs — `dispatch`, `Ask`, `Hop`, `Refused`;
  - tests/dispatch.rs — this world's chain, a fixture for verify-first and the other lane;
  - docs/04 — the five facts and the refusal routes.
- **Gate:** n/a. Instruments: every refusal on this world's chain names a creature; `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - The law's words say what kind of act is lawful. Only the world knows to whom, and a reach
    downward is lawful one rank at a time. "Not one hop" from above a target names the next rank
    toward it, never the human.
  - The commission itself is the next rung: carving the slice, the fresh context, the report as
    the parent's tool result, and the gate's *right author* calling `own`.

---

### [2026-10-24T23:04:36+02:00] rimuru — M7 dispatch: the review's fixes
- **Task:** fix what the Fable review of dispatch's world half found (verdict: merge after
  fixes).
- **Files:**
  - dispatch.rs:
    - a route never names a retired creature;
    - the budget refusal is the wire's `Hole::Budget`, its commander in `whom`;
    - a target that reports across a skipped rank is said as such;
  - doc.rs — a blank verify line is none;
  - tests: a second Elf, sideways Orc to Orc, a retired commander, a blank verify, a skipped rank;
  - docs/04.
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - A refusal that names its route must name a creature who can take the order. The chain still
    holds retired creatures, so every named route is checked for life.
  - A string that looks like a wire line must parse as one. The budget hole is built from the
    wire's own type.

---

### [2026-10-24T23:15:19+02:00] rimuru — M7: the right author, and commands granted exactly their soil
- **Task:** the gate's half of ontology rung 2–3: *right author* and blocking findings. The human
  asked for "a smart strategy" against shell commands writing past a creature's soil.
- **Files:**
  - crates/mnemorph-tools tool.rs — `with_soil` (exclusions, root files), `admits` by the
    longest claim, `grants` (exactly the soil, part by part around another's spot, plus what git
    ignores), `scratch`;
  - shell.rs — the policy narrowed for a commissioned creature;
  - crates/mnemorph-loop:
    - gate.rs — `Author`, `right_author`, `touches`;
    - commission.rs — `context_for`;
  - tests: shell.rs under the real guard, loop tests/right_author.rs;
  - docs/03, 05.
- **Gate:** n/a. Instruments:
  - under landlock, a command writes its own file and a new one in a granted folder, reads the
    world, and is refused another's spot and a root file;
  - the gate names a stray write's owner and route;
  - an overlap blocks the landing it touches, and not the one that ends it;
  - `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - Landlock only adds. "A claim minus a spot" is granted by walking the claim and leaving the
    spot out. The cost: no new file directly beside the spot, only by the write tool.
  - Granting exactly the soil would have broken every `cargo test`: `target/` is no one's. What
    git ignores is never a landing, so it is granted.
  - The third layer, restoring what a failed right-author check names from the commission's
    attic snapshot, belongs to the commission runner (the next rung), where that snapshot is
    taken.

---

### [2026-10-24T23:31:59+02:00] rimuru — M7 grants: a link never grants its target
- **Task:** close the escape the review of #146 demonstrated before its report was cut off.
- **Files:** crates/mnemorph-tools tool.rs (`grants` resolves every grant, keeps only what stays
  the creature's own, excludes the records from build output; `scratch` refuses a linked nest),
  tests/shell.rs (links to another's spot, another's zone, a file outside the world); docs/05.
- **Gate:** n/a. Instruments:
  - the link test fails on the previous commit, where the write through `src/to-theirs`
    succeeded, and passes now;
  - `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - A grant list is a list of names, but the guard grants what they resolve to. Every name is
    resolved and judged by the same `admits` before the kernel sees it.
  - My own grant test checked the grants by their resolved names, while the escape lived in an
    unresolved one. A test of a border must try to cross it, not just read the list.

---

### [2026-10-24T23:48:37+02:00] rimuru — the border, reviewed twice: Opus, then Fable on Opus
- **Task:** close what the sandbox review of #146 found. Opus 5.5 reviewed the code; Fable
  reviewed that review, as the human now wants for sandbox work.
- **Files:**
  - tools tool.rs:
    - `with_build` (build output handed by the world);
    - `linked` (a claim reached through a link is none);
    - a private per-creature scratch;
    - `ignored` exported, its `git` told to run no fsmonitor or hooks;
  - loop commission.rs — build output decided with `own`;
  - sandbox:
    - lib.rs — rules by an owner-only file past 256;
    - mnemorph-guard — `--rules`;
    - linux.rs — `truncate` refused below ABI 3;
  - loop gate.rs — the snapshot records mode and ctime;
  - tests: shell.rs under the guard, right_author.rs;
  - docs/05 and 03.
- **Gate:** n/a. Instruments: a crossing test per finding, under the real guard where the kernel
  is the border; `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - "Ignored" is not "build output". `.env` beside another's code is ignored too. Only `own`
    decides what a creature may write, and git's list merely nominates.
  - Grants and ownership must judge the same paths. Ownership reads them as written, and the
    kernel resolves them, so a link anywhere is checked before it is followed.
  - Open for the commission rung: a creature's seat is in its soil, but the tripwire reverts any
    write under `.mnemorph/`. "Doc truthful" decides which of the two owns the seat. Commissions
    are not wired yet, so none of this border is reached in production.

---

### [2026-10-24T23:57:53+02:00] rimuru — the file tools isekai had: multiedit, move, delete
- **Task:** the human asked for isekai's harness-side file tools, so that the work a commissioned
  creature's commands cannot do in a folder holding another's spot (rename, replace, delete) has a
  lawful tool.
- **Files:**
  - crates/mnemorph-tools:
    - files.rs — `multiedit`, `move`, `delete`, and `replace` shared with `edit`;
    - beneath.rs — `remove`, `rename` with no-replace;
    - tool.rs — the registry;
    - classify — the in-place refusal names `multiedit`;
    - tests/files.rs;
  - crates/mnemorph-loop solo.rs — the prompt names the tools;
  - docs/05.
- **Gate:** n/a. Instruments:
  - multiedit writes all or nothing;
  - move works in a folder holding another's spot, takes a directory with its files, refuses an
    overwrite, the other's spot and the outside;
  - delete refuses without an attic and keeps a copy with one;
  - `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - isekai had no move tool; it left moves to `mv`. Under exact grants `mv` in a mixed folder is
    refused, so the tool is the only lawful road, and it checks every file a directory carries.
  - isekai's delete renamed into an attic inside the world. Mnemorph's attic lives outside it as
    a git store, so delete snapshots first and refuses when it cannot.
  - `patch` (a unified diff across files, for models trained on one) is the next tool: it
    deserves its own tests and review.

---

### [2026-10-25T00:11:17+02:00] rimuru — the file tools, reviewed twice: Opus, then Fable on Opus
- **Task:** close what the review of #147 found. Opus 5.5 reviewed the code; Fable reviewed that
  review and named what it missed.
- **Files:**
  - tools:
    - files.rs:
      - delete asks the attic whether it holds the file;
      - move and delete act on the entry, never a link's target;
      - a folder holding another's spot, at either end, is never moved whole, and nothing moves
        into itself;
      - an unreadable subdirectory refuses the move;
      - a move reports its files;
      - an empty directory may be deleted;
      - an unchanged write or multiedit writes nothing;
      - edits refuse non-UTF-8 text;
    - beneath.rs — a failed rename unmakes its directories;
    - tool.rs — `writable_entry`/`writable_at`, `holds_spot`, git's files refused;
    - attic.rs — `holds`;
  - tests;
  - docs/05.
- **Gate:** n/a. Instruments: a test for each finding; `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - "Kept a copy" is a claim: ask the store, not the snapshot's success. The attic leaves out
    what the world ignores, and a delete trusted it blindly.
  - A tool that acts on a name — move, delete — must not resolve the last component. A tool that
    acts on content — read, write, edit — must. The two need two checks.

---

### [2026-10-25T00:18:42+02:00] rimuru — patch: a unified diff, whole or not at all
- **Task:** the last of isekai's file tools, for models trained to write unified diffs (the human
  is away and asked that M7's open work be carried through).
- **Files:** crates/mnemorph-tools patch.rs (parse, apply with drift and a unique-match fallback,
  create, delete under the attic's copy, rename refused), the registry, tests/files.rs; the solo
  prompt; docs/05.
- **Gate:** n/a. Instruments:
  - a five-file diff with an off-by-lines hunk, a missing final newline, a create and a delete;
  - a shifted insertion;
  - one bad hunk writing nothing anywhere;
  - a rename, an outside path, prose, and a create over an existing file all refused;
  - `just check`.
- **Result:** in review as a pull request.
- **Learned:** isekai's patch left earlier files written when a later disk write failed. Here the
  attic's snapshot comes first, and the error names what already landed.

---

### [2026-10-25T00:36:59+02:00] rimuru — patch, reviewed twice: Opus, then Fable on Opus
- **Task:** close what the review of #148 found.
- **Files:** crates/mnemorph-tools/src/patch.rs, rewritten:
  - a header only with its `+++`;
  - trailing blank context dropped;
  - git-only blocks refused;
  - CRLF kept;
  - parts of one path chained;
  - no-ops refused;
  - every change carries its original, and a failed write unwinds them all;
  - routes in every refusal.

  Also tests/files.rs and docs/05.
- **Gate:** n/a. Instruments: a test per finding; `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - Fable corrected Opus on two counts. Hunk counts are a guide and not a gate: models get them
    wrong, and strict counts would refuse their diffs. And the rollback is the plan's own
    originals, not the attic, which may hold nothing for an ignored file.

---

### [2026-10-25T00:54:38+02:00] rimuru — M7: facts, the ontology's rung 4
- **Task:** territory facts as events, read by the status table; keyed facts countering by code;
  anchors that drift; triage candidates; the `@ONTO` projection; the three fact findings.
- **Files:**
  - crates/mnemorph-world facts.rs — `Event`, `Fact`, `Facts::fold/load/record`, `Status`,
    `fact_id`, `digest`, `from_unsaid`, `collisions`, `candidates` (BM25), `projection`;
  - findings.rs — `fact_findings`;
  - tests/facts.rs;
  - mnemorph-wire — `fnv64a` exported;
  - mnemorphd — `onto check` reads the facts;
  - docs/09.
- **Gate:** n/a. Instruments: the exit's fact cases.
  - different `build=` values with no one naming the other read contested;
  - a fact whose cited lines change reads stale;
  - a resembling free fact reaches the parent and, left unlabelled, is `untriaged-fact`;
  - also the status table, inheritance, labels merging, the projection's order and cut;
  - `just check`.
- **Result:** in review as a pull request.
- **Learned:** a fact's id is written to disk, so its hash must be stable everywhere. The wire's
  FNV-1a already was, and Rust's own hasher is not.
- **Open:** filing a child's `@U territory` lines as facts when its report lands (the
  commission, in review) comes as a small follow-up once the commission is merged.

---

### [2026-10-25T01:07:23+02:00] rimuru — facts, after their review
- **Task:** fix what the review of #150 found (verdict: merge after fixes).
- **Files:**
  - crates/mnemorph-world facts.rs:
    - `fact_path` and `fact_text` in every id; `THETA_T`;
    - merges resolved for later events;
    - labels keep a creature on one side;
    - a settle reopened by a later colliding assertion;
    - a withdrawn fact countered;
    - per-token keyed normalisation;
    - the projection marks stale, gives reasons, counts each cut fact once and leaves no dangle;
  - findings.rs — untriaged gated on candidates;
  - tests/facts.rs;
  - docs/09.
- **Gate:** n/a. Instruments: a test per finding; `just check`.
- **Result:** in review as a pull request.
- **Learned:** the exit's own case, `build=` on one path, slipped through on a trailing slash.
  Ownership needs `src/` to mean a directory, and a fact's path needs it to mean the same thing
  as `src`. Each reads the path for its own purpose.
### [2026-10-25T00:30:41+02:00] rimuru — M7: one order, down and back
- **Task:** the commission runner, the heart of M7's exit: "the full one-order-down-and-back
  sequence runs with scripted bodies and passes its gate".
- **Files:**
  - crates/mnemorph-loop:
    - commission.rs — `CommissionTool`, `Commissioner`, `order`, `plan_items`, `repair`, the
      child's prompt;
    - run.rs — the `commission` call run by the loop, `with_commissioner`, `courts`,
      `spent_wt`;
    - gate.rs — `courts`, `changed`, `Tree::has`;
  - crates/mnemorph-tools — `Context::inherit`, `Outcome::ok`/`error` public;
  - tests/commission.rs;
  - the EndGate literals gain `courts`;
  - docs/04.
- **Gate:** n/a. Instruments:
  - Rimuru → elf → orc → slime, the Slime landing under its gate, each report climbing as its
    dispatcher's tool result;
  - a reach past a rank refused naming the next rank;
  - a Slime's command off its ground put back after its gate refused it twice;
  - `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - A grandchild's landing is the child's court work, or every rank above it "repairs" it away.
    The first run removed the Slime's file at the Elf's level.
  - A child that may dispatch needs the commissioner too. Without it, the order stopped at the
    Orc on the tool's "cannot commission", and the Slime never ran.
  - Still to come: the queue and wake lanes, Courts with fresh contexts per office, the council
    and relief, facts (ontology rung 4), and `mnemorph q`. The human's session does not open
    Rimuru as a dispatcher yet; that wiring comes with the lanes.

---

### [2026-10-25T00:50:02+02:00] rimuru — the commission, reworked after its review
- **Task:** the review of #149 said rework. Its findings are fixed:
  - court work was exempt by path, so a dispatcher could rewrite it;
  - a restored stray failed every gate above;
  - Esc did not reach a child;
  - a body could commission itself without end;
  - a failed child's landing rode up as court work;
  - prose read as DONE;
  - children ran outside the caps.
- **Files:**
  - loop:
    - gate.rs — `Tree::refresh`; `courts` gone from the gate;
    - run.rs — the baseline takes what a commission landed or put back; `with_interrupt`; the
      run's own `landed`;
    - commission.rs — seats claimed and freed; a zero slice refused; caps and stress from the
      commissioner; a failed child put back whole; off-the-wire answers are FAIL; INTERRUPTED;
      neutral put-back notes;
  - tools — `set_cancel`;
  - tests: tampering judged, a modified stray restored and silent above, Esc three ranks down,
    one court per seat, prose a FAIL;
  - each test world has its own attic.
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review again.
- **Learned:**
  - An exemption by name is a hole: "this path is judged" must mean "this path as it stands
    now". The baseline refresh makes any later edit a change again.
  - Tests sharing one attic beside their worlds made a correct restore fail at random. Every
    store a test writes must be its own.

---

### [2026-10-25T01:02:46+02:00] rimuru — the commission, third pass
- **Task:** the re-review of #149 said rework.
  - The baseline refresh laundered a dispatcher's own earlier write once its child touched the
    file, and absorbed put-backs that failed.
  - An Esc during setup was swallowed.
  - Seats were not freed on a panic.
  - The docs were short.
- **Files:**
  - loop:
    - gate.rs — `refresh` takes only paths the dispatcher had not changed since its baseline;
    - run.rs — a shared interrupt is not cleared at ask start;
    - commission.rs — `Order.at`, only real put-backs refreshed, `Seat` freed on drop;
  - tests: laundering (proven failing without the fix), a stray with no attic refused above, a
    zero slice;
  - docs/04.
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review again.
- **Learned:** "judged by its own gate" covers what the child did, not what the path held. The
  dispatcher's changes before the order are still its own. A baseline refresh must compare
  against the world as the order found it, not just as it stands after.

---

### [2026-10-25T01:22:18+02:00] rimuru — a commission's facts, filed
- **Task:** close the loop between the commission and the facts: what a child learned about its
  territory reaches the ontology in its name.
- **Files:** crates/mnemorph-loop/src/commission.rs (`file_facts`), tests/commission.rs (the
  Slime's `@U territory src/ build=cargo test` filed and supported by slime-s), docs/04.
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review as a pull request.
- **Learned:** the parent's triage of a new free fact rides its `@V #<new> …` verdict. The loop
  does not read verdicts yet, so a collision stays `untriaged-fact` until it does. The finding is
  the honest state.

---

### [2026-10-25T09:05:40+01:00] rimuru — the human's Rimuru dispatches
- **Task:** the human said "go". The session the human opens now uses the commission runner:
  Rimuru may hand a lane's work to its Elf.
- **Files:**
  - crates/mnemorph-loop/src/open.rs:
    - `lanes` (living Elves under a living throne);
    - `dispatch_prompt`;
    - the commission tool and a `Commissioner` carrying the session's ledger, caps and stress
      line;
  - tests/commission.rs — a session through `open_with` dispatches down and back; a world with no
    creatures stays solo;
  - docs/04.
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review as a pull request.
- **Learned:** in a test binary the guard is not beside the executable, so a child's verify
  command is refused unless the world allows unguarded commands. A real `mnemorphd` sits next to
  its guard.
- **Open:** the client sees only the commission's report, not the bodies working below.
  Surfacing their beats, labelled by body, is the next rung.

---

### [2026-10-25T09:13:51+01:00] rimuru — the dispatching session, after its review
- **Task:** fix what the review of #152 found (verdict: merge after fixes).
- **Files:**
  - open.rs:
    - the dispatching session's envelope is what the caps leave, so slices carve from it;
    - prices parsed once;
    - the commissioner's world is the canonical one;
    - the prompt no longer says "alone" then "commission";
  - tests — the model is offered the tool and told the lanes, and the bare world is not;
  - docs/02, docs/03, solo.rs — solo may hand down in a world with Elves, the model choosing,
    until the three code-read signals land.
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review as a pull request.
- **Learned:** without an envelope, every slice was "all of it". The caps still held, but the
  budget law the docs promise never ran. An envelope is what lets the first Elf leave something
  for the second.

---

### [2026-10-25T09:21:21+01:00] rimuru — the human sees the order at work
- **Task:** the gap #152 left. The client saw only a commission's report, not the bodies working
  below.
- **Files:**
  - loop:
    - run.rs — `Seen::From { body, run, seen }`;
    - commission.rs — `Commissioner.observer`, each child's beats wrapped in its name;
    - open.rs — the session's observer handed to the commissioner;
  - server lib.rs — `emit_as`; a wrapped beat is emitted under the innermost body and run;
  - cli transcript.ts — `me` learned from the first turn the human starts; other bodies' events
    become short lines under their names (`below`);
  - tests: the session sees elf-e, orc-o and slime-s; the CLI keeps the session busy while a
    child turns;
  - docs/04.
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review as a pull request.
- **Learned:** a child's `turnEnded` folded as the session's would have ended the human's turn
  in the terminal. Children's events must never touch the session's own state.

---

### [2026-10-25T09:33:30+01:00] rimuru — the order at work, after its review
- **Task:** fix what the review of #153 found (verdict: merge after fixes).
- **Files:**
  - tools — `Ask.body`, `Context::with_body`;
  - loop:
    - commission — each child names its context;
    - solo — `rimuru`;
  - server:
    - `GateRequest.body`, `Hello.body`, `SESSION_BODY`;
    - stdio test — a Slime's write on the wire under `slime-s` and its own run;
  - cli:
    - `initial(hello.body)`;
    - the card says who asks;
    - a body's dollars below count in the session's spend;
  - protocol types regenerated;
  - docs/04.
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review as a pull request.
- **Learned:**
  - An approval is the one moment the human acts, so it must name who asks.
  - My earlier entry said a child's beats are "wrapped once per rank". Every rank shares the
    session's commissioner, so each beat is wrapped once, in its own name.

---

### [2026-10-25T10:14:28+01:00] rimuru — M7: the shared work queue, and its door
- **Task:** the human said "go". The shared work queue's store, its rank law, the human's door
  `mnemorph q`, and relay-hold of a report's `@$` orders.
- **Files:**
  - crates/mnemorph-world queue.rs — `Item` snapshots with revisions, a fold that refuses a line
    that does not follow, `State` held/claimed/done/dropped, `Move`, the rank law, `add`,
    `move_item`, `open`;
  - tests/queue.rs;
  - mnemorphd — `q` list/show/add/claim/touch/requeue/close, `--json`, a refusal exits 2;
    tests/onto.rs;
  - mnemorph-loop commission.rs — `hold_orders`;
  - packages/cli args.ts and cli.tsx — `mnemorph q`, words passed whole.
- **Gate:** n/a. Instruments:
  - states, the rank law, a race won by the first line;
  - the door by hand and in a test;
  - an Orc's `@$` held in its name;
  - `just check`.
- **Result:** in review as a pull request.
- **Open, next rung:** the wake, which claims a held row at the turn boundary in the name of the
  body it is ordered to and dispatches it; the `queue` tool on bodies' shelves; the status line's
  "N held"; a report settling its oldest claim.

---

### [2026-10-25T10:18:15+01:00] rimuru — the queue, closed to bodies' hands
- **Task:** a side note warned that `mnemorph q` counted whoever ran it as the human, and that
  the tripwire left `.mnemorph/shared/` writable, so a body's command could move the queue as the
  human. The human added: "agent should pick only a job they can do".
- **Files:**
  - tools:
    - tripwire.rs — `shared/` is no longer spared: a body's command or tool editing the queue is
      put back or refused;
    - shell.rs — every command carries `MNEMORPH_BODY`;
  - world queue.rs — a body never claims in another's name;
  - mnemorphd — `q` under `MNEMORPH_BODY` reads but never moves; `q_args`, `print_items` and
    `refused` split out;
  - tests: the claim rule, a body's `q` refused, and the two old "shared is writable" tests
    flipped;
  - docs/04, docs/05.
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review as a pull request.
- **Learned:** an environment variable is courtesy, the file is the border. A body could unset
  `MNEMORPH_BODY`, but the tripwire puts back any change its command makes to the queue. The
  queue moves only through the harness's own process.
- **For the wake (next rung):** it claims only an order its body can carry out. The dispatch law
  must hold, and every path the order touches must be on the body's ground.

---

### [2026-10-25T10:32:17+01:00] rimuru — #154 review fixes: the queue moves out of the world; no process leaves its group
- **Task:** fix #154's review: Opus reviewed the code, then a fresh Fable reviewed Opus's report.
- **Decided:**
  - **The queue's store moves outside the world**, beside the attic
    (`$XDG_STATE_HOME/mnemorph/queue/<world>-<hash>.jsonl`). The tripwire put back the human's
    concurrent moves, and an escaped process could forge lines; this place fixes both. The cost is
    that the queue is machine-local and no longer travels with `merge=union`.
  - **seccomp refuses `setsid` and `setpgid`**: killing the group ends everything a command
    started. This held for every protected record, not only the queue.
- **Also fixed:**
  - 12-hex ids, regenerated while taken;
  - ties run in order of arrival;
  - only the human and the harness claim;
  - `--as`/`--for` must name living bodies;
  - orders go only down the ranks;
  - `touch` needs a note;
  - the classifier no longer spares `shared/`;
  - `mnemorph q` is a command only as the first non-flag word.
- **Open (`@?`):** whether the queue should travel between machines. Doing that safely needs a
  per-user key or a human-run `q pull`.
- **Open (`@?`):** a parent hash in each line, so that union-merged lineages cannot interleave.
  This matters only if the queue travels.
- **Learned:** an environment variable is courtesy; a file's place is a border. If a guarantee
  depends on what a body's command might do after it is "dead", check that the kill is real.

---

### [2026-10-25T10:37:07+01:00] rimuru — #154 verification fixes
- **Task:** Opus checked the review fixes.
- **Fixed:**
  - `human` and `harness` are reserved creature names: a doc taking one is a roster problem, not
    a body that stands over all;
  - the queue's file name uses `fnv64a`, which is stable across toolchains;
  - a body's `mnemorph q` refuses outright (the landlock read denial had shown it an empty queue);
  - page 05 names what refusing `setsid` breaks (Python `start_new_session`, Go `Setsid`, ptys,
    `timeout` signalling the whole command) and that macOS has no such rule;
  - `.gitattributes` is gone: its only line was for the queue's old place.
- **Open (`@?`):** whether a pty-using tool (pexpect, `script`) is needed often enough that
  refusing `setsid` must be relaxed, e.g. by moving to a cgroup kill instead.

---

### [2026-10-25T10:45:15+01:00] rimuru — the queue back in the world, gitignored, witnessed
- **Task:** the human chose to keep the queue in the world at `.mnemorph/shared/queue.jsonl`, but
  ignored by git: "no keep and add in git ignore its better".
- **Decided:** the queue stays machine-local. A witness under the state directory holds every line
  the harness or the human's `q` wrote. The tripwire puts the queue back line by line: the old
  lines stay, and of the added lines only those the witness holds are kept. A forged move is
  dropped, and the human's concurrent moves are kept.
- **Files:**
  - world queue.rs — `Queue::of` is in the world and witnessed; `witness_of`; `land` writes the
    witness first;
  - tools tripwire.rs — `restore_queue`, `queue_witness`, the queue kept apart from the 16 MiB
    budget;
  - shell.rs — `.mnemorph/shared/` is made before the snapshot;
  - .gitignore;
  - docs/04, docs/05, the roadmap.
  - Tests: the witness restore; a loop test that holds the world's and the tripwire's witness path
    together.
- **Open (`@?`):** a race window remains while the tripwire rewrites the queue. A move landing
  within that write is lost; it is narrow, and the witness still holds the line.

---

### [2026-10-25T10:51:33+01:00] rimuru — the queue rebuilt from its witness
- **Task:** Opus verified the witness design and found four holes:
  - a symlink at the queue's path survived and was written through (into the world, or out of
    it);
  - deleting or relinking `shared/` lost the queue;
  - a command could drop or reorder the human's concurrent moves;
  - `open.rs` passed a non-canonical world.
- **Fixed:**
  - the tripwire records the witness's length at the snapshot, and rebuilds the queue as the old
    bytes plus the witness's gain since, in the witness's order, never reading the command's file;
  - the rebuild runs after the generic restore, so `shared/` stands again;
  - whatever is at the path is removed and the file is created new;
  - `Queue::land` refuses a link at the queue or its shelf;
  - `Queue::of(cx.world())`.
- **Learned:** a restore that consults the tampered file inherits the tamper. Rebuild only from
  what the command could not reach.

---

### [2026-10-25T10:57:07+01:00] rimuru — the witness's lock
- **Task:** Fable checked the rebuild. All four holes are fixed. One real loss remained: a move
  landing between the tripwire's read of the witness and its write of the queue was overwritten
  for good.
- **Fixed:**
  - `Queue::land`, the snapshot and the rebuild all hold the witness's file lock (`File::lock`),
    so none of them can interleave;
  - the snapshot reads the queue and the witness's length under that lock;
  - page 04 says that with no home there is no witness.
- **Open:** the first move ever creates the witness. A rebuild that finds no witness takes no
  lock, so the very first move can still race it. The window is once per machine and
  microseconds wide.

---

### [2026-10-25T11:07:43+01:00] rimuru — M7: the wake
- **Task:** the human said "merge and continue after fix if needed": #154 was merged, and the
  next rung is the wake.
- **Files:**
  - mnemorph-loop:
    - commission.rs — `carrier`: the body it reports to may lawfully commission it, and every
      path the order names that exists lies on its ground;
    - commission.rs — `settle`: DONE closes, another word notes the exit, a refusal requeues;
    - run.rs — `Run::wake`, `woken` opening the next ask;
    - session.rs — `drive` wakes after each turn.
  - Tests:
    - the wake runs what its body can carry, leaves the rest held with one note, and opens the
      next turn;
    - an off-wire answer files no facts and holds no orders (the loose end from #151).
  - docs/04 relay-hold.
- **Decided:** a held order is commissioned by the body its target reports to, whoever ordered
  it. `Queue::add` already refused orders up the ranks, so that body acts in the orderer's stead,
  and the dispatch law is checked again in `order`. The wake's reports reach the next turn as a
  preface, at no extra model call.
- **Open:** the bodies' `queue` tool; the status line's "N held"; a wake that runs while the human
  is away (it now runs only after a turn).

---

### [2026-10-25T11:16:07+01:00] rimuru — #155 review fixes
- **Task:** fix Fable's review of the wake.
- **Fixed:**
  - the ground check counts the target's ground and that of every body below it, so an Orc
    carries orders through its Slimes;
  - a turn the human interrupted wakes nothing: an Esc and a departed client both end the turn
    `Interrupted`;
  - a spent budget stops the wake, so there is no claim/refuse churn;
  - `INTERRUPTED` and `CHECKPOINT` put an order back on hold, and every settle note is said once;
  - `named_path` drops `:line`, makes absolute paths inside the world relative, and holds a `..`
    or outside path;
  - the docs say "every existing path".
- **Open:** the ledger row of a woken commission names its dispatcher as the parent body. A
  `queue <id>` marker on the row would name the wake.

---

### [2026-10-25T11:27:38+01:00] rimuru — M7: the bodies' queue tool, and "N held"
- **Task:** continue M7 after #155: the `queue` tool for bodies, and the status line's held count.
- **Files:**
  - mnemorph-loop:
    - queue_tool.rs — `QueueTool`: list and show are free; touch, requeue, close and drop follow
      the rank law; there is no claim and no add;
    - run.rs — a `queue_keeper` on every commissioned run; `call_one` routes `queue` through it;
    - commission.rs and open.rs — register the tool.
  - mnemorph-server — `Event::Held` after each drive, `Hello.held`.
  - CLI:
    - transcript.ts — `held` state, `status(…, held)`;
    - app.tsx;
    - fixtures.
  - Tests: the queue tool follows the rank law (Rimuru closes, a Slime is refused, a claim
    is unknown); the held count reaches the status line.
  - docs/04.
- **Learned:** a Slime has no commissioner, because it cannot dispatch. Giving it one would let
  `commission`, called by name, run below the shelf check, so the queue is reached through its
  own keeper instead.

---

### [2026-10-25T11:36:19+01:00] rimuru — #156 review fixes
- **Task:** fix Fable's review of the queue tool.
- **Fixed:**
  - a body never closes or drops its own order (`move_item`): only a body above it, the human or
    the harness discharges it. A body could otherwise close work it never ran and report DONE;
  - `settle` says the order's real state;
  - `queue` is intercepted only when the run offers the tool;
  - the test helper registers `QueueTool`, as `open.rs` does.
- **Open:** the status line's count is refreshed only after a turn, so a `q add` from another
  terminal shows at the next turn.

---

### [2026-10-25T11:43:55+01:00] rimuru — M7: the journal's writer
- **Task:** continue M7: the append-only `log.md` writer, and the gate's verdicts through it.
- **Files:**
  - world log.rs — `Entry`, `render`, `append`: whole entries, append only, refused through a
    link;
  - loop run.rs — `log_verdict` on every verdict: `PASS`, `FAIL, handed back`, `FAIL`, with the
    changed paths and the failures;
  - tests: tests/log.rs, and the down-and-back test reads the Slime's PASS from the journal;
  - docs/03; the roadmap line reflowed.
- **Open:**
  - three fails running for one body on the same files harden into the lawful next acts
    (rewrite, split, escalate);
  - a concurrent session's command can still have its tripwire put back a journal entry written
    meanwhile. This is the same class as the ledgers under `instruments/`.

---

### [2026-10-25T11:50:45+01:00] rimuru — #157 review fixes
- **Task:** fix Fable's review of the journal's writer.
- **Fixed:**
  - `clean` turns every control character and line separator into a space in every field;
    before this, a `\r` in a verify command's output could forge an entry;
  - a pass that changed nothing is not journaled;
  - a world without `.mnemorph/` gets no journal;
  - the file is opened with `O_NOFOLLOW` (`libc`).
- **Open:** a concurrent session's tripwire can put back an entry. Append-only is checkable, so a
  put-back could keep growth whose prefix matches the snapshot, given a witness for what grew.

---

### [2026-10-25T12:18:54+01:00] rimuru — M7: grants from the giver's own budget, bounded by worth
- **Task:** the human's design: a shared budget, handed down the ranks. Each giver judges what a
  child is worth and gives from its own budget: "rimuru can judge to give budget from shared
  budget to an elf, the elf decide for an orc and orc decide for slime", and "each one deciding
  can give from his budget too".
- **Files:**
  - world worth.rs:
    - `Card` (spent wt from the usage ledger, gate passes and fails from the journal), counted
      for a body and every body below it;
    - worth `0.5 + 0.5 × (passes+1)/(passes+fails+2)`;
    - `share_pct` (page 06's compiled shares);
    - `ceiling = left × 90% × share × worth`;
  - loop:
    - commission.rs — `grant()` clamps the asked slice, and the report says when it was cut;
    - commission.rs — `cards_below` in every dispatching child's prompt;
    - open.rs — the cards in Rimuru's prompt;
    - run.rs — the wake draws at most a quarter of the pool and runs at most 3 orders;
  - tests:
    - a card counts the body and its subtree; a handed-back failure is not a verdict;
    - the ceiling keeps reserve, share and worth, and a chain of grants shrinks;
    - a bounded grant is told to the dispatcher;
    - a wake runs at most three orders;
  - docs/06.
- **Decided:** the giver judges, and the harness bounds it. A child's record counts in its
  parent's card, so funding failing children costs the parent. The learned value model (M11)
  replaces the worth score; the card and the clamp stay.
- **Next:** top-ups. A child out of budget asks its parent, and the parent gives from its own
  envelope.

---

### [2026-10-25T12:27:03+01:00] rimuru — #158 review fixes
- **Task:** fix Fable's review of grants and cards.
- **Fixed:**
  - the dispatcher is told whenever it gets less than asked, including the default half, which
    always exceeds the ceiling;
  - a handed-back failure counts half, so worth no longer drifts to 1;
  - only entries the end gate wrote count (`- **By:** the end gate`), so a hand entry is ignored;
  - `Records` is read once per prompt;
  - spend on cards is rounded to the thousand, so a card changes with the record, not every call;
  - a refused order no longer counts toward the wake's three;
  - the tool text and page 06 say "half, within the ceiling", and the plan weights are marked as
    to come.
- **Open:**
  - a creature reborn under a retired name inherits the old record;
  - cards in a child's system prompt make a second commission of the same child miss the cache.
    Moving them to the first user turn would keep the system fixed.

---

### [2026-10-25T12:32:07+01:00] rimuru — M7: the top-up
- **Task:** the second half of the human's grant design: a giver gives from its own budget, once
  more when the child runs out.
- **Files:**
  - commission.rs — `topup_wt` in the args and schema; `grant()` bounds it by what the parent may
    still give (90% of left, less the slice) and says when it is cut;
  - run.rs — `with_topup`; `perceive` gives it once when the slice runs out, then reads the caps;
  - tests: a top-up carries a child past its slice, and without one it stops at the checkpoint;
  - docs/06.

---

### [2026-10-25T12:37:35+01:00] rimuru — #159 review fixes
- **Task:** fix Fable's review of the top-up.
- **Fixed:**
  - slice and top-up together stay within twice the child's ceiling; before, a 1 wt slice with a
    huge top-up made worth decorative;
  - the child is nudged when the top-up is given;
  - with no cap there is no top-up.

---

### [2026-10-25T12:54:25+01:00] rimuru — M7: three fails harden into the lawful next acts
- **Task:** the human said "ok merge and continue". Research PR #6 was merged, and the next M7
  rung is page 03's three-fails rule.
- **Files:**
  - world worth.rs:
    - `Records` keeps each verdict's changed paths;
    - `stuck(name)`: the last three final verdicts are all FAIL, and the paths shared by all
      three;
    - `STUCK_AFTER`;
  - loop commission.rs — `lawful()` (the dispatch law plus the stuck check) refuses an order that
    names a stuck file, with the three routes;
  - tests: stuck needs three and a pass clears it; a refused order and a rewritten one;
  - docs/03.
- **Open:** "names a file" is a substring match on the order's text; a split or rename of the
  stuck file escapes it.

---

### [2026-10-25T13:01:35+01:00] rimuru — #160 review fixes
- **Task:** fix Fable's review of the three-fails rule.
- **Fixed:**
  - the wake's `carrier` checks stuck files, so a held order on one waits, noted once, instead of
    being claimed and refused every turn;
  - `worth::names` matches a whole word (`src/a.rsx` no longer matches `src/a.rs`);
  - changed paths are split on the backtick boundaries, so a path with ", " is read whole;
  - the records are loaded once per commission (`grant` and `lawful` share them);
  - a leading `./` no longer loses its dot to the trim in either path reader. Before this,
    `named_path` (#155) read `./src/x` as `/src/x`, an absolute path off every ground;
  - `./` steps are dropped before the ground check.

---

### [2026-10-25T13:17:37+01:00] rimuru — M7: the parent's triage of resembling facts
- **Task:** the human said "go". Next M7 rung: a free fact that resembles older ones reaches the
  parent, and the parent's `@V` labels are filed. This closes the ontology's exit item.
- **Files:**
  - world facts.rs — `label_of`: `#<new> support|counter|refines #<id>` or `#<new> distinct`;
  - loop commission.rs:
    - `file_facts` returns triage lines for new free facts with BM25 candidates above
      `THETA_T`, and the report carries them;
    - `file_labels` files the reporting body's labels, only on facts a body reporting to it said;
  - tests: label parsing; the round trip (the Orc sees the triage, counters, the label is filed,
    and a label on the Elf's own fact is refused);
  - docs/09.
- **Open:** Rimuru's own answers are not wire reports, so the throne's triage of an Elf's fact is
  not filed yet. It stays untriaged, and the finding says so.

---

### [2026-10-25T13:23:18+01:00] rimuru — #161 review fixes
- **Task:** fix Fable's review of the triage.
- **Fixed:**
  - triage rides as a hole (`@? triage …`), never cut; as prose it was the first thing a long
    report lost;
  - a label counts only when every supporter of the fact reports to the labeller;
  - the first label stands, across reports and within one; the test caught a second label in one
    report overriding the first;
  - quoted fact text is one line of at most 200 characters, with no quote or bracket of its own.
- **Open:** the throne's triage of an Elf's fact; a re-show of an unlabelled fact on the parent's
  next commission.

---

### [2026-10-25T13:32:57+01:00] rimuru — M7: the throne's triage
- **Task:** the human said "go". This closes #161's open item: the throne labels its Elves'
  resembling facts.
- **Files:**
  - loop commission.rs — `file_verdicts` shared by a child's wire report and the throne's answer;
  - run.rs — `file_triage` reads `@V` lines in the throne's plain answer;
  - session.rs — called after each turn;
  - open.rs — the dispatch prompt says how to label;
  - test: an Elf's fact is triaged by the throne in prose, and its label is filed;
  - docs/09.

---

### [2026-10-25T13:37:12+01:00] rimuru — #162 review fixes
- **Task:** fix Fable's review of the throne's triage.
- **Fixed:**
  - `@V` lines inside a code block are quoted, never said, and label nothing; the test puts a
    conflicting fenced label before the real one;
  - the prompt wording is clearer, and the doc comment says only an answered turn is read.

---

### [2026-10-25T13:50:59+01:00] rimuru — M7: housekeep --heal
- **Task:** the human said "go". Next M7 rung is self-heal's `housekeep --heal`. The clock-death
  memo waits, because its code is in commission.rs, which another session is changing (Thompson
  worth, in its own worktree).
- **Files:**
  - world heal.rs — `heal(world, apply)`: a retired stub for a seat with no doc; a broken
    *Reports to* relinked to the nearest living ancestor or the throne; problems and loops left;
    one journal entry per repair;
  - mnemorphd — `housekeep [--heal]`, exit 1 while anything is left;
  - CLI — `mnemorph housekeep` passes through like `q`;
  - tests: tests/heal.rs (dry run, three repairs, the rest of a doc untouched, journaled, idempotent),
    and the CLI route;
  - docs/03.

---

### [2026-10-25T13:57:55+01:00] rimuru — #163 review fixes
- **Task:** fix Fable's review of `housekeep --heal`.
- **Fixed:**
  - a relink goes only to a living ancestor of the race that may command the body (an Orc for a
    Slime, an Elf for an Orc); with none, it is left. Before, a Slime relinked to its Elf broke the
    race law;
  - no write through a link: race dir, seat and doc are checked, and a stub is created new;
  - the relink splices only the field's own line, keeping CRLF, the trailing newline and prose
    field-looking lines;
  - inside a body's command `housekeep` refuses (exit 2);
  - exits: 0 when whole, 1 when anything is left or a repair waits for `--heal`, 2 on errors;
  - a journal write that fails is left as a finding, not swallowed;
  - the stub name check also covers names the roster refused.

---

### [2026-10-25T14:16:45+01:00] rimuru — M7: relief
- **Task:** the human said "yes cargo clean and continue". `cargo clean` freed 39.5 GB (the disk
  had filled). Next M7 rung, clear of the other session's commission work: relief.
- **Files:**
  - world relief.rs — `relieve(world, name, apply)`:
    - a split in halves by claims, or by the tree under a one-directory ground;
    - otherwise the seat retires;
    - the young get docs (created new, never through a link), the old seat gains one `Retired`
      line, and one journal entry is written;
  - mnemorphd — `relief <name> [--apply]`, refused in a body's command;
  - CLI — passes it through;
  - tests: a split by prefixes (CRLF doc, memory kept, no overlap), a split by tree, a retire,
    an Elf refused;
  - docs/03.
- **Next:** the council — strain counted per body, `@INSTRUCT`/`@SPLIT`/`@ESCALATE` at 5, relief
  at 7, forced escalation after three warrants.

---

### [2026-10-25T14:25:29+01:00] rimuru — #164 review fixes
- **Task:** fix Fable's review of relief.
- **Fixed:**
  - an Orc's living Slimes are relinked to the young that covers their zone; before, they were
    orphaned with High findings;
  - a root-file claim is written `./FILE`, so the young parse;
  - preflight checks both seats and the old doc before any write, and a failed young write is
    undone; the dry run checks too;
  - the tree split descends through a lone directory, a link is a file, and hidden entries go to
    `-a`;
  - an ascended race gives young of the base race, seated beside the old;
  - Minds are copied;
  - nested claims are deduplicated;
  - the old seat's open queue orders are named, to re-route;
  - `heal::splice` is shared.
- **Open:** moving queue orders to the young automatically, and relief's automatic trigger (the
  council).

---

### [2026-10-25T14:33:32+01:00] rimuru — M7: the council
- **Task:** continue M7: the council, relief's automatic trigger.
- **Files:**
  - loop council.rs — `Thresholds` (`stress.escalate_at`/`relieve_at`/`max_consults`, defaults
    5/7/3); `tally`; `sit`:
    - strain is the failed acts, plus one when the report did not answer DONE;
    - a clean DONE clears it, and an interrupted run adds nothing;
    - a warrant comes at 5, a forced ESCALATE after 3 warrants, relief at 7, then the trigger
      resets;
  - stress.rs — `failed()`; run.rs — `failed_acts()`;
  - commission.rs:
    - `report_for` gathers facts, labels, holds and the council;
    - `council()` turns a warrant into a hole, relief into a note, and the last warrant into the
      forced status;
  - open.rs — thresholds from the config;
  - tests: tests/council.rs (warrants, the escalation, the clearing, an interrupt, relief at 7 and
    the reset) and the warrant reaching the parent;
  - docs/03.
- **Open:**
  - re-asks are not yet counted;
  - a gate denial by the human still counts as a failed act (the doc says denials never add).

---

### [2026-10-25T14:41:45+01:00] rimuru — #167 review fixes
- **Task:** fix Fable's review of the council.
- **Fixed:**
  - a DONE clears, whatever failed on the way; before, a red-green body drifted to relief;
  - a gate denial carries `Meta.denied`, and the stress meter does not count it;
  - relief acts only after a warrant; before, one 7-failure run skipped the parent;
  - the tally is cleared only when relief acts; a refused relief (an Elf) escalates and keeps it;
  - only answered runs count, so strain is never read from how the loop ended;
  - the forced ESCALATE keeps the body's own status (`was: …`);
  - docs: the config keys' real names, and re-asks marked as not counted yet.
- **Open:**
  - a parent's loop does not act on a council-forced ESCALATE in a child's report beyond relaying
    it;
  - the tally is never compacted.

### [2026-10-25T14:45:30+01:00] rimuru — a slice that runs out is never a failure on the card
- **Task:** Veldora asked what Mnemorph gained from the value studies, then "do both": pin the
  rule the budget-loop study found worth keeping, and build the instruments the real runs showed
  missing (the second, as its own pull request).
- **Files:** crates/mnemorph-loop/tests/commission.rs (a Slime funded one weighted token writes,
  checkpoints on budget before answering, and its card reads 0 passes, 0 fails, worth 0.750; no
  `gate:` verdict in the journal), docs/06-consumption.md (the rule, beside worth).
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review as a pull request. No behaviour changes: the test holds what the code
  already does.
- **Learned:**
  - The research simulated a Thompson draw for the worth that sizes a slice and found it no better
    than the mean (−0.001 landed): the owner works every order it owns, so its record is observed
    whatever its slice, and there is no exploration to buy. What mattered was the rule this test
    pins: counting a run that ran out as a failure cost 2 to 10 landing points.

### [2026-10-25T14:45:30+01:00] rimuru — the instruments a value model needs, journaled per act
- **Task:** Veldora: "do both" — after the guard test, the four instruments the real Terminal-Bench
  runs showed missing: lines changed per write, edit misses, verification scripts counted as
  tests, and a guts reading journaled for every act.
- **Files:** mnemorph-tools tool.rs (`Meta.lines_added`, `lines_removed`, `edit_miss`), files.rs
  (`line_delta`, a multiset count; write, edit and multiedit set it; a missing `old_string` is a
  typed refusal, marked a miss), mnemorph-loop stress.rs (verification scripts are benches;
  `Reading`, the last act as the guts read it; the bench tally in its own method), beats.rs
  (`Beat::Guts`, ignored by the rebuilt conversation), run.rs (`record_guts` per act),
  mnemorph-server (the beat reaches no surface), tests (tools, the check-script detector, a run's
  guts beats), docs/03-loop.md (the bench tally, the guts journal).
- **Gate:** n/a. Instruments: `just check`.
- **Result:** in review as a pull request. No behaviour changes but one: a verification script now
  counts in the bench tally, so its stall rules apply to it.
- **Learned:**
  - The test caught an inline check whose script holds a `;` (`from run import f; assert …`):
    the tally cuts a command at its first `;` to find the runner, which dropped the `assert`. The
    interpreter is read from the cut command, the `assert` from the whole one.
  - Fable's review found the same `;` muting the checks: the tally read any `;` or `|` as hiding
    the exit status, so a passing `python3 -c "import m; assert …"` was never green, a failing one
    never red, and eight of them stopped the run. An inline check's script is now found (its quoted
    argument, or a heredoc's body): `assert` is looked for inside it, `|` and `;` outside it.
---

### [2026-10-25T14:47:59+01:00] rimuru — M7: the clock-death memo
- **Task:** the human said "merge and continue after fix if needed". #167 (the council) was merged;
  the next rung is the last self-heal piece, the clock-death memo.
- **Files:**
  - commission.rs:
    - `Death` rows in `instruments/deaths.jsonl`;
    - `clock_memo`: on a salvaged checkpoint, the next-fit sibling (same race, same parent, best
      worth) or "a smaller order, or a longer clock", as a note in the report;
    - a unit test;
  - docs/03.
- **Next in M7:** Courts per office (fresh contexts); Metis attached to dispatch; solo escalation
  measured on 30 tasks (costs live runs: ask first).

---

### [2026-10-25T14:53:09+01:00] rimuru — #168 review fixes
- **Task:** fix Fable's review of the clock-death memo.
- **Fixed:**
  - the memo keys on the clock's own checkpoint, so a salvage turn that wrote nothing is still a
    death;
  - the memo is a hole, never cut;
  - a sibling stuck on its files is not named;
  - a tie goes to the sibling that spent least, then by name;
  - the death row names the body suggested;
  - docs: instruments, not the ledger, and the same base race.

---

### [2026-10-25T15:12:12+01:00] rimuru — M7: offices, and Metis attached to dispatch
- **Task:** the human chose "offices": per-commission model choice by office, with Metis's
  routing attached to dispatch.
- **Files:**
  - loop commission.rs:
    - `office` on the commission args and schema, validated (`office_of`);
    - `Router` (config, Metis's decisions, the session's provider);
    - `route_child` calls `route::route` before the court runs; the court rides the routed model,
      or the dispatcher's when the model is on another provider, said in the report;
    - Metis's decision wt is added to the commission's spend;
  - open.rs — `router()`, the commissioner's own decisions;
  - tests: a (race, office) pin rides and is named in the report; an unknown office is refused;
  - docs/02.
- **Open:** a court on another provider needs its own client (multi-provider sessions).

---

### [2026-10-25T15:19:13+01:00] rimuru — #169 review fixes
- **Task:** fix Fable's review of offices and routing.
- **Fixed:**
  - the session's provider is the client's own (`model.provider()`). Before, it came from the
    stripped model id, so a vLLM session looked like Anthropic;
  - routing runs first, and Metis's cost comes out of `left` before the grant, so the parent's
    envelope is never overshot; the route row names the dispatcher (`commission`);
  - a model that cannot ride says why, never "another provider";
  - `Decisions` is Clone, and one decider serves the run and the commissioner;
  - `Reason` reads in words, and nothing is said when nothing was routed;
  - the route doc's office comes from the commission's `office`.
- **Open:**
  - `Stop.left` is a fixed 60 s, though the decider's own policy clamps it to its turn;
  - a refusal after routing drops Metis's cost from the in-memory envelope; the ledger keeps it.

---

### [2026-10-25T15:32:17+01:00] rimuru — M7: solo escalates on signals read by code
- **Task:** the human said "run measurement". The measurement needs solo's escalation by code
  first; this PR builds it.
- **Files:**
  - tools — `Meta.unheld` and `Outcome::unheld` at both shelf refusals;
  - loop run.rs:
    - `note_unheld` records an outward act or a seal the shelf refused;
    - the ask is kept in `preface`;
    - `Run::escalate`: on a context checkpoint or an unheld act, hand the ask to the Kijin, the
      Dark Elf, or the Elf whose lane the ask names (else the first), with a handoff, and answer
      with its report;
  - session.rs — escalate after each turn;
  - test: an outward refusal hands to the Kijin, and a quiet turn stands;
  - docs/02.
- **Open:**
  - the boundary signal waits for solo's zone to narrow;
  - the stress path is untested (a context checkpoint needs a real fill).
- **Next:** the 30-task bench and runner, then the live run (the key comes from the human).

---

### [2026-10-25T15:39:17+01:00] rimuru — #170 review fixes
- **Task:** fix Fable's review of solo escalation.
- **Fixed:**
  - a refused commission keeps the turn's ending (noted), never "Escalated";
  - an interrupted or failed turn never escalates, and an Esc during the escalation is honoured;
  - an unheld outward act hands the act itself (tool and input) to the Kijin, and solo's answer
    stands beside the report; with no Kijin it stays;
  - the stress path stays a checkpoint (the session is full), the Elf's report as its salvage;
  - the dead seal branch is dropped (no tool yields a seal);
  - lane choice matches path words component-wise;
  - the throne's triage is read before escalation, so the Elf's verdicts are not filed as
    rimuru's;
  - docs/02.
- **Open:** the stress path's test needs a real context fill; the boundary signal.

---

### [2026-10-25T15:47:13+01:00] rimuru — M7: the solo-against-hierarchy bench
- **Task:** the human said "run measurement" and has $25 of OpenRouter credit, so free models get
  the higher daily cap. Step 2 of 3: the bench and its runner.
- **Files:**
  - bench/tasks — 30 stdlib-Python tasks (starter, ask, verify outside the world);
  - bench/world — the creatures (throne, Elf, Orc, Slime);
  - bench/run.py:
    - `--check` (every task fails untouched);
    - fresh worlds, solo and hierarchy modes;
    - the key only in the child's environment;
    - results per run, resume, a stop on rate limits;
  - bench/README.md.
- **Checks:**
  - `run.py --check`: 30 tasks, none passes untouched;
  - `onto check` on the bench world: PASS, 0 findings.
- **Next:** the live run (the key from the human), then the write-up.

---

### [2026-10-25T15:54:40+01:00] rimuru — #171 review fixes
- **Task:** fix Fable's review of the bench.
- **Fixed:**
  - rate limits are read on stdout as well as stderr (a body's 429 is a tool result), with a
    narrower pattern, and any limit stops the run;
  - hierarchy runs are verified: `below` counts the calls of bodies under the throne, and a
    hierarchy run with none is `valid: false`;
  - a timeout kills the run's whole process group, and the timeout is 30 minutes;
  - stderr is scrubbed before it is cut, and a key file others can read is refused;
  - task 25: `arith.py` (no shadowing of the stdlib `numbers`), and any `math` import is caught;
  - the README: results are tracked; the guard helper is needed.

---

### [2026-10-25T19:16:21+01:00] rimuru — the bench's smoke run, and what it found
- **Task:** the human gave the OpenRouter key (pasted in chat; it was passed only in the run's
  environment, never written). The smoke run on task 01 found three faults before any real
  measurement.
- **Found:**
  - an Elf tried to write the code itself, and was rightly refused: no race card told it to hand
    the writing down;
  - Rimuru gave the Elf `budget_wt: 1`, so the Elf stopped after one call;
  - `mnemorph run` ended at the first `turnEnded`, which was the Elf's (children's beats reach
    the client since #153), so the throne's turn was killed as "interrupted".
- **Fixed:**
  - `race_card` in every commissioned body's prompt;
  - `SLICE_FLOOR` (20 000 wt, a commissioner setting, 0 in the test helper): a slice is raised
    to at least one call, and the dispatcher is told; a deliberate 0 is still refused;
  - run.ts waits for the session body's own turn end, and prints only its text;
  - bench `--keep` and `--out`;
  - docs 02/06, the bench README.
- **Learned:** run the smoke before the sixty. Each of these would have turned every hierarchy
  result into noise.

---

### [2026-10-25T19:22:19+01:00] rimuru — the second smoke run, and #172's review
- **Task:** the second smoke run (on #172's build) and Fable's review of #172.
- **Found by the smoke:**
  - the Elf now tried to commission its Orc, and the dispatch law refused it (no plan item);
  - the Elf could not write its own `plan.md`, since the file tools protect all of `.mnemorph`;
  - the CLI's `dist` predated the run.ts fix (`just check` tests the source, not the build).
- **Fixed:**
  - the file tools allow a body's own `plan.md`, directly in a seat that is one of its soil roots,
    and nothing else in `.mnemorph`; the Elf's race card says to write the plan item first;
  - the review:
    - the pre-fix smoke results are dropped;
    - the slice law is written whole (the floor outranks worth, never the reserve);
    - the note names the bound applied;
    - run.ts keeps children's acts and warnings on stderr, under their names;
    - the race cards use the law's words;
    - the bench's `--keep` goes through argparse;
  - docs 05/06.
- **Security note:** the plan exemption relaxes a protection, so Opus reviews it.

---

### [2026-10-25T19:30:49+01:00] rimuru — #172's security review: Opus, then Fable on Opus's report
- **Task:** the plan exemption relaxes a protection, so Opus reviewed it and Fable reviewed
  Opus's report.
- **Opus:** no escalation. Two low findings: the exemption keyed on any soil root under
  `.mnemorph`, and `plan_items` followed links. Also a docs asymmetry.
- **Fixed:**
  - `Context.seat` (`with_seat` from `context_for`), compared exactly;
  - a `plan.md` that is a link is no plan;
  - the Elf's card and docs/05 say a plan is written with the file tools, and a command's write
    is put back.
- **Fable:** confirmed each fix; nothing missed escalates (inherit keeps the seat; a symlinked
  seat is still refused by `admits`; `beneath` closes the TOCTOU). Safe to merge. Its suggested
  test for a linked plan is added.

---

### [2026-10-25T19:40:57+01:00] rimuru — the fourth smoke passes the hierarchy end to end
- **Task:** fix what the third smoke found.
- **Found:**
  - children starved at the 20 000 floor (the bench sets no cap);
  - the Orc could not code its Slime's verify line, since docs are protected;
  - a per-minute rate limit stopped the run.
- **Fixed:**
  - with no cap there is nothing to carve: a commission's `budget_wt` is not read and the slice
    is unbounded (an ask of 0 is still refused);
  - the bench Slime's doc carries `python3 -m compileall -q src` as its verify line;
  - the runner waits out rate limits (70/140/280 s) before calling it the daily cap;
  - two tests now set a cap to keep their meaning;
  - docs/06.
- **Result:** task 01 hierarchy PASS, 125.9 s, 29 calls (26 below), 109 525 wt: throne → Elf
  (its plan) → Orc → Slime (it wrote, its gate passed).
- **Open (design):** an Orc has no tool to code its Slimes' verify lines (the docs are
  protected); the bench seeds one.

---

### [2026-10-25T19:47:27+01:00] rimuru — #173 review fixes, the measurement running
- **Task:** fix Fable's review of #173 while the measurement runs. It measures commit `9d17040`;
  the source is built into a separate target, so the measured binary is never rebuilt mid-run.
- **Fixed:**
  - `grant()` works on a plain `u64` after the no-cap return (the dead `Option` plumbing is gone);
  - the commission tool says `budget_wt` is not read with no cap;
  - the runner stops at once on the daily cap, counts a limited attempt's spend into the row it
    records (with `attempts`), and names a kept limited world;
  - a nested test: under no cap, `budget_wt` limits neither the Elf nor the Orc it commissions.

---

### [2026-10-25T19:55:49+01:00] rimuru — M7: the verify tool
- **Task:** the human asked to fix the gap a side note named: outside the bench, an Orc could not
  code its Slime's verify line (docs are protected), so the chain stalled at the Orc.
- **Files:**
  - world verify.rs — `code(world, orc, slime, command)`:
    - only a living Orc, on a living Slime reporting to it;
    - one line of 1 to 300 characters, no backtick, never through a link;
    - the line is set or added, other bytes kept, and journaled;
  - loop verify_tool.rs — `VerifyTool` (class dispatch), offered to Orcs, routed in `call_one`;
  - the Orc's race card names it;
  - docs 02/05;
  - tests: world tests/verify.rs; the loop round trip (refused, coded, then commissioned).
- **Security:** it writes a creature doc for a body, so Opus reviews it and Fable reviews the
  review.
- **Built and tested** in a separate target (`../tempest-review-target`): the measurement runs on
  the main target's binary.

---

### [2026-10-25T20:03:53+01:00] rimuru — #174's security review: Opus
- **Task:** Opus reviewed the verify tool.
- **Opus found:**
  - HIGH: verify commands ran after the gate's judgement, so a verify line could write code
    unjudged. This was true for any verify line, not only the Orc's;
  - MEDIUM: the tool replaced the first verify line, so an Orc could weaken a human's check;
  - LOW: a gap between reading and writing the doc, a linked seat unchecked, and `fs::write`
    following links;
  - LOW: control characters got through.
- **Fixed:**
  - the gate's `run_verify` snapshots around the verify runs and fails on any change beyond
    git-ignored output ("a check never writes");
  - the tool only adds (never replaces, once);
  - it parses the text it read, refuses a linked seat or doc, and writes a new file renamed over
    the doc;
  - control characters and line separators are refused;
  - the bench worlds ignore `__pycache__/`;
  - docs 02/03; tests (a sneaky write fails the gate, built output passes; add-only; control
    characters).
- **Next:** Fable reviews Opus's report.

---

### [2026-10-25T20:10:39+01:00] rimuru — the report folder
- **Task:** the human, on the side note that checks may no longer write: "can we do it in init
  inside the harness, or create folders .nmem_report… and gitignore them?" Also Fable's follow-ups
  on #174.
- **Files:**
  - world report.rs — `prepare` makes `.mnemorph/report/`, and adds it and the common test
    leftovers to `.git/info/exclude` (local, never committed, idempotent, never through a link);
    called when a session opens;
  - tools — the tripwire, the file tools and the classifier treat `report/` like `tmp/`;
  - run.rs — a "check never writes" failure fails the landing whole, with no retry;
  - gate.rs — the refusal says where to put a check's output;
  - verify.rs — a link anywhere on the doc's path is refused;
  - tests: report.rs (made, ignored once, no git), and a check writing the report folder passes;
  - docs 03/05.

---

### [2026-10-25T20:14:06+01:00] rimuru — #175's security review: Opus
- **Task:** Opus reviewed the report folder.
- **Opus found:**
  - MEDIUM: the gate's snapshot honoured `.git/info/exclude`, so an unanchored `coverage/` hid any
    tracked `src/coverage/` from the whole gate;
  - LOW: the no-retry rule matched text a test could print;
  - LOW: a race in `prepare` (`fs::write` follows a link).
- **Fixed:**
  - the snapshot does not read the local exclude file;
  - the coverage patterns are anchored at the root;
  - `GateVerdict.wrote` is a structured flag, and the run fails whole on it;
  - `prepare` writes a new file and renames it over `exclude`.
- **Open:**
  - a solo run's FAIL does not put back its writes (only a commissioned child's landing is);
  - a self-ignoring `newdir/.gitignore` hides a new directory from the gate.

---

### [2026-10-25T20:19:58+01:00] rimuru — #175: the fixes the last entry claimed
- **Task:** Fable's check of Opus's review found that the previous entry claimed fixes the code
  did not hold. The edit script had stopped at a missed anchor after rustfmt reflowed run.rs, so
  run.rs and report.rs were never written. Only `snapshot`'s `.git_exclude(false)` had landed.
- **Landed now:**
  - run.rs fails the landing whole on `verdict.wrote`, not on text;
  - report.rs anchors `/.coverage`, `/htmlcov/`, `/coverage/`, `/.nyc_output/`, and writes
    `exclude` as a new file renamed over it;
  - the LEFTOVERS doc no longer claims the patterns hide anything from the gate.
- **Tests:** a check that prints "a check never writes" passes; a write to `lib/coverage/` under a
  local `coverage/` exclude is still judged. With `.git_exclude(false)` removed, that test fails.
- **Lesson:** after a scripted edit, grep for each change, rather than trusting that the build and
  tests passed.

---

### [2026-10-25T20:28:03+01:00] rimuru — PASS / FAIL / NOT_VERIFIED (#102)
- **Task:** the M7 tail, after the triage. Each end-gate check says PASS, FAIL or NOT_VERIFIED.
- **Files:**
  - gate.rs: `Verdict` and its `word()`; `GateVerdict.unverified`; `passed()` means PASS only.
    A verify line that did not run (`Meta.class` is `None`) or timed out (the new
    `Meta.timed_out`) is unverified; a crash is a failure;
  - run.rs: "could not verify" in the note and the ending; an `Unverified` line in the journal;
  - beats.rs: `Beat::Gate` gains `unverified` and `verdict` (both serde default);
  - the server's `Event::GateVerdict` gains `verdict` and `unverified`; the protocol TS is
    regenerated;
  - the CLI says "not verified";
  - docs 03 and 04.
- **Test:** `a_check_that_gave_no_result_is_not_verified`: unrun gives NOT_VERIFIED, true gives
  PASS, false gives FAIL, SIGSEGV gives FAIL.
- **Check:** after the scripted edits, each anchor was confirmed to land.

---

### [2026-10-25T20:34:14+01:00] rimuru — #176's review: Fable
- **Fable found:**
  - the worth card and the stuck streak (worth.rs) ignored `NOT_VERIFIED`, so a "NOT_VERIFIED,
    handed back" reset a FAIL streak: a body with hanging tests dodged the three-fails rule;
  - docs 03 left out the new journal words and the refused-line case;
  - a beat from an old journal showed an empty word in the CLI.
- **Fixed:**
  - `NOT_VERIFIED` counts as a fail on the card and in the streak, and any ", handed back" is
    skipped;
  - docs 03 and 06;
  - the server derives the word for old beats.
- **Test:** `a_check_that_gave_no_result_counts_as_a_fail`.

---

### [2026-10-25T20:40:06+01:00] rimuru — never routed above the parent (#42)
- **Task:** the M7 tail. #42: a child may never be routed above its parent's tier.
- **Files:**
  - route.rs: `Work.ceiling`; `capped` holds floor, default and ceiling at or under it;
    `tier_of` reads a model's tier back (the highest tier riding it, else the race's default);
  - commission.rs `route_child` passes the parent's tier;
  - docs 02.
- **Test:** `a_body_is_never_routed_above_its_parent`: hard work stops at strong, then at fast;
  plus `tier_of`.

---

### [2026-10-25T20:45:04+01:00] rimuru — #177's review: Fable
- **Fable found:**
  - a parent pinned to a model outside the tier table read as its race default: children could be
    capped too low, or ride above a cheap pinned parent;
  - docs 02: the Dark Elf floor and the cap read as if they contradicted each other.
- **Fixed:**
  - `tier_of` returns `Option`; an unknown model caps nothing (a pin is a choice, not a tier);
  - docs: the cap applies in every office and outranks the Dark Elf's floor.
- **Test:** a pinned parent leaves hard work at frontier.

---

### [2026-10-25T20:53:25+01:00] rimuru — unrun checks are blameless
- **Task:** a side note: on a machine without the sandbox, every verify line goes unrun. #176
  counted that NOT_VERIFIED as a fail, so every body would be marked failing and stuck, and the
  council could split or retire it, for the machine's fault.
- **Files:**
  - gate.rs: `GateVerdict.timed_out` and `unrun()`: NOT_VERIFIED with no timeout;
  - run.rs: an unrun landing ends at once, with no retry, journaled `NOT_VERIFIED, unrun`, and
    tells the human to make the sandbox work or set `sandbox.allow_unguarded`;
  - worth.rs: `, unrun` counts neither way and breaks no streak. A timeout still counts as a fail;
  - docs 03/06.
- **Tests:** right_author (`unrun()`) and worth (unrun entries leave the streak and the card as
  they were).

---

### [2026-10-25T20:59:21+01:00] rimuru — #178's review: Fable
- **Fable found:** `unrun()` fired on any check that did not run, refusals included. A body could
  symlink a path its check reads to a credential, get the check refused, and end blameless. The
  "fix the sandbox" hint was then wrong too.
- **Fixed:** blameless means only that the gate's context had no kernel guard and none allowed
  unguarded (`GateVerdict.unguarded`). A refused, denied or timed-out line on a guarded machine
  stays NOT_VERIFIED and counts. The `timed_out` plumbing from this branch is gone. Docs 03.
- **Test:** a failure beside a refusal is FAIL and not unrun; a refusal alone is not unrun.

---

### [2026-10-25T21:04:48+01:00] rimuru — prose-claim (#131, one of three)
- **Task:** the M7 tail. #131's `prose-claim` is the one fully specified of the three
  missing codes; `heir-dead` and `orphan-mind` wait on the human, since no heir field and no
  shared mark exist yet.
- **Files:**
  - findings.rs `prose_claims(roster, world)`: backticked path-like words in a living Slime's
    non-field lines that exist in the world, outside `.mnemorph/`, and are not covered by its
    Territory. One low finding per path;
  - `doc::claim` is pub(crate), so mentions read by the claim rule;
  - mnemorphd adds the code to housekeep's findings;
  - docs 09.
- **Test:** `a_path_named_in_prose_and_not_claimed_is_a_prose_claim`.

---

### [2026-10-25T21:09:41+01:00] rimuru — #179's review: Fable
- **Correction:** the previous entry said housekeep reports `prose-claim`. It is `onto check`
  (mnemorphd's `check` command); housekeep reports no findings.
- **Fable found:**
  - a directory named without its slash read as unclaimed;
  - paths no Slime can claim (workspace, throne, harness, human soil) still fired, with wrong
    advice.
- **Fixed:**
  - a directory gets its slash;
  - only soil `explain` rules Zone, Domain or Unclaimed counts, which subsumes the `.mnemorph/`
    check;
  - docs 09.
- **Test:** extended with `src/a` and `./Cargo.toml`.

---

### [2026-10-25T21:54:57+01:00] rimuru — heirs and shared Minds: #131 whole
- **Task:** the human: "add the fields now and continue".
- **Files:**
  - doc.rs: the `Heirs` and `Heir of` fields;
  - relief.rs: the retired doc gains `Heirs` beside `Retired` (in its own line endings), and each
    young carries `Heir of` instead of a prose line;
  - findings.rs:
    - `heir-dead` (high): an heir with no doc, or retired with no living line below it. A
      relieved heir lives on in its own heirs; cycle-safe;
    - `mind_findings` gives `orphan-mind` (medium): an entry of `.mnemorph/minds/` (a directory
      with a README, or a `.md` file) that no living creature wears and whose header does not
      say `- **Shared:** yes`;
  - mnemorphd adds both to `onto check`;
  - docs 03 and 09.
- **Tests:**
  - relief's fields round-trip;
  - `a_dead_line_of_heirs_is_heir_dead`;
  - `a_mind_nobody_wears_and_not_shared_is_an_orphan`.

---

### [2026-10-25T22:00:19+01:00] rimuru — #180's review: Fable, and a side note
- **Found:** Fable, and a side note to the human, both found that `heir-dead` pointed at the
  retired creature's whole old ground. High blocks landings under its paths, so a healthy
  sibling's half, or ground the human re-granted, would fail for a dead line not its own. Fable
  also found the heirs line's ending was taken from the whole doc, and a `minds/README.md` index
  read as an orphan.
- **Fixed** (the human: "do it in a smart way and effective way"):
  - the finding's paths are only unkept ground: the dead heir's own claims, or, with its doc gone,
    the old ground's claims that no living creature owns or claims inside;
  - high while some ground is unkept, which blocks only paths nobody owns; low once all are kept;
  - the inner ending comes from the line the heirs follow;
  - the index is skipped;
  - docs 09.
- **Test:** a sibling's half untouched (`src/u/b/` only); re-granted ground gives low; the
  missing-heir fallback.

---

### [2026-10-25T22:45:48+01:00] rimuru — the bench report (#96, in part)
- **Task:** the human: "ok go on". #96, honest measurement.
- **Files:**
  - bench/report.py:
    - the fixed matrix (missing, extra or doubled is an error; `--partial` names the gaps);
    - success needs verify plus exit 0, no timeout, and a valid hierarchy;
    - a paired bootstrap over task groups (10 000, seed 96) for hierarchy minus solo, stating the
      estimand and the group count;
    - no aggregate score;
  - run.py `--check` refuses a task whose world holds its verify or an answer file;
  - `just bench`, part of `just check`, runs with `PYTHONDONTWRITEBYTECODE=1`, since a check never
    writes;
  - bench/test_report.py; the README.
- **Built in a worktree:** ~/kaginari/tempest-bench, so the queued measurement in the main
  checkout keeps its own run.py.
- **Open on #96:** the with/without-law arm needs a harness switch that omits the fixed
  per-session prompt; that is a design change of its own.
- **Interim (33 of 60, partial):**
  - success: solo 94% (one broke the rules), hierarchy 88%;
  - hierarchy costs about 4.6× the calls, 5.4× the wt and 10× the time;
  - the difference is −0.06, interval [−0.25, +0.125], 16 groups.

---

### [2026-10-25T22:50:46+01:00] rimuru — #181's review: Fable
- **Fable found:**
  - exit 0 means "ended answered", not "kept every rule": a checkpoint, an escalation and a
    failure all exit 1, and a gate refusal the body recovered from leaves 0. The real case was an
    `11-max-of-bug` solo run that escalated after three failed acts with a correct file;
  - the bootstrap is thin with few discordant pairs;
  - the leak regex was too broad;
  - the justfile's `layers` comment was displaced.
- **Fixed:**
  - the wording is now "ended answered", with the column "correct but unanswered";
  - discordant pairs are reported, with a warning under ten;
  - the leak guard uses a full-name match;
  - the recipe moved; the README.
- **Follow-up on #96:** record each run's `turnEnded` ending and gate events from `--json`, so a
  rule broken on the way is judged, not only how the run ended.

---

### [2026-10-25T23:05:06+01:00] rimuru — the bench reads why a run ended
- **Task:** a side note: the report read only the exit code, so a provider or machine error with
  a correct file would count as a failed task. The human: "can you fix".
- **Files:**
  - report.py `cause()` reads the recorded `ending`, or for older rows the CLI's
    `mnemorph: <ending>` stderr line:
    - escalated, checkpoint, gate, timeout and never-delegated are failed tasks;
    - machine (no kernel guard), provider and interrupted measured nothing: they are listed to
      rerun, never counted, and a later row stands in for them;
    - a per-arm "not a success" breakdown;
  - run.py records `ending` and takes `--redo-invalid`;
  - tests; the README.
- **On the data:** the one solo failure is `escalate: more than two failed acts in a row`, the
  harness's own three-fails rule, so it rightly counts as a failure. Hierarchy's two are wrong
  answers. Nothing is infrastructure so far.

---

### [2026-10-25T23:10:05+01:00] rimuru — #182's review: Fable
- **Fable found:**
  - "could not verify twice" (a body's check hanging) matched the no-guard prefix, so it was rerun
    and never counted;
  - a model refusal fell to provider, so a refusal was rerun until the model complied;
  - the bench's 30-minute kill could not tell slow work from a stall;
  - a misleading `ending` on a kill.
- **Fixed:**
  - the exact no-guard phrase (with its colon);
  - `refused` counts;
  - a malformed conversation is a "harness bug", rerun but flagged loudly;
  - the bench kills at 65 minutes, past the core's one-hour clock, so slow work ends as
    `checkpoint` (counted) and a kill is a hang (rerun);
  - tests; the README.
- **Lesson:** a cause prefix shared by a machine stop and a body stop is a dodge. Match the exact
  phrase, and test both.

---

### [2026-10-25T23:51:36+01:00] rimuru — the sandbox reads uv's interpreters and tools
- **Found by:** the large bench's smoke run on sqlglot. It needs Python 3.9+, and the bench puts
  uv's 3.12 first on PATH. Landlock refused to execute it (`Permission denied`, exit 126): uv's
  interpreters under `~/.local/share/uv/python` are named by no rule. `uv tool` binaries, which
  `.local/bin` links to, failed the same way.
- **Fixed:** `HOME_TOOLCHAINS` gains `.local/share/uv/python` and `.local/share/uv/tools`, never
  `.local/share/uv` whole, since its `credentials/` holds `uv auth` logins. Docs 05.
- **Test:** sandbox `tests/policy.rs`: both are read, and no rule names a directory above the
  credentials, or the credentials themselves.

---

### [2026-10-25T23:57:12+01:00] rimuru — #183's security review: Opus
- **Opus found:** the change holds. No secrets in uv's python/tools; read and execute only;
  symlinks resolve inside.
  - LOW: uv's credentials were closed only because no rule happened to name them;
  - LOW: the test checked equality, not containment;
  - pre-existing: `.cache` (allowed) holds Hugging Face's token and git's credential socket.
- **Fixed:**
  - `HOME_CREDENTIALS` gains `.local/share/uv/credentials`, `.cache/huggingface` and
    `.cache/git`: the classifier refuses them, and Seatbelt hides them;
  - a containment test showed that `.cargo/credentials*` is likewise held by the readable
    `.cargo` on Linux, an older gap;
  - the test names the toolchain-held credentials and requires them hidden where the platform
    can;
  - the comment says Linux cannot hide them.
- **Open:** narrow `.cargo` and `.cache` on Linux to what builds need (issue).

---

### [2026-10-25T23:59:50+01:00] rimuru — #183: Fable checked Opus
- **Fable confirmed Opus's review:** safe to merge.
- **Refinements made:**
  - hide only Hugging Face's `token` and `stored_tokens`, not its model cache;
  - name the legacy `~/.git-credential-cache/`.

---

### [2026-10-26T00:18:14+01:00] rimuru — the attic and the stress jump (#86)
- **Task:** the M7 tail, #86's four bugs.
- **Files:**
  - attic.rs:
    - `holds` is false for a gitlink, so a nested repository is never promised as kept;
    - a `.git` file (worktree, submodule) is kept as `dotgit-file` and put back by `repair_git`;
    - an exclusive lock (`File::lock`) around every snapshot and restore;
  - run.rs `fold_under`: at the stress line, fold first whatever the batch if that brings the
    next request under the line. This covers a jump from under 60%, and a resume above the line;
  - docs 03 and 05.
- **Tests:**
  - attic: a nested repository, a git file, four concurrent snapshotters;
  - loop: `a_jump_past_the_stress_line_folds_before_it_checkpoints`, with a scripted-usage
    model. It fails with the fold taken out.

---

### [2026-10-26T00:23:45+01:00] rimuru — #185's review: Fable
- **Fable found:**
  - the shell path still promised a nested repository: `deleted_since` listed gitlinks, so
    `rm -rf vendor/sub` read "kept in the attic";
  - a stale `dotgit-file` could win over the mirror, and the reverse flip failed;
  - a resume double-counted fold savings.
- **Fixed:**
  - `gone_since` reads modes: `deleted_since` gives kept files only, and `lost_since` gives
    gitlinks, which the shell reports as NOT kept;
  - `keep_git` keeps only `.git`'s current form;
  - a resume past the fold mark starts folded. No dedicated test: it is the same check as
    `ease_context`'s.
- **Tests:** `a_deleted_nested_repository_is_reported_lost`, `only_the_current_form_of_git_is_kept`.
### [2026-10-26T00:22:00+01:00] rimuru — solo gets the chain's steps
- **Task:** a side note: on the large bench, solo stops at 40 steps while a hierarchy's bodies
  each bring their own. The human: "adding number of steps seems right, you should add it to
  solo mode".
- **Files:**
  - config: `steps.solo` and `solo_steps()`, defaulting to `SOLO_STEPS` = 100 (Elf 30 + Orc 30 +
    Slime 40, the shortest chain solo stands for); the budget's caps stop it whatever steps are
    left;
  - solo.rs `configure` sets the session body's steps; commissioned children keep their race's;
  - docs 03.
- **Test:** `the_configuration_reaches_the_run`: 150 from the config; 100 unset.
- **Order agreed:** the human asked whether to finish Mnemorph before the bench. Agreed. The
  running large and small runs finish as a "before" baseline; no new suites until memory (M8) and
  the ontology's mesh land; then the campaign suite and an OpenCode adapter, before and after.

---

### [2026-10-26T00:34:33+01:00] rimuru — #186's review: Fable
- **Fable found:**
  - a dispatching Rimuru got 100 steps too;
  - `actBudget` (40 turns with nothing landed) would still stall solo at 41, leaving the raise dead
    for the read-heavy work that needs it;
  - "the budget's caps stop it" was false without `budget.*`.
- **Fixed:**
  - open.rs `session_steps` sets `steps.solo` only with no lanes; `configure` no longer touches
    steps;
  - `Stress::for_steps`: the act budget is the run's steps when it has more than 40, and runs
    reset with it;
  - honest wording: without `budget.*`, steps, the clock, the context line and the stall guards
    stop it;
  - docs 03;
  - `ids()` was extracted to keep `open_with` under 100 lines.
- **Tests:** `solo_steps` 150 and 100; `Stress::for_steps(100)` takes 100 turns and stalls at
  101; a 30-step body keeps 40. The lanes condition has no end-to-end test (Session hides its
  limits).

---

### [2026-10-26T00:58:33+01:00] rimuru — append-only requests (#38)
- **Task:** the M7 tail, #38.
- **Finding:** nothing moves the prefix today. The system prompt is built once, the model is
  fixed per session, no date sits in the prompt, and asks, wakes and nudges append.
- **Files:**
  - compact.rs `fingerprints` (per block, role marks, breakpoints aside) and `moved`;
  - run.rs `watch_prefix` (via `next_request`) warns when a request rewrote what the last one
    sent with no fold;
  - docs 06.
- **Tests:**
  - `every_request_only_appends_to_the_one_before`: two asks, four tools, system and tools
    identical, no warning;
  - `a_rewritten_or_dropped_message_moves_the_prefix`: an appended message and an extended last
    message are fine; a rewritten or dropped one moved.
- **Left open on #38:** the thinking-strip recovery has no trigger until a mid-session model
  switch exists.

---

### [2026-10-26T01:04:04+01:00] rimuru — #187's review: Fable
- **Fable found:** no false positive. An strace of the two folding tests showed 8 folds and no
  "prefix moved".
- **Taken:**
  - the watch covers the system prompt and the tools too;
  - fingerprints hash fields directly, with no clone or Debug pass;
  - the long-run compaction test asserts a fold never reads as a moved prefix;
  - a test comment corrected; docs 06.
### [2026-10-26T01:06:10+01:00] rimuru — the provenance lint (#10)
- **Task:** #10's lesson "provenance lives only in log.md; a lint refuses dates and change notes
  in law, canon and creature docs".
- **Files:**
  - findings.rs `provenance(roster, world)`: `doc-provenance` (medium) for an ISO date or a
    change-note phrase in `.mnemorph/law.md` or any creature doc, fenced code aside, one per line;
  - `has_date` is a word-bounded `YYYY-MM-DD` (19xx/20xx), with no regex dependency;
  - mnemorphd adds it to `onto check`;
  - docs 09.
- **Test:** `a_date_or_a_change_note_in_a_living_doc_is_doc_provenance`. On Mnemorph's own world
  it finds nothing.

---

### [2026-10-26T01:13:53+01:00] rimuru — #188's review: Fable
- **Fable found:**
  - `as of` matched inside "schemas of";
  - `'s order`, `added by` and `changed on` are mostly legitimate prose;
  - inline backticks were not skipped.
- **Fixed:**
  - `has_phrase` matches words of their own;
  - the three prose-prone phrases are dropped;
  - backtick spans are skipped like fences;
  - docs 09;
  - the test now carries those false positives and stays at 2 findings.
- **Lesson again:** a scripted edit stopped at a missed anchor, and the test still passed because
  its own edit had not landed yet. Each edit was grepped before trusting the run.
### [2026-10-26T01:11:17+01:00] rimuru — refusals name the owner and the ways on (#10)
- **Task:** #10's lesson "an out-of-territory refusal names the owner and carries the three arms
  (widen, birth, route)".
- **Files:**
  - tools: `OwnerOf` and `Context::with_owner_of`; `ways_on` adds the owner and the arms to the
    soil refusal. The tools stay below the world: the lookup is a closure;
  - commission.rs `context_for` fills it from `explain`: a domain is routed, never written, so
    ground only a domain covers reads as nobody's (birth);
  - docs 02.
- **Test:** `a_refusal_names_the_owner_and_the_ways_on`: a sibling's file routes, a domain-only
  file is a birth, and the human's file names no route.

---

### [2026-10-26T01:17:41+01:00] rimuru — #189's review: Fable
- **Fable found:**
  - "the parent commissions X" holds only for a same-parent sibling;
  - a seat and another body's scratch read as route targets;
  - "Rimuru founds its keeper" names no mechanism (the ontology says Rimuru routes its birth).
- **Fixed:** the loop writes the whole sentence (`ways_on` in commission.rs, by owner and rule):
  - a sibling routes by the parent; any other route goes up to a common commander;
  - a seat, or a scratch, is never routed;
  - nobody's ground is a birth Rimuru routes;
  - the human's and the harness's soil are theirs;
  - tools only append; docs 02.
- **Test:** sibling, birth, scratch and the human's.

---

### [2026-10-26T01:22:59+01:00] rimuru — a foreign file survives a cleanup turn (#10)
- **Task:** #10's lesson "no body deletes or reverts what it did not write. Test: a foreign
  untracked file survives a cleanup turn".
- **Finding:** it already holds. A Slime's `rm -rf notes` runs and the shell says the attic kept
  the file; the Slime's gate fails right-author on `notes/human.md`; the landing is put back whole.
  The notes return, and the Slime's own write goes with them.
- **Test:** commission.rs `a_foreign_untracked_file_survives_a_cleanup_turn` asserts each step of
  that chain from the journal.
- **False start:** the first version passed vacuously. A guess that the classifier refused the
  `rm` was wrong; reading the journal showed the real chain.
### [2026-10-26T01:28:43+01:00] rimuru — writes attributed per body (#10)
- **Task:** #10's lesson "write attribution is per body (per sandbox)".
- **Finding:** tool calls run one after another in a session, so the risk is a second session on
  one world: its writes landed in a child's tree diff, failed the child's right author, and were
  then put back, destroying that session's work.
- **Files:**
  - tools `Context::could_have_written`: under a guard, a narrowed body's grant (soil, build,
    scratch nest, own plan); otherwise true;
  - gate.rs: right author judges only what the body could have written;
  - commission.rs `repair` skips another writer's change, judged by the child's own rebuilt
    context with the parent's guard;
  - docs 03.
- **Test:** `another_writers_change_is_not_the_bodys_under_a_guard`, guarded and unguarded. No
  end-to-end put-back test: a scripted run cannot make a second session write mid-run.

---

### [2026-10-26T01:37:34+01:00] rimuru — #191's security review: Opus
- **Opus found:** no full laundering hole. The predicate matches the real grant or is more
  generous, symlinks are resolved, and a missing guard runs nothing.
  - LOW: `repair` rebuilt the grant after the run (a changed `.gitignore` moves the build grant);
  - LOW: skipped changes were named nowhere;
  - INFO: Seatbelt off macOS runs unguarded but read as guarded.
- **Fixed:**
  - the grant is captured before the child runs and passed to `repair`;
  - `repair` returns another writer's changes, and the report says "[another writer changed, left
    as it is: …]";
  - Seatbelt counts as a guard only on macOS;
  - docs 03;
  - `held_before` was extracted to keep `order` under 100 lines.
- **Next:** Fable checks Opus's review.

---

### [2026-10-26T01:45:15+01:00] rimuru — #191: Fable checked Opus
- **Fable confirmed Opus:** safe to merge. A skipped path stays in the dispatcher's view and
  floats up; solo is not narrowed, so nothing is skipped there. One LOW: the second
  `context_for` leaked a `/tmp` scratch dir per commission.
- **Fixed:**
  - `Grant` (`Context::write_grant`) is taken from the child's real context before it runs, with
    no second context;
  - `Context::admits` and the grant share one `admits` function, so the two cannot drift.

---

### [2026-10-26T01:49:58+01:00] rimuru — #191: macOS CI
- **Found:** macOS CI failed the attribution test. The grant stores resolved paths, but the
  predicate compared the path as asked, and macOS temp dirs sit behind `/var` → `/private/var`.
- **Fixed:** `Grant::could_have_written` resolves the asked path with `real()`.
- **Test:** it now also asks through a symlinked alias of the world, so Linux catches the same
  class. It fails without the fix.
- **Note:** #191 was correctly not merged on the red check.

---

### [2026-10-26T02:07:40+01:00] rimuru — M8 rung 1: notes as events
- **Task:** M8 begins, after M7's tail.
- **Files:**
  - mnemorph-memory note.rs: `Polarity` (`dissents`), `Status` (ordered by strength), `By`, and
    `Change` (create, status, confidence); `Event` and `Note`; `fold` orders by (time, id), the
    latest status and confidence stand, a merge's duplicate takes the stronger status, an orphan
    event is set aside, and a second creation is ignored;
  - store.rs: `Tier` (personal, shared, short), `append` and `read` on the ledger's `Journal`
    (one synced line, tolerant read); `inherit` (an heir gets the notes sourced in its ground,
    every heir gets those with no path, the old file is kept); `union_merge` (`merge=union` via
    `.git/info/attributes`, idempotent, never through a link);
  - relief hands notes to heirs on a split; the session open sets union merge;
  - memory → ledger, in check-layers and docs 05; docs 08.
- **Tests:** memory notes.rs (5), relief's notes-follow-the-split.
- **Next rungs:** recall (BM25), the write rule, proofs.

---

### [2026-10-26T02:15:31+01:00] rimuru — #192's review: Fable
- **Fable found:**
  - `union_merge` did nothing in a worktree (`.git` is a file);
  - `inherit`'s '/' guess dropped `.mnemorph/log.md#…` notes and sent `README.md:3` to every
    heir;
  - event ids could collide across machines;
  - the fold depended on line order for colliding keys;
  - `inherit` read sources in file order;
  - a failed `inherit` left relief half-done.
- **Fixed:**
  - git names its attributes path (`rev-parse --git-path`), resolved against the world for git
    before 2.31, whose answer is relative (this machine runs 2.25);
  - a note goes to the heirs whose ground covers its source path (`:line` and `#…` dropped),
    else to all;
  - ids carry a random per-process mark;
  - the fold sorts by a total order;
  - `inherit` reads the fold's source;
  - relief inherits before seating and undoes the heirs' files on failure.
- **Tests:** a real repo and a worktree; journal and root-file sources; colliding events fold the
  same in either order; distinct ids.

---

### [2026-10-26T02:29:47+01:00] rimuru — M8 rung 2: recall
- **Task:** M8 rung 2 (#125: the dissent seat and the declared omissions).
- **Files:**
  - mnemorph-memory text.rs: `words`, `content_words` (stop words aside) and `bm25`, moved from
    world's facts.rs, which now uses them, so there is one implementation;
  - recall.rs: `Weights` (the page's defaults, for config); `Relation` (own 1.0, race 0.6,
    shared 0.3, log 0.1); `recall`:
    - silent with no content words;
    - dormant and tombstoned never recalled;
    - the meaning term alone clears the floor θ;
    - the score adds relation, salience, confidence, freshness (half-life H) and dissent β;
    - one seat for the best dissent; the cut are counted, at most three named;
  - `Note.born`.
- **Tests:** the two roadmap fixtures (a buried correction recalled; a poor match under θ despite
  every prior), silence, own over shared, dormant and tombstoned excluded.
- **Lesson:** a fixture where every note holds the query's words makes BM25 rightly weigh them at
  nothing. The realistic fixture carries notes about other things.
- **Disk:** it filled to 33M mid-build. My dev target (13G) and nine merged worktrees were cleared;
  memory feedback-disk-budget.
- **Small bench complete** (60 runs, measured build 9d17040):
  - success: solo 93%, hierarchy 83%;
  - the difference is −0.10, interval [−0.267, +0.067], 30 groups, 7 discordant;
  - hierarchy costs about 4.4× the calls, 5.2× the wt and 8× the time;
  - `26-readme-usage` hierarchy failed during the disk-full window: rerun it before the
    write-up.

---

### [2026-10-26T02:33:11+01:00] rimuru — #193's review: Fable
- **Fable found:**
  - another creature's proposed note was weighed;
  - the seats differed from the page with three or more dissents in the top k (the page keeps
    k − 2 seats for agreement);
  - the word lists are rebuilt per call;
  - the English stop list was thin.
- **Fixed:**
  - only the asker's own proposed notes are weighed;
  - the seats are the page's, literally (k − 2 agreeing, the best dissent, then the fill);
  - the doc says a caller keeps the words between calls;
  - the stop list is wider and says it is English only.
- **Tests:** the seats with dominant dissent; own-only proposed notes.

---

### [2026-10-26T02:39:49+01:00] rimuru — M8 rung 3: the write rule
- **Task:** M8 rung 3 (#124: a model's words stay personal; four routes promote).
- **Files:**
  - mnemorph-memory write.rs: `Claim<Proposed>` and `Claim<Grounded>`; `Proof`, built only from
    its route's evidence:
    - checked needs a verify line over a non-empty diff;
    - accepted, a verdict naming the claim;
    - corroborated, another creature with disjoint anchors;
    - confirmed, the human;
  - `propose` (personal, proposed, 0.6 / 0.5);
  - `ground` (personal and shared, at the route's confidence: 0.7 checked or confirmed, 0.6
    accepted or corroborated);
  - commission.rs `remember`: an answering child's non-territory `@U` lines land proposed; a
    checked landing grounds them; the report notes it;
  - wire: `UnsaidKind::word` is public.
- **Tests:** memory write.rs (proofs refused without evidence; propose, then ground in both tiers);
  commission (a checked landing grounds; a bare answer stays proposed; territory is not memory).
- **Next:** wire accepted (the parent's verdict naming a claim) and the turn's recalled anchors.

---

### [2026-10-26T02:53:51+01:00] rimuru — #194's review: Fable
- **Fable found:**
  - a false **checked**: `remember` built the proof from the landing, which includes the child's
    commissions. An Orc that wrote nothing itself passes untouched, with no verify line run and no
    journal entry, yet its words were grounded;
  - `append` was public, so the type rule was honour-system;
  - the cite could not be resolved;
  - the docs named `Note<Proposed>`.
- **Fixed:**
  - the run keeps its last passed verdict (`passed_gate`) with the journal heading the gate wrote
    (`log::append_entry`); `GateVerdict.ran` counts the verify lines; `checked_proof` builds only
    from those and cites the heading;
  - memory's `append` and `Tier::journal` are crate-private: outside the crate only `propose` and
    `ground` write;
  - docs 08 say `Claim<…>`;
  - `order_of` was extracted to keep `order` under 100 lines; a stray doc line was removed.
- **Test:** the Orc's claim stays proposed while its Slime's is grounded.
- **Disk:** it fell to 3.1G as stale dev test binaries piled up. The dev target was cleared and is
  rebuilt with `CARGO_INCREMENTAL=0` (4.4G).

---

### [2026-10-26T03:03:14+01:00] rimuru — recall in the loop
- **Task:** reconsolidation (rung 4) needs the anchors each turn recalled, and nothing in the loop
  recalled. Recall is wired first.
- **Files:**
  - memory recall.rs: `Memory` and `gather` (personal as own; shared by owner: own, race or
    shared; a note grounded from personal counted once); `lines` (`@M <id> <scope> <standing>
    <polarity>: <text>`, the cut line, "dissent: none selected");
  - the saturation is retuned against the fixtures to [0.5, 1.0, 1.5]: with [1, 2, 3] a lone
    note matching three content words of four stayed under θ, while a word every note holds stays
    at about 0.04;
  - run.rs `preface` recalls once per ask (`RECALL_K` = 6) and opens the ask with the lines;
  - `Beat::Recall { anchors, cut }` is journaled; server and conversation ignore it;
  - docs 04 and 08.
- **Test:** `an_ask_recalls_what_memory_holds_and_journals_the_anchors`.

---

### [2026-10-26T03:09:12+01:00] rimuru — #195's review: Fable
- **Fable found:** safe to merge.
  - The saturation depends on corpus size: kept as the spec's absolute formula, since both
    normalised alternatives let a word every note holds count as a match; calibration is now an
    issue;
  - unreadable note lines were dropped silently: now warned;
  - the recall row came before its ask: it now follows, via `open_ask`.
- **Test:** the recall row's order and the warning.

---

### [2026-10-26T03:17:54+01:00] rimuru — M8 rung 4: reconsolidation
- **Task:** M8 rung 4 (#126), now that each ask journals its recalled anchors.
- **Files:**
  - memory note.rs: `Link` (note, weight), `Create.links`, `Note.links`;
  - write.rs `revise`: one grounded revise note per failed act (salience 1.0, confidence 0.9, by
    the gate, citing its verdict), linking each non-dissent anchor at 0.9 when cited and 0.4 when
    recalled; shared when any anchor is shared, else personal; nothing when nothing was recalled;
  - `Anchor`;
  - run.rs keeps the ask's anchors (reset per ask); `log_verdict` returns its journal heading;
    `judge` reconsolidates on a run-ending FAIL (a check that wrote, or a second FAIL), never on a
    first failure, NOT_VERIFIED or an unrun machine; the answer naming an anchor makes it cited.
- **Tests:** memory `a_run_ending_failure_revises_what_it_rode_on`; solo
  `a_run_ending_failure_revises_the_note_it_rode_on` (0.9, cited, citing the FAIL entry).
- **Not yet:** confidence gained by three passes after recall.

---

### [2026-10-26T03:25:25+01:00] rimuru — #197's review: Fable
- **Fable found:**
  - one revise per gate line (8 foreign paths gave 8 permanent notes);
  - "cited" read only the final answer;
  - a FAIL then NOT_VERIFIED wrote nothing, while a check that wrote blamed the notes;
  - raw test tails became note text.
- **Fixed:**
  - one revise per failed rule, each line by its first line, capped;
  - cited by scanning the ask's text and its acts' arguments (`ask_from`);
  - the ask keeps its last FAIL (`failed_in_ask`) and reconsolidates by it when the last chance
    is spent, whatever the second verdict; a check that wrote never reconsolidates;
  - docs 08 say so.
- **Tests:** eight lines of one rule give one note; citation through a tool's arguments.

---

### [2026-10-26T03:32:41+01:00] rimuru — M8 rung 5: consolidation and forgetting
- **Task:** M8 rung 5 (#127).
- **Files:**
  - memory consolidate.rs:
    - `distill`: grounded agreeing notes become one schema note (salience 0.8, confidence the
      mean of the four most salient capped at 0.95, linked to each); the two most salient stay
      active, the rest go dormant; dissent is skipped, never merged or slept;
    - `forget`: dormant; abstract (a schema note at max(0.4, s − 0.1) and max(0.3, κ − 0.1),
      the original dormant); tombstone, short tier only (the file is rewritten without the text,
      the event kept);
  - write.rs `cache` writes the short tier;
  - `union_merge` also adds `.mnemorph/memory/short/` to git's local exclude (via `local_line`);
  - docs 08.
- **Tests:** consolidate.rs (3); notes.rs checks the exclude.
- **Not yet:** the desk's distillation trigger, which belongs to evolution.

---

### [2026-10-26T03:38:36+01:00] rimuru — #198's review: Fable
- **Fable found:**
  - a write-rule hole: `distill` and abstract-`forget` grounded caller text on a free proof
    string;
  - the tombstone rewrite was not atomic and left the source and tags;
  - the schema's owner came from the first merged note;
  - the `Link` doc was too narrow;
  - `cache` was written and never read.
- **Fixed:**
  - proofs are built in code (`distilled: <ids>`, `abstracted: <id>`);
  - the tombstone writes a temp file renamed over the old, clearing text, tags and source;
  - `distill` takes its owner;
  - the doc is widened;
  - `gather` reads the body's short tier as its own;
  - the tombstone was extracted to keep `forget` under 100 lines.
- **Tests:** the proofs, and a cache recalled by its own body only.

---

### [2026-10-26T08:04:11+01:00] rimuru — M8 rung 6, first half: the unproven write
- **Task:** M8 rung 6 (#128), the unproven-write finding. The replay loop and lock-in follow in
  their own PR.
- **Files:**
  - memory audit.rs:
    - `why_unproven` and `unproven`, the resolution rule: human always; route `<route>: <what>`;
      gate and import cite something; a record never claims a standing, except a schema whose
      `distilled:`/`abstracted:` ids were grounded in its tier;
    - `resolved`, `findings`, `unfrozen`, `status`;
  - `gather` folds only resolved events and counts the rest (`Memory::unproven`);
  - propose, ground, revise and cache refuse while frozen;
  - the loop warns;
  - `mnemorphd memory status [--world] [--json]` exits 1 while anything is found; the CLI passes
    it through and lists it;
  - world re-exports memory, so mnemorphd keeps to its one edge;
  - docs 08 gain the resolution table and the freeze.
- **Found by the new rule:** #198's abstract-forget still copied the original's proof. Fable's
  fix there missed, because a scripted `replace` without a count check failed silently against
  rustfmt's layout, and so did its test assertion. Both are fixed here.
- **Lesson, again:** every scripted edit asserts its count.
- **Slip:** `npx biome` in a worktree without `node_modules` fetched the unrelated npm package
  `biome` (0.3.3, an env-var manager). It created an empty `~/.biome`, and I removed it and the
  npx cache. Use `pnpm exec` after `pnpm install`, never bare `npx`.
- **Tests:**
  - audit.rs (2);
  - mnemorphd tests/memory.rs, the real binary;
  - args.test.ts;
  - consolidate's abstraction proof.

---

### [2026-10-26T08:12:26+01:00] rimuru — #199's review: Fable
- **Fable found (two lawful writes froze creatures, both reproduced):**
  - a schema of schemas failed, since only non-record groundings counted;
  - `inherit` sent a sourceless schema to every heir but its merged notes to one, freezing the
    others;
  - unresolved events on ownerless shared notes were reported to nobody;
  - commission swallowed the freeze message;
  - docs 08 promised existence checks the code doesn't make;
  - a misplaced doc comment.
- **Fixed:**
  - the grounded set grows to a fixpoint;
  - a linked note follows its linking note's heirs, also to a fixpoint;
  - `status` adds a `(shared, no owner)` row that freezes no one;
  - commission prints the error;
  - docs 08 state the shape rule and that it catches sloppy edits, not forgery;
  - the doc comment is moved back.
- **Tests:**
  - a schema of schemas plus a split, which fails with the old `inherit` (checked);
  - the ownerless row.

---

### [2026-10-26T08:26:08+01:00] rimuru — M8 rung 6, second half: the replay loop and lock-in
- **Task:** M8 rung 6 (#128), the two remaining findings, each with its act.
- **Files:**
  - memory pathology.rs:
    - `fingerprint`: SHA-256 over the query, the anchor ids and the office (adds `sha2` 0.11,
      MIT/Apache);
    - `Trace` and `trace`/`traces`, in machine-local `short/recalls/<name>.jsonl`;
    - `replays`: 3 in the last 5, the current included, nothing landed, no dissent;
    - `locked_in`: a grounded revision linked to a recalled agreeing note, newer than all;
    - `best_dissent`, with the floor set aside;
    - `force`, which takes the lowest agreeing seat;
    - `findings` and `traced`, for status;
  - loop run.rs:
    - `treat` at each recall: lock-in first, then the replay; each act is a warning;
    - escalation adds an `@M replay:` line;
    - `end` appends the trace; `landed_in_ask` is the gate heading; `with_office`;
  - commission passes the office;
  - mnemorphd `memory status` lists the trace findings (`print_memory`, a `pathologies` field in
    `--json`); the exit code still reflects only unproven writes;
  - docs 08.
- **Tests:**
  - pathology.rs (3);
  - loop.rs: a replayed recall escalates on the third ask, and lock-in forces a non-matching
    revision. The lock-in test was checked to fail with the detector disabled.
- **Noted:** `esc_stops_the_child` failed once in the full run at 12 s under load 9.5 from the
  bench containers, and passed three times alone. It is a timing test, not this change.

---

### [2026-10-26T08:36:02+01:00] rimuru — #200's review: Fable
- **Fable found:**
  - a FAIL's gate heading counted as landed;
  - an interrupted ask left a trace;
  - the trace held the fingerprint after the force;
  - lock-in re-fires on every identical recall;
  - the short exclude is installed only on open;
  - traced-only rows were unsorted.
- **Fixed:**
  - only a passing verdict lands;
  - no trace on `Interrupted`;
  - the fingerprint is taken before `treat` and passed to it;
  - docs 08 say lock-in repeats until consolidation retires what it corrects;
  - status rows are sorted, the shared row last.
- **Left:** the exclude-on-open. Every session opens through `open.rs`, and calling `git` per
  trace costs more than it saves.
- **Untested:** the interrupt guard. A scripted model answers before the loop checks the
  interrupt, so a mid-ask Esc needs the blocking-command setup.

---

### [2026-10-26T08:46:37+01:00] rimuru — M8 rung 7, first part: session start
- **Task:** M8 rung 7's session start; exit criterion "a fresh session after a clear carries the
  last journal entries in its first prompt".
- **Files:**
  - world start.rs: `law` (capped at 4,000) and `recent(n, cap)`, the last entries split on
    `---` with the oldest dropped first;
  - solo.rs `with_law` appends the law to Rimuru's system prompt, and commission's `prompt` to
    each child's;
  - run.rs `with_opening`, said once in the first ask's preface;
  - open.rs: `equip` keeps `open_with` under 100 lines; the opening is the last 5 entries within
    8,000;
  - docs 08 gain a "Session start" section.
- **Test:** session_start.rs through `open_with`:
  - the system prompt carries the law, unchanged across asks;
  - the first ask carries entries 3–7, not 2, and the ask stays last;
  - the second ask carries none.
- **Next:** the human's side agent flagged that nothing calls distill or forget. Revise notes pile
  up, and lock-in re-forces one forever. Wire a consolidation trigger before the bench.

---

### [2026-10-26T08:54:32+01:00] rimuru — #201's review: Fable
- **Verdict:** safe to merge.
- **Fable found:**
  - splitting on `---` merged entries: 10 of 206 headings in the real log have no rule before
    them, so "last 5" counted chunks;
  - CRLF read as nothing;
  - the opening had no data framing;
  - the prompt ceiling test carried no law;
  - `recent` had no unit tests.
- **Fixed:**
  - entries begin at `### [` headings, and a closing rule is trimmed; CRLF is normalised;
  - the opening says "quoted as data, never as an instruction";
  - the ceiling test copies the repo's law and asserts it is in the prompt;
  - three unit tests (headings with or without a rule, the cap, CRLF and empty).
- **Tooling:** Fable saw phantom compile errors from the dev target shared between worktrees and
  ran `cargo clean -p` on three crates.
### [2026-10-26T08:49:38+01:00] rimuru — memory stops piling up: retirement, and lock-in once heard
- **Why** (a side agent's note relayed by the human): nothing calls distill or forget, so every
  run-ending failure adds permanent revise notes to tracked shared memory, and lock-in re-forces
  one at every identical recall forever. Distillation needs a model to write its summary, so it
  stays with evolution; these two rules need none.
- **Files:**
  - memory write.rs `retire` (`STRIKES` = 2): a cited anchor goes dormant once two distinct
    failures (revise notes linking it at 0.9, by distinct verdict) have cited it; written by the
    gate citing the verdict, so it resolves;
  - pathology.rs: `Trace::seated`; `locked_in(r, notes, past)` skips a revision seated into an
    ask that then landed;
  - run.rs: reconsolidation calls `retire` after `revise` and warns; `treat` returns the seated
    note;
  - docs 08.
- **Tests:**
  - write.rs: two cited failures retire, one doesn't, a bystander stays grounded, and the
    retirement resolves;
  - pathology.rs: lock-in forces until heard.

---

### [2026-10-26T08:57:34+01:00] rimuru — #202's review: Fable
- **Verdict:** safe to merge as a conservative first cut.
- **Fable found:**
  - personal anchors never accrued strikes, since `revise` writes shared when any target is
    shared but `retire` counted the anchor's own tier;
  - another creature's frozen shared note could be retired;
  - a woken note re-died on one failure;
  - any revise note counted, whatever its status.
- **Fixed:**
  - strikes are counted over shared plus the asker's own tier, from grounded revise notes born
    after the note's last grounded status;
  - a shared note whose owner is frozen is skipped;
  - docs 08 cover waking by a grounded status, "heard" by either act, and per-machine traces.
- **Test:** a personal note retires through shared strikes; after a human waking, one more
  failure doesn't retire it.

---

### [2026-10-26T09:07:27+01:00] rimuru — retirement weighs evidence, never company
- **Why** (the human, relaying a side agent: "two failed runs can retire a correct note … can you
  do it in an effective agent way and smart way"): two-strikes blamed a note for any failure it
  sat beside. A weak bench model fails often, and correct notes would have been retired.
- **The rule now:**
  - **attribution:** a failure strikes only when its failed lines share a specific term with the
    note (path, file, identifier; `file:line` meets the file, a path its file name);
  - **counter-evidence:** a passing, landed ask credits the notes it cited (`confirm`, Gate
    Confidence events citing `passed:`; every third pass adds +0.05, as page 08 already promised
    and nothing did);
  - **doubt first:** a first attributable failure takes 0.1 of confidence (floor 0.3);
  - **retire** only when two or more distinct attributable failures outnumber passes, since the
    last grounding;
  - both copies (the creature's and the shared) move together;
  - **review:** `memory status` lists retired notes with their undo; `mnemorph memory wake
    <note>` is the human's alone.
- **Found by the tests:**
  - a same-millisecond grounding hid the strike (now `>=`);
  - `utf8.sql::test_x` didn't meet `utf8.sql` (terms now yield their parts);
  - retiring a personal note left its shared copy recalled by others (now both);
  - `retired` listed a note twice (deduplicated).
- **Process:** #202's queued merge was stopped first, so the blunt rule never reached main.

---

### [2026-10-26T09:15:58+01:00] rimuru — #202's second review: Fable
- **Fable found:**
  - **"cited" was always true** (since #197): `cited_anchors` scanned every message since
    `ask_from`, including the ask's own user message where recall prints the `@M` ids. So every
    recalled note matched itself, `confirm` credited all of them, and reconsolidation's 0.9/0.4
    split was dead;
  - bare numbers and common terms were specific;
  - proposed notes could be struck but never credited;
  - `wake` grounded anything.
- **Fixed:**
  - only `Role::Assistant` messages are scanned (the body's text and its acts' arguments);
  - specific terms drop bare numbers, bare directories, a stock file list and the plain
    seven-letter rule;
  - `confirm` credits proposed notes;
  - `wake` undoes only a gate retirement (`gate_retired`);
  - docs 08.
- **Tests:**
  - solo.rs: a recalled-but-unnamed note links at 0.4. It fails without the role filter
    (checked). The earlier test passed only because its note was also named;
  - text.rs: common terms;
  - write.rs: `wake` refuses a housekeeping-dormant note and a proposed one; a proposed note is
    credited.
- **Lesson:** a test whose positive case also satisfies the bug proves nothing. Every "only when"
  rule needs its negative case.

---

### [2026-10-26T09:08:27+01:00] rimuru — M8 rung 7: the compaction drain
- **Task:** M8 rung 7's drain; exit criterion "a stress-zone run drains instead of dying".
- **Files:**
  - run.rs: `Stop::Drain` at the context line when the window leaves room for twice
    `SALVAGE_TOKENS`, else the plain checkpoint;
  - `drain` runs `last_turn`, now shared with the clock's `salvage` (no tools, 4096 tokens, the
    clock's grace, ledgered);
  - `keep_unsaid` reads the drain's `@U` lines with the wire's own `Report` reader and proposes
    each, tagged with its kind and `drain`;
  - the salvage text gains `[memory: N line(s) kept as proposed notes]`;
  - docs 03.
- **Tests:** loop.rs: a full context drains two `@U` lines into proposed notes, with no tools and
  4096 tokens; at the window's edge, no drain turn is sent.

---

### [2026-10-26T09:20:19+01:00] rimuru — #203's review: Fable
- **Fable found:**
  - **a re-drain loop:** no flag, so a next ask or a resume at a full context drained again,
    another call and duplicate notes;
  - the clock's tenth was too short a grace for 4096 tokens;
  - `@U territory` was kept as memory, against page 08;
  - escalation dropped the drain text;
  - room is a heuristic, and a child's report omits the salvage.
- **Fixed:**
  - `drains`, a counter because clippy caps a struct at three bools; a resume counts `drain`
    stops in the journal; once drained, the run checkpoints plainly;
  - `last_turn` takes its grace: the salvage keeps a tenth of the clock, the drain gets what is
    left up to `DRAIN_GRACE` (120 s);
  - territory goes through `facts::from_unsaid` + `Facts::record`; the salvage line counts
    notes and facts apart;
  - escalation prepends the salvage to the report;
  - docs 03.
- **Left:** the room estimate (a provider error degrades to no salvage) and the child report's
  salvage.
- **Tests:** territory filed and not kept; a run drains once (no second turn, no second note).
- **Tooling:** worktrees sharing one dev target clobber each other's workspace crates (cargo's
  hash ignores the path). `cargo clean -p` the touched crates when switching worktrees.

---

### [2026-10-26T09:34:36+01:00] rimuru — M8 rung 7: the toolbox
- **Task:** M8 rung 7's toolbox, and two exit criteria: each entry metered in tokens of exactly
  what is injected; every command a prompt or hint names exists in the build.
- **Files:**
  - world minds.rs: `load`, `Mind::{injected, price, line}`, `wearable` (the doc's Minds field
    plus shared ones), `manifest`, `HEAVY` = 2,000;
  - loop wear_tool.rs: `wear`, intercepted in `call_one` like `verify`, offered to every body (a
    stable tool tier); a dispatching Rimuru is refused a heavy Mind;
  - `Run::with_manifest`, said with the first ask, from open.rs (rimuru) and `child_run`;
  - mnemorphd tests/commands.rs: every backticked `mnemorph`/`mnemorphd` command in the Rust and
    TS sources is in the CLI's or the core's help (short spans only, no `${`). A planted
    `mnemorph resume` fails it (checked);
  - docs 05 (Minds are world, not memory) and docs 08 ("The toolbox").
- **Found:** docs 03 promises `mnemorph resume <run>`, which no binary ships. Opened #204 rather
  than rewrite the doc unasked.
- **Not done:** prices are estimates at 4 bytes a token. Exact counts need the provider's
  counting API (a network act); the ledger records the real tokens on use.

---

### [2026-10-26T09:46:14+01:00] rimuru — #205's review: Fable
- **Fable found:**
  - **children never got the toolbox.** My scripted edits to `child_run` sat behind a missed
    edit and never ran, while the journal, the PR and docs 08 claimed them;
  - `dispatches` read the throne only;
  - a worn Mind's size was unbounded;
  - `about` took bullets and rules, cut mid-word.
- **Fixed:**
  - `child_run` pushes `WearTool` and calls `with_manifest`;
  - any body holding the commissioner is refused a heavy Mind;
  - `wear` refuses a Mind over half the context left;
  - `about` skips headings, fields, bullets, quotes, fences, tables and rules, and cuts at a word.
- **Tests:**
  - commission.rs: a commissioned Elf's first ask lists the Mind and its tools have `wear`;
  - wear_tool.rs: the room cap.
- **Lesson, third time:** a script that stops at its first miss leaves the edits after it
  unapplied, and a passing build says nothing about them. After any scripted batch, grep for each
  intended change before claiming it.
### [2026-10-26T09:44:07+01:00] rimuru — a note is cited by use, not only by its id
- **Why** (the human, relaying a side agent): since #202, "cited" means the body typed the
  note's long id, which bodies rarely do. Doubt, credit and retirement would almost never fire,
  and corrections would keep piling up.
- **Now:**
  - an ask cites a recalled note when its own text or acts' arguments name the id, or touch a
    specific term the note names (`anchor_terms` from `specific_terms`; bare directories and
    stock files still don't count);
  - recall's header asks bodies to name the ids they rely on;
  - each trace counts `recalled` and `cited`, and `memory status` prints the rate, so the bench
    can show whether the rules fire.
- **Tests:**
  - solo.rs: three notes, cited by id 0.9, by use 0.9, untouched 0.4; it fails without
    cite-by-use (checked);
  - pathology.rs: the cite-rate line.
- **Tooling:** a reviewer building in the shared dev target clobbered this build again, so this
  branch builds in a private `tempest-cited-target`, deleted after merge.

---

### [2026-10-26T09:50:41+01:00] rimuru — #206's review: Fable
- **Fable found:** "use" was too broad. Every term in the body's text and every tool input
  counted: reads, greps, test commands, the ask echoed back. Notes on the task's target file
  would be credited on every pass and never retired, trading a dead rule for an inflated one.
  The header invited listing every id.
- **Fixed:**
  - use means change: only the paths of write, edit, multiedit, delete, move and patch acts
    (`changed_paths`);
  - terms the ask itself names are dropped;
  - the header asks for "the id of a note that changes what you do, and only that";
  - docs 08.
- **Tests:** solo.rs now holds five notes. Two are cited at 0.9 (by id, by changing the file).
  Three stay at 0.4: one changed but named in the ask, one only read, one untouched. The broad
  rule would cite the read-only note through `read`'s arguments.
- **Disk:** it reached 1.4G free. The private cited target (6.6G) was deleted, and every build
  is back on the one shared target, cleaned per crate when switching worktrees.

---

### [2026-10-26T09:59:55+01:00] rimuru — M8: the accepted route
- **Task:** wire the **accepted** route (`Proof::accepted` existed, uncalled); exit criterion "a
  findings-only commission's accepted claims reach shared memory with no test run, and its
  rejected ones stay personal".
- **Files:**
  - memory write.rs `proposed(world, owner, id)` rebuilds a stored proposed claim as
    `Claim<Proposed>`, so grounding still goes through the typestate;
  - commission.rs `remember` lists each claim left proposed, `id «text»`, with how to accept;
  - run.rs: `commissioned` (the bodies this ask ordered, reset per ask); `accept` runs at `end`
    on an answered ask, and each `@V accept <id>` grounds the matching proposed claim of a
    commissioned body, citing "<body> accepted it in run <run>: <line>", or warns it named
    nothing;
  - model.rs `ScriptedModel::then` lets a test model queue a reply from what it was shown;
  - docs 08.
- **Test:** commission.rs. An `Accepting` model reads the minted id from the Elf's report and
  accepts one of two claims. The accepted one is grounded and shared at 0.6 with its proof; the
  other stays personal and proposed. It fails without `accept` (checked).

---

### [2026-10-26T10:10:12+01:00] rimuru — #207's review: Fable
- **Fable found:**
  - **a body could ground its own words:** `commissioned` took the order's `to` before the order
    ran, so a refused order to itself qualified it, and with a drained claim's id from recall it
    could accept its own claim;
  - **the ids were cut first:** the offer rode the `other` prose line, which `emit` cuts first
    past 2048 bytes;
  - the id parse was fragile;
  - the whole model line went into the proof.
- **Fixed:**
  - `Order::offered` carries exactly the `(owner, id)` pairs a report offered; the run keeps them
    per ask (`acceptable`), and `accept` grounds only those;
  - the offer is a hole (`accept? <id> «text» — … @V accept <id>`), which is never cut;
  - `accept` reads every id on a line, trims punctuation and warns on a bare line;
  - the proof cites `<body> accepted <id> in run <run>`;
  - docs 08.
- **Tests:** commission.rs: a body can't accept its own claim through a refused self-order; a
  report long past the cut still offers and grounds.

---

### [2026-10-26T10:20:01+01:00] rimuru — M8: checked observes only its diff; the 50-landing exit run
- **Task:** M8's last exit criterion, the 50-turn write-rule run. It needs "none of its claims
  about the world beyond the diff grounds through **checked**", and the code grounded every claim
  of a passing landing.
- **Files:**
  - memory write.rs: `Proof::checked(verdict, verify_lines, changed: &[String])` keeps the
    changed paths' specific terms as `scope`; `Proof::covers(text, source)`; `ground` refuses a
    checked proof for a claim it doesn't cover;
  - commission.rs: `checked_proof` passes the changed paths; `remember` grounds only covered
    claims, and the rest stay proposed and are offered for acceptance (#207);
  - docs 08.
- **Tests:**
  - commission.rs `write_rule_tests::fifty_landings_…` drives the loop's own `checked_proof` and
    `remember` over 50 landings (one diff claim and one world claim each; every fifth diff claim
    wrong; verify catches 9 of 10, and a caught landing ends failed). Shared memory holds exactly
    41 notes, all checked proofs, exactly one wrong (1/41), no world claim, and every event
    resolves. Without the scope rule shared holds 82 (checked);
  - commission.rs: the landing test now has a diff claim (grounded) and a world claim (proposed);
  - write.rs: the new signature.
- **Note:** this is a unit-level drive of the post-landing path, not 50 scripted model turns.
  The gate itself is covered by the loop's gate tests.

---

### [2026-10-26T10:22:14+01:00] rimuru — #208: claims name their file; status counts what is shared
- **Why** (the human, relaying a side agent): with checked scoped to the diff, a cheap model's
  unnamed claims never reach shared memory, so the bench could show little from memory.
- **Done:**
  - the child prompt asks `@U law <path> <fact>` for a rule of a changed file, and the drain asks
    to name the file, so the work's own tests can prove the claim;
  - `mnemorphd memory status` opens with "memory: N note(s) shared and grounded; personal:
    P proposed, G grounded" (`standing`);
  - docs 08.
- **Tests:** mnemorphd tests/memory.rs expect the standing line.
- **Noted:** `the_clock_reaches_a_running_command` failed once at load 8.4 and passed twice
  alone in 0.5 s; it is a timing test.

---

### [2026-10-26T10:25:27+01:00] rimuru — #208's review: Fable
- **Verdict:** safe to merge after the doc and comment fixes.
- **Fixed:**
  - docs 08's example claim names its path, and the page says a diff of common files observes
    nothing;
  - the scope adds each changed path's directories (`src/parser/mod.rs` is in `src/parser`);
  - `remember`'s doc comment is corrected, and it passes the claim's source to `covers`;
  - the exit test's doc says it bypasses the gate and the model, and that its ratio is by
    construction;
  - a memory-level test of `covers` and `ground`'s refusal.
- **Left:** a world claim that names a changed file still grounds; that is inherent without
  reading claims.

---

### [2026-10-26T10:54:36+01:00] rimuru — #41: structured compaction
- **Task:** #41, chosen by the human as "the mesh" to do before the bench. At the context line a
  run compacted only by folding and then stopped, so long runs lost their steps.
- **Files:**
  - compact.rs: `KEEP_BYTES` (40,000); `bytes` (now public); `tail_from` (the earliest
    assistant turn whose rest fits); `compacted` (the ask, the summary, the tail);
  - beats.rs: `Beat::Compacted { summary, tail }`, replayed by `conversation`, which keeps the
    last ask's text; the server ignores the beat;
  - run.rs:
    - `Stop::Compact` at the line while `compactions < Limits::compactions` (default
      `COMPACTIONS` = 2) and the window has room;
    - `compact()` runs `SUMMARY_NOTE` on a scratch copy (no tools, ledgered), rebuilds from
      `ask_said`, records the beat, keeps `@U` through `keep_unsaid`, scales the occupancy,
      resets the fold, `ask_from` and the prefix watch;
    - a failed summary drains;
    - `meet` holds the stop dispatch (`ask` was over 100 lines);
  - docs 03.
- **Tests:** loop.rs:
  - a full context compacts and the run answers; the summary turn has no tools; the next
    request leads with the ask and summary and keeps a tool call; the journal has `Compacted`;
    `conversation(journal) == messages` (resume); the discovery is a proposed note;
  - spent compactions drain;
  - the old drain and context tests opt out with `compactions: 0`.

---

### [2026-10-26T11:03:45+01:00] rimuru — #209's review: Fable
- **Verdict:** mergeable after #1; #2 before the bench runs on it.
- **Fable found:**
  1. a resume right after a compaction read the summary turn's full-context reading and
     compacted the small conversation again;
  2. the summary turn missed the cache, because clearing the tools invalidates the whole prefix,
     costing up to 150k tokens at full price twice an ask;
  3. a later ask's compaction lost the Minds' manifest;
  4. a failed compaction drained regardless of `drains`, and said nothing.
- **Fixed:**
  - `Beat::Compacted` gains `head` and `occupancy` (serde default); resume reads the estimate
    when a compaction follows the last model turn;
  - the summary request is the run's own `request()`, with tools and marks, over the messages
    plus the note, then restored;
  - `manifest_said` keeps the manifest, and the head carries it when the ask did not;
  - warnings on a timeout or an empty summary; `meet` drains only when `drains == 0`;
  - docs 03.
- **Tests:**
  - loop.rs: two compactions with replay equality; a resume answers without compacting;
    the summary turn's tools equal the previous turn's;
  - compact.rs: `tail_from`'s assistant start and empty tail.
  - The resume test does not isolate the stale-reading case (its ledger's last row is already
    small); that path is three lines and reviewed.

---

### [2026-10-26T11:38:55+01:00] rimuru — a solo answer keeps its own claims
- **Why** (the human, relaying a side agent, as the paid campaign started): only a commission's
  report turned `@U` into notes. A solo answer kept nothing, so the campaign's `solo-carry` arm
  would carry almost nothing. The Mnemorph arm was stopped a minute in, with no row written;
  OpenCode's arm runs on.
- **Files:**
  - solo.rs: the prompt ends with `@U law <path> <fact>` / `@U colony <fact>` lines;
  - run.rs `remember_own` at `end` on an answered ask, for a run that isn't commissioned (a
    child's claims stay its dispatcher's): the answer is read as a report (`@S DONE` wrap), facts
    are filed, and `commission::remember` runs with a checked proof only from this ask's passing
    gate (`landed_in_ask`);
  - commission.rs: `checked_proof`, `remember` and `file_facts` are now `pub(crate)`;
  - docs 08.
- **Test:** solo.rs: a solo answer's diff claim is grounded and shared, its colony claim stays
  proposed, and shared memory holds one note.

---

### [2026-10-26T11:42:41+01:00] rimuru — #210's review: Fable
- **Verdict:** safe to merge and measure for solo; no double-counting on the solo path.
- **Fixed:** `remember_own` runs only without a commissioner, so a throne with lanes is a
  dispatcher and its children's `@U` lines are not kept twice. This matches docs 08 ("Rimuru
  alone in solo").
- **Bench:** the large suite's world seeds Elves, so its "solo" Rimuru had a commissioner and only
  40 steps (`steps.solo` applies without lanes). The bench's solo modes now run without the roster
  below the throne (Kaginari/bench).

---

### [2026-10-26T12:26:40+01:00] rimuru — stress for a body alone: escalation warns, repin is a streak
- **Why:** the campaign's first Mnemorph solo rows (build 145ded7, sqlglot) stopped on the
  harness's own rules while OpenCode passed the same tasks:
  - task 09 `stall: repin` at 49 calls; repin counted re-reads across the whole run until a
    landing, and three are ordinary exploration in 3 MB of code;
  - task 07 `escalate` at 14 calls; a solo body with no lane below has nowhere to escalate, so
    the run just ended.
- **Files:**
  - stress.rs: `rereads` resets on any act that is not an unchanged re-read; `failed_streak`
    resets once `Escalate` is signalled;
  - run.rs: `heed` (the signal handling, out of `act`, which passed 100 lines) treats `Escalate`
    as a warning note when the run has no commissioner and no commission;
  - docs 03.
- **Tests:**
  - loop.rs: a lone body is warned and answers; a commissioned body still escalates;
  - stress.rs: interleaved re-reads never stall, three back to back do.
- **Not changed:** solo's 100 steps (task 08 stopped there; OpenCode used 137). It is the human's
  call.

---

### [2026-10-26T12:30:57+01:00] rimuru — #211's review: Fable
- **Verdict:** safe to merge for the solo bench.
- **Fable found:** a throne with lanes still dead-ended, because `escalate()` hands off only
  unheld acts and context checkpoints. Also, "said once a streak" overstated it: the warning
  comes every third failure.
- **Fixed:**
  - the warning applies whenever no one is above (`caller.commission` is None), throne with
    lanes included;
  - the comment is corrected;
  - docs 03 drop "with nothing landed" for repin.
- **Test:** commission.rs: a throne with lanes is warned and answers.
- **Fable confirmed:** guts and doom still bound a lone failing body (doom at 15 acts), and a
  re-read alternated with cheap acts is bounded by actBudget.

---

### [2026-10-26T13:00:05+01:00] rimuru — the bench found three bugs: no guard, a dead-end pointer, compaction blocked
- **Context:** the campaign's Mnemorph rows (solo, sqlglot) lost to OpenCode, which passed
  5 of 5. The human: "there's certainly a bug, we should find it."
- **A, the bench build (mine):** after clearing the review target I rebuilt only `mnemorphd`.
  With no `mnemorph-guard` beside it, every shell command was refused ("no kernel guard is in
  force"), so every Mnemorph row since then measured the missing helper. Fixed in the bench: the
  adapter refuses a build without a guard (Kaginari/bench #9).
- **B, the read pointer:** a re-read of an unchanged window answered "that result still holds"
  with no hint that the file goes on. DeepSeek re-read `parser.py` three times (it wanted line
  6136), got the same pointer, and `repin` stopped the run. After a fold the pointer also named
  text no longer sent.
  - Fixed: the pointer comes once and says where the file continues (`Seen::pointed`); asked
    again, the window is shown; a fold or compaction forgets reads (`Context::forget_reads`).
- **C, compaction blocked:** a few 2000-line reads took the context to 230k, past the assumed
  200k window. The room check then skipped compaction, though the provider had just accepted
  that much. A compaction resends what was accepted, so it now needs no room; only the drain does.
- **Tests:**
  - loop.rs: pointer once with the continuation offset, then content; a stuck body still stops on
    the `loop` stall;
  - the window-edge drain test opts out of compaction.

---

### [2026-10-26T13:03:23+01:00] rimuru — #212's review: Fable
- **Verdict:** safe after one fix.
- **Fable found:**
  - a failed compaction drained without the room check, sending a second doomed request at the
    window's edge and spending the run's one drain;
  - `repin` is in practice reached only through the pointer.
- **Fixed:**
  - `meet` drains after a failed compaction only when `drains == 0 && room_to_drain()`, else it
    checkpoints;
  - the edge test is back on default limits, with a failing summary turn;
  - docs 03 say the pointer alone counts, and the `loop` stall and the guts bound a stuck reader.

---

### [2026-10-26T13:29:15+01:00] rimuru — a correction stands where the note it corrects stands
- **Why:** the gap matrix (Fable, `gap-matrix.md`) found that docs 08's head start (0.154 ≈ 0.37
  of meaning) assumes equal relation. A revise note is owned by the failing body, so when A
  recalls its own note corrected by B's run, w_r·Δrel = 0.34·0.7 = 0.238 > 0.154 and the
  correction ranks below. Its failure-line text can also fall under θ.
- **Files:**
  - recall.rs `lend_to_corrections` in `gather`: a revise note takes the closest relation among
    its linked notes and their texts as matching tags, in memory only;
  - docs 08.
- **Test:** recall.rs: B's correction of A's note has relation Own for A and is recalled for A's
  query. It fails without the lending (checked).

---

### [2026-10-26T13:34:32+01:00] rimuru — #213's review: Fable
- **Verdict:** safe to merge.
- **Fable found:**
  - lending from every link let a correction surface on queries about bystanders;
  - copying words into tags raised their document frequency and could dilute the very note the
    correction corrects.
- **Fixed:**
  - lending comes from cited links (weight ≥ 0.9), and from all links only when none is cited;
  - the borrowed words live in `Note::lent`, in memory only; recall scores a note on its own words
    plus `lent`, while document frequency stays the notes' own;
  - docs 08.
- **Tests:**
  - the recall test also asserts the corrected note is still recalled and that the stored tags
    stay `["failure"]`;
  - the lock-in loop test now asserts the revision is recalled (on its borrowed words);
  - the pathology lock-in test clears `lent` to keep covering a revision that misses the floor.

---

### [2026-10-26T13:45:13+01:00] rimuru — the mesh live: facts anchored, shown, settled
- **Why:** the gap matrix's top three mesh gaps were one theme. The outer layer was built and
  tested, but on the live path it had no input (anchors), no output (`@ONTO`) and no closure
  (settle).
- **Files:**
  - world facts.rs: `anchored` (a fact with no anchor is anchored at its `about`); `digest`
    hashes a whole file too;
  - commission.rs + run.rs: both `from_unsaid` callers file `anchored` facts;
  - commission.rs `file_verdicts`: `@V settle #<id>` from an Orc records `Settle` with its proof
    (others are refused and told);
  - run.rs `onto_view`: the `@ONTO` projection for a commissioned body's soil and every existing
    path the ask names (up to 6 paths, 3 facts each), in the preface beside `@M`, only when there
    is a fact;
  - mnemorphd `onto settle <id>`, the human's (a body is refused); the CLI's help;
  - docs 09.
- **Tests:**
  - world facts: anchored, then stale once its file changes;
  - mnemorphd: the human settles, a body and an unknown id are refused;
  - loop: an ask naming a path carries `@ONTO` with the contested fact;
  - commission: an Orc's `@V settle` settles exactly the fact it names.
- **Not done:** the owner's gated landing citing `#id` as a third settle route.

---

### [2026-10-26T13:53:18+01:00] rimuru — the live mesh: Fable's review
- **Fable found:**
  - any Orc could settle any fact;
  - named paths were lowercased, so a real `src/Lexer.rs` was missed, and `/etc/passwd` or `../x`
    passed `exists()` and showed as `@ONTO` lines (a host-path oracle);
  - the seat and scratch crowded the soil's paths;
  - fact text was unbounded and unframed, so a text could fake `[stale]` or a status;
  - keyed directory facts went stale on any file added;
  - docs 09 named the unbuilt route.
- **Fixed:**
  - settle only where the soil of the Orc, or of a creature reporting to it, admits the fact's
    path (the test also refuses a `docs/` fact);
  - named paths keep their case and stay inside the world (absolute and `..` refused), and come
    first; soil without `.mnemorph`;
  - fact text cut to 200 characters with no brackets, framed as data;
  - keyed directory facts unanchored;
  - docs 09.
- **Checked:** the host-path asserts fail with the filter off (`@ONTO /etc/passwd: nobody` shows).

---

### [2026-10-26T14:12:10+01:00] rimuru — the corroborated route, live
- **Why:** the gap matrix's M17. `Proof::corroborated` had no caller, and no claim carried
  anchors (every proposal passed `source: None`), so the ε² route was dead.
- **Files:**
  - memory note.rs: `Create`/`Note` carry `evidence`, serde-defaulted, so older journals read
    and older binaries ignore it;
  - memory write.rs: `propose_on` takes the evidence; `corroborate` finds another creature's
    proposed claim with the same content words, negated alike (`same_claim`; stop words drop
    `not`), whose evidence is non-empty and disjoint, then grounds both, each citing the other;
    the `Seconded` type;
  - loop run.rs: `Run::evidence` = the paths read or changed since the ask (`acts_since_ask`);
  - loop commission.rs: `remember` files the evidence and tries corroboration for what no check
    grounds; the report counts it; a child's evidence reaches `report_for`;
  - docs 08.
- **Not `source`:** a note's `source` is the single place it is about (`covers`, heir routing);
  evidence is a list, and kept apart.
- **Tests:**
  - memory: same evidence, the same creature, a negation, and no evidence are each refused; a
    second voice grounds both. Checked to fail with the negation test off.
  - solo: a world claim grounded by another creature's earlier claim, its evidence the file it
    read.

---

### [2026-10-26T14:17:12+01:00] rimuru — corroboration, Fable's review
- **Fable found:**
  - an echo loop: a parent restating its child's offered claim corroborated it with no
    independent look;
  - one-sided grounding if ours failed after theirs;
  - recall counting both shared copies;
  - one content word sufficed, with no age limit.
- **Fixed:**
  - `corroborate` takes `kin` (`commission::line_of`: up the `reports_to` chain and down);
  - both tiers are checked unfrozen, and ours is grounded first;
  - the older claim is grounded personally only;
  - `SAME_WORDS = 3`, `SECOND_VOICE_DAYS = 30`, newest match first;
  - docs 08 also say only read and changed files count as evidence.
- **Tests:** kin and two-word claims refused; one shared copy.
- **Open (side-agent note):** the route may rarely fire.
  - Solo worlds have no second creature, so it never fires there.
  - Reworded claims don't match.
  - Measure on a world with several creatures before counting on it.

---

### [2026-10-26T14:21:22+01:00] rimuru — memory event ids sort in the order made
- **Found by CI on #215:** `event_id` wrote its counter in plain decimal, and the fold sorts ids
  as strings. Within one millisecond, counter `10` sorted before `9`. A grounding that landed in
  the same millisecond as its note's creation, across a digit boundary, folded first and was lost:
  the note stayed proposed.
- **Fix:** store.rs pads the counter to 16 hex digits. The mark is random per process, so old ids
  never share a mark with new ones, and old journals keep their order.
- **Test:** `ids_of_one_millisecond_sort_in_the_order_made` (1100 ids); checked to fail
  unpadded.

---

### [2026-10-26T14:54:48+01:00] rimuru — the confirmed route, live
- **Why:** the gap matrix's last dead route. `Proof::confirmed` had no caller (the `confirm`
  function is pass-credit, a different act).
- **Files:**
  - mnemorphd: `memory confirm <claim>` finds the proposed claim in any creature's tier and
    grounds it by `Proof::confirmed("the human's word")`; a body is refused, and an id no creature
    proposes is refused with a pointer to `memory status`;
  - `memory status` lists the 10 newest proposed claims per creature, each with its vouch
    command (`proposed_of`, `PROPOSED_SHOWN`);
  - the help in core and CLI;
  - docs 08.
- **Test:** listed with its vouch; a body and an unknown id refused; the human's confirm grounds
  it in shared memory.

---

### [2026-10-26T14:58:31+01:00] rimuru — memory confirm, Fable's review
- **Fable:** safe to merge. A body cannot forge a confirm: it could unset `MNEMORPH_BODY`, but
  the tripwire puts back any change it makes under `.mnemorph/`, the same border as `wake` and
  `onto settle`.
- **Fixed:**
  - `confirm` reads only `.jsonl` tiers, so a stray directory no longer aborts it;
  - `memory status --json` gives proposed claims their own `proposed` field, not `pathologies`
    (`print_memory_json`, moved out of `memory` for its length).
- **Test:** the JSON field and a stray directory.

---

### [2026-10-26T15:08:36+01:00] rimuru — recall's weights in config
- **Why:** the gap matrix's M8b. Docs 08 said every recall weight lives in config; `run.rs`
  passed `Weights::default()`.
- **Files:**
  - config: eleven `recall.*` Number keys and a public `number()`; a Number key now refuses a
    negative float as well as a negative integer (the schema said "zero or more"; `-1.0` passed,
    and the `budget.*_usd` caps had the same hole);
  - loop solo.rs: `recall_weights` (each set key over `Weights::default()`), applied by
    `configure`;
  - `Limits.weights` (`Weights` is now `Copy`); the commissioner carries `weights` to children,
    as it carries `stress_tokens`; run.rs recalls with `self.limits.weights`.
- **Docs 08:** the keys; the dissent seat stated over C_θ (matching dissents), as the code does
  (the matrix's 2.C), with lock-in named for the case it does not cover.
- **Tests:**
  - config: the keys read, unset is None, a negative and a string are refused;
  - loop: the default floor recalls the note; an unreachable floor recalls nothing.

---

### [2026-10-26T15:13:13+01:00] rimuru — recall weights, Fable's review
- **Fable:** every run gets the configured weights; the C_θ docs are exact.
- **Found:**
  - unbounded values made NaN scores that cleared the floor (saturation 0 is `0/0`, a half-life
    of 0, `inf`), and a floor of 0 matched every note;
  - the config→Weights key names were untested;
  - `number()` dropped integers above `i32`.
- **Fixed:**
  - config `recall_bound` in `validate` (finite; saturations and half-life > 0; floor in (0, 1];
    weights ≤ 1), with an error naming the file;
  - `the_configuration_reaches_the_run` sets `[recall]` and reads it back through `configure`;
  - `number()` goes through `u32`;
  - docs 08 state the bounds.

---

### [2026-10-26T15:18:17+01:00] rimuru — the triage fixture: Pr[old fact not in top 3]
- **Why:** gap matrix item 14 (§2.G). The bound on a silent contradiction leans on
  `Pr[old not in top 3]`, which docs 09 said the fixtures measure, and none did.
- **Built:** world tests/facts.rs `TRIAGE_CASES`. 8 paths, each with a reworded new fact, its
  true duplicate, and 6 distractors sharing its words. `the_true_duplicate_is_shown_for_triage`
  asserts no miss and prints the ranks.
- **Measured:** 0 of 8 missed; ranks `[3, 1, 1, 1, 2, 1, 3, 1]`.
  - A first version with 3 distractors also gave 0 of 8, but then the top 3 of 4 is nearly
    guaranteed. I grew it to 6 and checked that each case shows 3 facts.
  - The synonym case ("sixty minutes" for "one hour") ranks 2nd, saved by "entries".
- **Docs 09:** quote the rate and its looseness. 0 of 8 is consistent with a miss rate of one in
  four (the 95% upper bound is ~31%), so the fixture should grow with real misses.

---

### [2026-10-26T16:06:34+01:00] rimuru — the session lands through the end gate
- **Found while asking why the bench's solo worlds ground nothing:** `open.rs` never gave the
  session run an `EndGate`; only commissioned children had one.
  - The solo prompt promised "the end gate checks them", docs 02 promised solo runs under the end
    gate, and `EndGate`'s own doc said `author` is "None in solo".
  - So for any solo session: no gate on landings, no check for deleted or skipped tests, no
    one-chance fix, and `passed_gate()` was always None. No claim could be grounded by the
    checked route; #210's `remember_own` grounding was dead outside tests.
- **Fix:** open.rs `session_gate` builds the run's `EndGate` from the verify lines of the
  session body's card, with no author; docs 03 say where they come from.
- **Test:** session_start `the_session_lands_through_the_end_gate`. Through `open_with`, the gate
  sends a failing answer back, and the fixed answer's `@U law` claim is grounded. Checked to fail
  with the gate detached.
- **Bench consequence:** this campaign's Mnemorph arms ran without a session gate. A
  `solo-carry` measured on a build with this fix also needs a `Verify:` line on the bench
  world's Rimuru card.

---

### [2026-10-26T16:12:41+01:00] rimuru — the session gate, Fable's review
- **Fable:** mergeable. Lanes are safe: `before` is refreshed with landed and put-back work. All
  entry points (server, CLI) go through `open_with`. One behaviour change wanted a deliberate yes:
  tests-intact now fails a solo session's legitimate test deletion or rename. The one retry can
  only undo it, and a second FAIL reconsolidates the memory the ask cited.
- **Decided:** in solo the human is at hand. A verdict failing on tests-intact alone is put to
  them on the approval gate as a destructive act (`Run::human_allows`): allowed, it lands; denied,
  or no human (`NoHuman`, the bench, headless), it fails as before. A tests-intact-only failure
  never reconsolidates. Committees keep the strict rule (an author is set).
- **Also:** `Context::gate()` is public for the loop; docs 03 state the solo exception and that
  the session's verify lines are read at open (Fable's point 3).
- **Test:** solo `a_deleted_test_is_the_humans_to_allow_in_solo` (approve lands, `NoHuman`
  fails). Checked to fail with the approval disabled.
- **Not done:** `Run::resume` has no gate, and no callers yet; it should take one when it gets
  a caller (#204).

---

### [2026-10-26T16:16:04+01:00] rimuru — the gate waiver, Opus's security review
- **Opus found:**
  1. (medium) the approval card showed model-chosen paths raw, so a test file named with a newline
     or an escape code could forge or hide card lines;
  2. (low–med) skipping reconsolidation on tests-intact-only applied outside solo too, sparing the
     classic reward hack (delete or skip the failing test) its memory cost.
  - Nothing auto-approves (`NoHuman`, Terminal and the server's gate deny on silence), and
    `Context::gate()` being public reaches only compiled-in tools.
- **Fixed:**
  1. run.rs `card_list` escapes control characters and caps the list at 5 plus a count;
  2. reconsolidation is skipped only for `solo && tests_only`.
- **Test:** solo `a_deleted_tests_name_cannot_forge_the_card` deletes through the shell (the
  delete tool already refuses such a name). Checked to fail unescaped.
- **Left, pre-existing:** the `Ending::Failed` text and the log carry the same raw path; hollowed
  assertions with an unchanged test count pass tests-intact.

---

### [2026-10-26T16:20:04+01:00] rimuru — the gate waiver, Fable reviews Opus's review
- **Fable found what Opus missed:**
  1. The `Gate` beat is recorded before the ask, and the CLI renders it raw
     (`transcript.ts`, `app.tsx`) one line above the escaped card. The same raw text reached
     `Ending::Failed` and the note handed back. Escaping only the card was bypassed.
  2. `is_control` misses the bidi overrides, zero-width characters and line separators.
  3. A denied waiver was asked again on the retry.
- **Fixed:**
  1. `judge` escapes every failure and unverified line once, at the source, with `shown`, before
     any beat, log, note, ending or card;
  2. `shown` covers C0/C1, U+200B–200F, U+2028–202E, U+2060–2069 and U+FEFF, and doubles a
     backslash (`\u{…}` throughout, unambiguous);
  3. the waiver is asked once an ask (`!*retried`); `card_list` keeps the cap.
- **Test:** the card test adds U+202E, asserts one ask, and checks the ending carries no raw
  newline. Checked to fail with the source escape off.

---

### [2026-10-26T16:30:31+01:00] rimuru — suggest the session's verify line
- **Why:** with #219 the session lands through the end gate, but a card with no `Verify:` line
  runs no check, so solo claims still never ground. That's every project by default, the bench's
  worlds included (a side-agent note).
- **Built:**
  - world verify.rs `suggest`: a settled or agreed `test=` fact about a top-level file (the root
    itself is no fact path), else the manifest's command: Cargo, Python, Node (pnpm/yarn/npm by
    lockfile), Go, or a Makefile `test:`;
  - `missing_session_verify` says it;
  - mnemorphd `onto check` raises `no-session-verify` (low); `memory status` prints a note, also
    when no memory exists yet (`session_verify_note`);
  - docs 03 and 09.
- **The card stays the human's:** only ever suggested.
- **Tests:**
  - world: detection order, the ontology's word first, and silence once the card has its line;
  - mnemorphd: the note with `go test ./...`.

---

### [2026-10-26T16:34:07+01:00] rimuru — the verify suggestion, Fable's review
- **Fable found:**
  - the `test=` fact source is model-written (two creatures' `@U`, or an Orc's settle), so
    `test=rm -rf ~` could become the line the human pastes, and its value was printed raw;
  - npm's placeholder test script suggested an `npm test` that always fails;
  - the `--json` output carries no note.
- **Decided:** suggestions come only from the manifest, as constant commands the harness picks.
  The ontology source is dropped: no fact a model wrote is ever offered for pasting. This also
  removes the raw-printing finding.
- **Fixed:** npm's "no test specified" suggests nothing.
- **Left:** the note is text-only; JSON stays rows.
- **Test:** two creatures asserting `test=rm -rf ~` leave the Cargo suggestion unchanged; the npm
  placeholder gives none.

---

### [2026-10-26T19:03:45+01:00] rimuru — the end gate's verify limit, `gate.verify_secs`
- **Why:** each verify command had a fixed 120 s, and `suggest` offered `cargo test --workspace`
  for any Cargo.toml: a cold build of a large compiled project (Mnemorph's own included) outruns
  it, so the gate said NOT_VERIFIED and the suggested line never grounded a claim.
- **Built:**
  - config `gate.verify_secs`: an Int, default 120, 10 to 600 (the bash tool's own ceiling, so
    no value is clamped unseen), an error naming the file outside it;
  - `EndGate::verify_secs`, set by the session's gate and carried to children by the
    Commissioner, as `stress_tokens` is;
  - `suggest`: a workspace whose root is a package gives `cargo test -p <name>`, the name taken
    only when Cargo would accept it; `missing_session_verify` adds, for Cargo, Go and TypeScript,
    that a cold build may need the key raised;
  - docs 03 and 05.
- **Decided:** the tracked file may set it. A longer limit runs the same classified, sandboxed
  command, only for longer, within the ask's clock; a killed command grounds nothing either way.
  The project knows its own build; the human's local file still wins.
- **Tests:** the key's default, bounds and layers; the gate kills `sleep 3` at 1 s (NOT_VERIFIED)
  and passes it at 5 s; the root-package narrowing, and names Cargo would refuse kept out.
### [2026-10-26T19:17:57+01:00] rimuru — a call with broken arguments is answered, not fatal
- **Found by the bench:** the checked arm's task 09 ended after 51 calls with "the stream could not
  be read: tool read's arguments: EOF while parsing". The model streamed a finished `read` call
  whose JSON was cut short. Both providers made that a fatal `Error::Decode`, unless the stop was
  `MaxTokens`. docs/lessons.md (M4) promises such a call is answered with a named error and never
  run.
- **Fix:**
  - provider `MALFORMED`: openai.rs and anthropic.rs keep the call, with its raw text and the
    parse error under that key;
  - `finish()` can no longer fail and returns a `Response`;
  - loop `call_one` answers a `MALFORMED` input "this call's arguments were not valid JSON (…);
    nothing ran — send the call again".
- **Tests:**
  - provider: a stream ending `{"path": ` gives a kept call, not an error;
  - solo: the call is answered and the run goes on to its answer.

---

### [2026-10-26T19:48:32+01:00] rimuru — broken arguments, Fable's review
- **Fable:** safe.
  - The replay encodes the marker as a valid object.
  - Every reader before `call_one` treats it as a plain object.
  - The loop detector still counts identical repeats.
- **Follow-ups done:**
  - the error names the tool, as lessons.md promises;
  - the echoed error is capped at 200 characters, and a non-string error is "unreadable";
  - OpenAI warns when it drops a call cut by max_tokens, as Anthropic does;
  - an Anthropic test;
  - the solo test writes a README and asserts `read` never ran.

---

### [2026-10-26T19:07:12+01:00] rimuru — the terminal UI in Rust, part one
- **Asked:** a Ratatui terminal UI with Crush-grade polish and the lowest lag, the session hosted
  in-process; built on a Rust kit of Charm's good parts (Bubble Tea, Lip Gloss, Bubbles,
  Harmonica, Glamour).
- **Built:** crate `mnemorph-tui` (binary `mnemorph-tui`):
  - `kit`: box styles and cell-measured text, textarea with history, viewport on a critically
    damped spring, spinner, key help, Markdown from a theme-derived stylesheet, syntect
    highlighting in palette colours;
  - `app`: one model, `Msg`, a pure `update` returning `Cmd`s, `view`;
  - `transcript`/`blocks`: entries folded from `mnemorph_server::events`, tool cards with
    folded output and diffs (`similar`), per-entry layout cached per width, only the visible
    window laid out;
  - `gate`: the approval card over `Pending`; `run`: the session on its own runtime,
    frames coalesced to 30 a second, the terminal restored on exit and on panic.
- **Core:** `mnemorph_loop::shown` is public, beside `hidden`, the predicate it escapes by; the
  transcript escapes with `hidden`, the card with `shown`.
- **Kept out:** bincode (syntect's dump reader) is unmaintained, not vulnerable: ignored in
  deny.toml with its reason.
- **Tests:** the test backend's buffer for Markdown, highlighting, edit and patch diffs, a write's
  head, folding, spinner to ✓, header and status, narrow widths, the light page, a hostile
  approval, queueing and Esc; the palette against the TypeScript one; a scripted session hosted
  end to end; 2,000 messages under 33 ms a frame in a debug build.
- **Left for part two:** the sidebar (its column is reserved in the layout), mouse scrolling,
  expanding a folded card.

---

### [2026-10-26T19:25:11+01:00] rimuru — the terminal UI, the security review's round
- **Opus found:** an act cut at six rows could hide its tail from the approval card; `y`…Enter
  typed ahead approved; card and input could overflow a short terminal; SIGTERM/SIGHUP left the
  terminal raw; an expired card stayed; entities (`&#x202E;`) decoded after escaping; a panicked
  session kept the UI busy; a four-backtick fence holding three read as closed.
- **Fixed:**
  - `approval`: the act scrolls in the card, head, cut and last line shown; `y` refused until
    read to its end; keys ignored for 300 ms after the first draw and keys stamped before it; only
    a bare `y`/`n`, then Enter; a countdown, and the card leaves at its deadline;
  - the card takes the input's place, lines drop by priority, the stack is clipped to the area;
  - signal-hook (already under crossterm) turns SIGTERM/SIGHUP into a quit; a panic off the UI
    thread no longer restores the terminal under a live UI; `supervise` reports the session
    task's panic;
  - the Markdown renderer escapes each parsed text; fences track their character and length;
  - the palette's names are destructured exhaustively.
- **Tests:** each finding has one (`tests/ui.rs`, `run.rs`, `transcript.rs`, `markdown.rs`);
  a pty run checked SIGTERM exits cleanly.

---

### [2026-10-26T19:55:30+01:00] rimuru — the terminal UI, Fable's review of the review
- **Fable found:** `viewed` came from geometry, so End jumped past an act's middle and `y` still
  passed; after the arming delay a word ending in `y` (`okay⏎`) or `y\r` in one read approved; a
  second card overwrote the first, whose wait hung to its deadline; SIGINT and SIGQUIT were not
  handled.
- **Fixed:** rows count as seen only when drawn and contiguous with those seen before (`seen_to`,
  reset on a new width); a key that is not `y`/`n`/Enter/scroll, or a paste, re-arms the card and
  clears the choice; Enter decides only 150 ms after its letter; cards queue first come first
  shown, the count on the card, and a turn's end, Ctrl+C or a signal denies them all; the four
  quit signals share one list.
- **Left:** the commissioning chain on the card — the gate's ask names the body, not its line.
- **Tests:** End then `y` refused, steps then `y` accepted; `okay⏎`, `deploy⏎`, `y\r` in one batch,
  a paste; two cards answered in order; the signal list; a pty run of SIGINT and SIGQUIT.

---

### [2026-10-26T21:14:06+01:00] rimuru — the terminal UI, part two: the sidebar and the mouse
- **Asked:** the sidebar in the column part one reserved (session, memory, files, bodies),
  Ctrl+O and a fold under ~100 columns; the wheel on the kit's spring, a click or a key to expand a
  folded card; Fable's three minor items from part one's review; a small mark in the header.
- **Built:**
  - `sidebar`: four sections from what the observer and the world give, nothing guessed; files
    marked A/M/D by the act (a `write` by whether its path existed, which the observer looks at on
    the run's task before the act runs); bodies nested by the `Seen::From` chain, each in its
    roster race's colour; every claim, path and name through `safe::one_line`;
  - `mnemorph_memory::standing`: the reading `memory status` prints, now shared with the UI and
    with the claim grounded last; the UI reads it and the roster through `Cmd::Survey` on a
    runtime thread, at start and after each line;
  - `kit`: a gauge (Bubbles' progress bar), `ViewportMsg::To`, `clip_left`;
  - mouse capture; the transcript's window records each row's entry for a click; Alt+↑↓ moves the
    focus between cards and scrolls it into view; Ctrl+T expands the focused card;
  - the header's mark: MNEMORPH drawn across the water races' blues.
- **Fixed:** `rearm` on a card not yet drawn no longer arms it from the key; every approval open
  or waiting is denied however the loop ends; a terminal mode that fails after the terminal is
  taken gives it back (`enter_with`).
- **Tests:** each section with data and hostile text, the race colours and nesting, the toggle,
  the fold at 99/100 columns, the wheel's spring, click and key expand/collapse, focus scrolling,
  the three fixes, `standing`'s counts and latest claim; 2,000 messages with the sidebar open,
  667 files and forty bodies, still ~3 ms a warm frame in a debug build.
- **Learned:** a `write` result does not say whether it created the file; only a look before the
  act tells A from M, and the observer runs at the right moment for it.

---

### [2026-10-26T22:05:00+01:00] rimuru — the terminal UI, part two: Fable's review
- **Fable found:** the panic hook left mouse capture on; unordered surveys could overwrite a newer
  reading with an older one; the bodies' depth-first order searched the whole list per body,
  per frame; the observer stat a model-chosen path on the session's task, before the gate.
- **Fixed:** `run::release` turns the mouse and bracketed paste off and pops the keyboard flags
  once, from the hook or the drop; `Cmd::Survey(seq)` and `Msg::Surveyed { seq, .. }` drop stale
  readings; `Bodies` keeps a name and parent→children index as events fold in, and `walk` is
  lazy; the observer posts a `write`'s path (`Msg::Writes`) and `Cmd::Look` checks it on a runtime
  thread.
- **Learned:** a look off the session's task races the write it looks for. Only "not there" is
  proof (the write created it), so the A mark may arrive late and upgrade an M; "there" proves
  nothing.
- **Tests:** release pops once and turns both modes off twice; a late survey is ignored; 3,030
  bodies walk in tree order and draw under the frame budget; a look answered before or after the
  result, a raced "there", a delete-then-write, a failed write.

---

### [2026-10-26T23:40:00+01:00] rimuru — the terminal UI, part three: alive and progressive
- **Asked:** the human tried PR 2 — "no loading; thinking and tool calls load all at once".
  Match Isekai's TUI (read only: `tui/` and its screenshots), progressive from the first frame.
- **Built:**
  - the activity strip, from the frame after Enter until the turn ends: a spinner —
    Isekai's braille dot at 10 fps, the phase as a verb in a shimmer, elapsed and tokens, then
    the body and the phase in words (thinking, writing, calling, running `bash: …`, waiting for
    approval, compacting) and the context near its line; the header's right says the same;
  - the reasoning: providers stream it (`StreamEvent::Thinking`) and a call's arguments
    (`StreamEvent::ToolInput`); one live line, folded to `✓ thought for 12s`; opaque reasoning
    from the journaled blocks placed where its step began;
  - cards from the first byte (summary from the partial JSON), `⎿ ok · 1.2s`, output folded to
    three lines, reads/greps/globs grouped, the class read off the UI thread;
  - Isekai's depth as kit styles (`kit::slab`): spined panels, lit edge, cast shadow; small-caps
    pane labels; the welcome card; the toast; the sidebar's vitals, folding sections, file tree;
  - the approval as a modal over the dimmed view, numbered answers (the third denies with a
    reason), every pick through the same arming, reading and confirmation rules;
  - `kit::mnanim`: the `.mnanim.json` decoder; the sidebar's head plays the working body's 24 px
    idle loop (only while it works and shows; idle, frame 0 and no ticks), the welcome shows
    Rimuru's first frame — on a hand-made placeholder until mnemorph-animations ships 24 px.
- **Tests:** the status on send before any token, the phases, the thought's fold and duration,
  the card before its result, grouping, a 500-line fold, the portrait swap as a child starts and
  ends, no ticks while idle or hidden, the gate's line and toast, a 2×2 file; 2,000 messages still ~3–8 ms a debug frame.
- **Learned:** the stream never carried reasoning to a surface: both providers kept it to replay
  only. A surface that wants it needs its own event, never part of `started` (a retry must stay
  possible while only reasoning has been shown).
- **Open (@?):** the server's protocol does not yet carry `Thinking`/`ToolInput`; the Ctrl+K
  palette and the per-body view (Ctrl+T in Isekai) are left for a later part.

---

### [2026-10-27T00:30:00+01:00] rimuru — the terminal UI, part three: Opus's review and the real portraits
- **Art:** the eleven races' idle loops at 32 px (mnemorph-animations `df20cfa`,
  `gen/mnanim.py --size 32`, 1.1 MB) replace the placeholder and its tint; the sidebar widens to
  36 columns for the 32-column portrait, still folding below 100; the head keeps only the state
  under it. 2,000 messages with the portrait playing: ~5.5 ms a streaming frame (debug).
- **Opus found, fixed, tested:** ↑↓ from no pick landed on "Yes" and ↓ wrapped through it — now
  from no pick to "No", clamped at the ends, and ↓ then Enter denies; a group with a stopped call
  read "ok" — it counts "N stopped"; a retry that streamed reasoning twice left it doubled — the
  journaled text replaces whatever streamed; a non-ASCII or overflowing frame could panic the
  decoder — an error now; the toast drew over the approval — hidden while a card is open.
- **Learned:** an arrow on a gate card is a pick like any other: its landing spot is a safety
  default, and the safe default is the denial.

---

### [2026-10-27T01:15:00+01:00] rimuru — the terminal UI, part three: Fable's follow-ups
- **Fable found, fixed, tested:** the portrait followed the last event's body, so two working
  children thrashed it and an ending fell back to Rimuru mid-run — now it is the newest body still
  working (`sidebar.bodies`), else the session's, and the loop restarts only on a real change; each
  race decoded lazily on the UI thread (~24 ms) — now `mnanim::warm` decodes all eleven on a
  blocking thread at start and frames only read the cache (`ready`), drawing nothing until then;
  the streaming refactor is pinned by the whole assembled content of a stream with reasoning and a
  call, for both providers.
- **Learned:** "who acts" is a state, not an event: read it from the bodies' standing, never from
  whichever one spoke last.

---

### [2026-10-26T23:58:00+01:00] rimuru — the terminal UI, part four: the portraits in real pixels
- **Task:** draw the races' portraits through terminal image protocols, as sharp as their 256 px
  source; the half-block portrait becomes the fallback.
- **Files:** `probe.rs` (the start-up probe: `TIOCGWINSZ`, `CSI 16 t`, XTVERSION, Kitty's query,
  DA1 last; `poll` on `/dev/tty`, 200 ms, no reader thread left behind), `kit/pixels.rs` (replies,
  the choice Kitty → iTerm2 → Sixel → half blocks, the box math, Lanczos pictures, the three
  encoders, a median-cut quantizer for Sixel), `pictures.rs` (what a frame wants, the placer that
  sends, moves and erases), `assets/portraits/` (mnemorph-animations `df20cfa`), `--probe`,
  `MNEMORPH_TUI_IMAGES`.
- **Chose:** our own layer over `ratatui-image` 11: its probe reads stdin on a thread that
  outlives its timeout (it would eat the UI's first keys), it ranks Sixel over iTerm2 for xterm.js,
  and its pictures live in a cell's symbol, out of reach of the approval and of exact erasing.
- **Tests:** the choice from xterm.js's replies with and without `;4`, a Kitty OK, none; the env
  override; 32 columns of 8 × 17 → 256 × 255; Kitty chunks and terminators, iTerm2 and Sixel
  payloads; nothing wanted or sent under an open approval, back after it; no re-send on an
  unchanged frame; the fold, the short column, half blocks; the probe's timeout.
- **Learned:** a terminal image is not a cell: the view must say where pictures go, and the
  runtime must erase what text cannot cover — by id under Kitty, by writing the cells again
  under Sixel and iTerm2.
- **Open (@?):** not yet seen in a real terminal by the human; under Kitty a move re-sends the
  PNG (upload once and place by id is the animation's seam).

---

### [2026-10-27T00:40:00+01:00] rimuru — the terminal UI, part four: Opus's four lows
- **Fixed, tested:** Kitty pictures are placed below the text (`z=-1`), so the approval card
  shows over one even where a delete is ignored; replies of a timed-out probe arriving late as
  keys (Alt+P / Alt+_ … Alt+\, `?…c`, `…t`) are dropped for two seconds, and no key reaches an
  open card then; iTerm2 images are sized in cells, and the cell is read again from
  `TIOCGWINSZ` on a resize (a font zoom rebuilds the pictures); a panic deletes Kitty's
  pictures before the terminal is given back, once.
- **Learned:** a probe's reply has no owner after its timeout: whatever reads the input next
  takes it as typing, so the window after a timeout is the probe's to guard.

---

### [2026-10-27T01:30:00+01:00] rimuru — the terminal UI, part four: Fable's three on the pictures
- **Fixed, tested:** the late-reply filter ate Ctrl+C (a bare `c` matched, modifiers unread) and
  every key on a card — now Control and Esc always pass, reply shapes are plain or Shift, and
  only Alt+P/_/[/\ carry Alt; Sixel with no window pixels falls back to half blocks after a
  resize (the old box repainted) until a size returns; a moving box is erased and its picture
  held back until it has stayed put 150 ms — a scroll sends none per frame.
- **Learned:** an input filter that names a key shape must also name its modifiers.

---

### [2026-10-27T09:09:21+01:00] rimuru — VS Code images detected by the graphics query
- **Found in the human's terminal:**
  - VS Code's xterm.js with `terminal.integrated.enableImages` on still answers DA1 `?1;2c`,
    with no `4`;
  - the probe read that as "images off" and drew half-blocks;
  - `--probe` showed iterm2 yes and drawing blocks.
- **Fix:**
  - probe.rs QUERIES sends XTSMGRAPHICS (`CSI ? 1 ; 1 ; 0 S`) before DA1;
  - kit/pixels.rs `graphics()` reads a success answer (`CSI ? 1 ; 0 ; n S`);
  - `Replies.graphics` counts as images on (iTerm2 in its family, Sixel outside it).
- **Test:** `a_graphics_answer_turns_images_on_where_da1_is_silent`.
- **Unconfirmed:** that VS Code answers the query (its image add-on documents XTSMGRAPHICS); the
  human's `--probe` will tell.
### [2026-10-27T09:11:37+01:00] rimuru — solo: find and read with the tools, the root named
- **Why:** on the bench, solo bodies spent calls finding where they stood (`pwd`,
  `find / -maxdepth 3 -name sqlglot`, `cd /tmp`) and sometimes searched or read through bash
  (`grep -rn … | grep …`, `cat … | head`) instead of the grep and read tools.
- **Fix:** solo.rs SOLO_PROMPT says to find with glob and grep and read with read, and that bash is
  for running things (tests, builds, scripts); the body's system prompt ends with the repository
  root's absolute path.
- **Check:** one regression task on the bench (below), comparing tool use and calls with the
  campaign's run of the same task.

---

### [2026-10-27T11:40:43+01:00] rimuru — pictures in the sidebar alone; Veldora at rest
- **The human, testing in VS Code:**
  - the TUI showed nothing where real pixels should be, while blocks worked;
  - "keep image only on side bar", and on the start screen put Veldora, then Rimuru when it works,
    and so on.
- **Done:**
  - view.rs `welcome_art` always draws half blocks, so the welcome scrolls with nothing to
    withdraw;
  - pictures `Art::Veldora` and kit/pixels `VELDORA` (Mnemorph's own `assets/portraits/veldora.png`,
    256 px) for the sidebar's picture while no body works;
  - sidebar `veldora_blocks`, Veldora in half blocks when the terminal draws no images;
  - a working body's race and loop during a turn, as before;
  - docs 05.
- **Tests:** the pictures and progress tests now expect Veldora at rest, the sidebar alone wanted,
  and one Veldora build and one working-body build.
- **Open:** whether the sidebar's picture shows in the human's VS Code (the empty box) is
  unconfirmed; their check on this build tells.

---

### [2026-10-27T12:00:59+01:00] rimuru — VTE: the cell from its text area, Sixel by its version
- **The human's Ubuntu terminal** (GNOME Terminal, VTE 0.60.3):
  - no XTVERSION, no `CSI 16 t`, TIOCGWINSZ pixels 0×0, and DA1 `?65;1;9c`;
  - it answers `CSI 14 t` as `4;1575;882`, width first, for 175×49 cells: a 9×18 cell;
  - forcing Sixel showed only the half-block fallback, because the cell was unknown;
  - VTE 0.60 draws no Sixel anyway (VTE enables it from 0.76).
- **Fix:**
  - probe.rs QUERIES adds `CSI 14 t`;
  - kit/pixels.rs `Replies.area` and `cell_from_area`, which take the order that gives a plausible
    cell;
  - `VTE_VERSION ≥ 7600` means Sixel;
  - docs 05.
- **Test:** `vte_gives_its_cell_by_the_text_area_and_sixel_by_its_version` uses the human's exact
  replies.

---

### [2026-10-27T12:04:54+01:00] rimuru — #229, Fable's review
- **Fable found:**
  1. `cell_from_area` took the spec's order when both are plausible. VTE 120×50 with a 9×18
     cell answers `4;1080;900`, and the spec order gives 7×21, so the Sixel box spills a row.
  2. `h * 10` on u16 overflows.
  3. `VTE_VERSION ≥ 7600` as a decision fires only when VTE's own DA1 omits Sixel, likely when
     its Sixel is off, which gives a blank box.
  4. Small items: the late `CSI 4;…t` shape undocumented, a dead branch and a stale comment in
     the sidebar head, and an `assert_ne` that proved only "not Rimuru".
- **Fixed:**
  1. u32 maths; the order the window divides exactly wins, then the one nearest 2:1, then the
     spec's;
  2. `VTE_VERSION` is a note in `--probe` (a VTE that lists no Sixel draws half blocks), and VTE
     with Sixel on is detected through DA1;
  3. the filter shape documented and tested; the head cleaned;
  4. the idle head asserted equal to `veldora_blocks`.

---

### [2026-10-27T13:48:18+01:00] rimuru — #38, append-only requests
- **Why:** the bench read a 94% cache hit rate against OpenCode's 97%. Every move of the cached
  prefix resends the whole history uncached.
- **Audit** of the loop and the providers' request building, for what rewrites an earlier part
  of the request mid-session:
  - append-only already: the manifest and the opening (taken once, in the ask's message), recall
    and `@ONTO` (the ask's message), a Mind worn (a tool result), steering, gate failures, stress
    and top-up notes (appended), `forget_reads` (the read tracker, not the conversation), the
    wake's lines (the next ask's message). The model, the date and the configuration never
    changed mid-session: there was no lane for them;
  - documented rewrites: the fold, the structured compaction;
  - accidental: the drain and salvage turns cleared the tool list, so the turn with the fullest
    context of the run was resent uncached.
- **Fix:**
  - the frame (system prompt and tools) is built once in `Run::new`;
  - drain and salvage send the tools and keep only the words of a reply;
  - `Run::update` and `Run::switch_model` append a `[context update]` note;
  - `Beat::Prefix` records each move: fold, compaction or unexplained;
  - an unexplained move warns, panics in a debug build, and strips the earlier asks' thinking
    once.
- **Tests:**
  - an eight-turn session with steering, a Mind, a fallback, a config reload and a fold;
  - the recovery's unit test.
- **Proof the session test is not vacuous:**
  - updates edited into the system prompt fail it ("request 5 rewrote the system prompt", or the
    debug guard's panic);
  - steering edited into the ask's message fails it ("one move, the fold: [(2, 2), (6, 6)]").

---

### [2026-10-27T14:03:35+01:00] rimuru — #231, Fable's review
- **Fable found:**
  1. The debug panic came after the recovery, so the recovery could never run in dev, and a
     debug TUI died mid-session.
  2. Under preserved thinking, an unexplained edit invalidates every later thinking block. Keeping
     the in-flight turn's thinking guarantees a 400 that the ladder does not retry. A turn that
     held only thinking was left whole.
- **Fixed:**
  1. An unexplained move is a warning and a `prefix` beat in every build.
  2. The strip, still once a session, takes every thinking block, the turn in flight's included,
     and keeps tool calls and words. A thinking-only turn becomes "(reasoning omitted)".
  3. Docs 06 states the rule.
- **Test:** `the_strip_takes_all_thinking_and_leaves_no_gap`.

---

### [2026-10-27T13:30:18+01:00] rimuru — #184, toolchain credentials closed on Linux
- **Task:** Landlock granted `.cargo` and `.cache` whole, so `.cargo/credentials*`,
  `.cache/huggingface/token` and `.cache/git` stayed readable to a sandboxed command on Linux.
- **Fix:** `HOME_TOOLCHAINS` (one list, both platforms) names `.cargo/bin`, `registry`, `git`,
  `config(.toml)`, `env`, and the named caches under `.cache` (uv, pip, pypoetry, pre-commit,
  go-build, golangci-lint, node, pnpm, node-gyp, typescript, yarn, ms-playwright, deno, bazel,
  sccache, huggingface/hub, kaginari-bench). Docs 05 and a lessons row.
- **Measured** (landlock ABI 1, before and after, identical but for the credentials):
  - `cargo tree` / `cargo fetch --offline` read `.cargo/bin`, `config*` and `registry`; cargo opens
    `.package-cache` and `.global-cache` for writing and goes on without them.
  - pnpm reads `.cache/node` (corepack); the bench's Python reads `.cache/kaginari-bench` and uv's
    interpreters.
  - `uv run` and `go build` need a writable cache (`UV_CACHE_DIR`, `GOCACHE` in `$TMPDIR` works),
    and `cargo build` hits EXDEV on ABI 1: both before the change as well.
- **Test:** `toolchain_credentials_cannot_be_read_but_their_caches_can`, and the policy tests
  hold no credential under any rule. With `.cargo` and `.cache` granted again, all three fail.
- **Learned:**
  - A read grant on a toolchain tree is a read grant on every secret a tool drops in it; name
    what builds use instead.

---

### [2026-10-27T14:10:00+01:00] rimuru — #184, Opus's review: the rest of the class
- **Opus found:** `.config/git` (git's credential store), `.m2` and `.gradle` (`settings.xml`,
  `settings-security.xml`, `gradle.properties`) and `.nvm` (`versions/node/*/etc/npmrc`) were
  granted whole; `.cargo/config.toml` and `.gitconfig` can hold tokens; `Policy::existing`
  granted wherever a toolchain link led.
- **Fixed:**
  1. `.config/git/{config,ignore,attributes}`, and `.config/git/credentials` in `HOME_CREDENTIALS`;
  2. `.m2/repository`, `.gradle/caches`, `.gradle/wrapper`;
  3. each nvm Node's `bin`, `lib`, `include`, listed when the policy is made;
  4. `.cargo/config.toml` and `.gitconfig` stay readable, documented as an accepted risk;
  5. a home toolchain rule resolving onto, inside or above a credential is dropped and kept in
     `Policy::refused`.
- **Test:** the escape test reads none of the new credentials and reads the caches beside them;
  `each_nvm_node_is_read_without_its_npmrc`; `a_toolchain_linked_to_a_credential_is_refused`.
  Disable-and-run: each whole tree back fails 2–3 tests; the link check off fails its test;
  `.config/git` whole is caught by the link check alone, and with it off fails 4.
- **Measured:** node, npm (`npm ls`, `config get`), pnpm and git run with the narrowed grants;
  npm goes on when its global npmrc is unreadable.
- **Learned:**
  - The "above" half of the link check also guards the list itself: a tree named whole that
    holds a credential is refused even when no test names it.

---

### [2026-10-27T14:40:00+01:00] rimuru — #184, Fable's review of the review
- **Fable found:** `~/go` (GOPATH) was granted whole, so `go/src` (the human's projects, their
  `.env`) was readable; `.cargo/git/db/*/config` holds remote URLs that may embed a token; no test
  proved a link inside a granted cache leads nowhere.
- **Fixed:** `go/bin` and `go/pkg` only; the cargo git-db configs documented as an accepted risk
  beside `config.toml`; the escape test plants `.cache/uv/x → .ssh` and reads the key through it.
- **Test:** the policy test names `go` and `go/src` as never granted whole; the escape test's
  read through the link is refused (Landlock resolves at open time).

---

### [2026-10-27T16:00:00+01:00] rimuru — the welcome opens on a MNEMORPH wordmark and a card that says what Mnemorph does
- **The human, launching the TUI:** Rimuru's half-block portrait on the start screen looked
  blurry (the sidebar's Veldora picture is good); "a fixed Mnemorph logo on the main load page"
  and "a better card describing Mnemorph".
- **Done:**
  - welcome.rs: MNEMORPH in five rows of block letters from a small static font (`█ ▀ ▄` and
    spaces, one foreground a cell), coloured across its columns Rimuru → elf → orc → slime;
    `◈ MNEMORPH` on one line where the card is narrower than the wordmark; the tagline, four lines
    on what Mnemorph does, the chips, and MEMORY and VERIFY under WORLD / MODEL / SESSION;
  - sidebar `Survey` carries the session card's verify line, or the manifest's suggested test
    command when it has none (`world::verify::suggest`), read off the UI thread with the memory;
  - the welcome's portrait gone: view `welcome_art`, `welcome::still`, `pictures::Slot::Welcome`
    and the transcript window's `head` offset;
  - perf: the cold first frame keeps a cold-start margin (250 ms, after a 104 ms flake on CI
    macOS); every warm frame still holds 33 ms;
  - docs 05.
- **Tests:** welcome unit tests (crisp glyphs only, the gradient across the width, the narrow
  fallback, the four lines at 80 / 120 / 200, VERIFY's suggestion, no picture wanted, nothing
  overflows); sidebar `the_survey_reads_the_verify_line_or_a_suggestion`.
- **Learned:** a portrait in half blocks cannot be made sharp; text glyphs in one colour a cell
  can. The header's mark already reads `◈ MNEMORPH`: a test of the card's fallback reads the card,
  not the screen.

---

### [2026-10-27T17:30:00+01:00] rimuru — #232, reviewed: the wordmark's real threshold; the resize frame's cold-start margin
- **Done:** the docs and welcome.rs state the rule — the 62-cell wordmark shows on a card 68 wide
  or more: a terminal of 72 columns folded, 108 with the sidebar open; perf's first frame after a
  resize keeps the 250 ms cold-start margin, warm, streaming and paging frames still under 33 ms.
- **Tests:** `the_wordmark_shows_from_72_columns_folded_and_108_open` (71/72, 107/108); the
  fallback test pins 67/68 on the card.

---

### [2026-10-27T21:00:00+01:00] rimuru — #204: `mnemorph resume <run>` ships, through the session's one door and its end gate
- **Done:** mnemorph-loop `resume`/`resume_with` open a session on a stopped run through the same
  `build` as `open`/`open_with` — config, provider, sandbox, `session_gate`, observer,
  commissioner — only the run differs (`Run::resume`, the journal's opening not said again);
  module `resume`: `stopped` (newest first), `take` (an id or `last`; refuses an answered, failed
  or escalated run, a missing or corrupt journal, a commissioned body's run, a held run — before
  any key is asked for), `hold` (an OS file lock beside the journal, kept by `Session` while it
  lives); `Beat::Rides` journals the model a run rides, a resume keeps it and a configuration
  change becomes `switch_model`'s context update; a session's checkpoint warns "ask again to go on,
  or later `mnemorph resume <run>`". Commands: `mnemorphd --resume <run|last>`, `mnemorphd runs
  [--json]`, `mnemorph resume <run|last>`, `mnemorph runs`, `mnemorph-tui --resume <run|last>`;
  the SDK's `stdio({ resume })`. Docs 03 (Resume), 05 (the TUI), both helps.
- **Tests:** loop `resume.rs` — a resumed run's answer is sent back by a failing verify line (with
  `.with_gate` removed from `build` the test fails: answered "Done." after two requests), the
  refusals, `last`, the model switch (no unexplained prefix move, the sent prefix appended to);
  mnemorphd `runs.rs`; tui `resume.rs`; CLI args; the commands-exist test with the new hints.
- **Learned:** a resume must refuse before the key check, or a missing run reads as a missing key;
  a code span wrapped across lines breaks the book's build (`<run>` read as an HTML tag).

---

### [2026-10-27T22:30:00+01:00] rimuru — #233, reviewed: escaped listings, no unheld session, the lock probe waited out
- **Done:** `runs` escapes the journal's ask and stop reason with `shown`; a fresh run whose lock
  cannot be made opens no session; a resume retries its lock for 250 ms past a listing's probe; a
  session ending with nothing to resume unlinks its lock file while holding it. Docs 03.
- **Tests:** loop `a_hostile_journal_is_shown_escaped`, `a_run_that_cannot_be_held_opens_no_session`,
  `a_probe_never_refuses_and_finished_locks_go`.

---

### [2026-10-27T16:51:34+01:00] rimuru — #92: a gate approval binds to a plan fingerprint, rechecked at the spawn
- **Done:** `mnemorph_tools::Fingerprint` (module `fingerprint`): the act, HEAD, the diff (index as
  listed, tracked bytes and mode bit, untracked-not-ignored; a world with no repository: every
  file its ignore files leave in) and the target (branch, every ref, `remote.*`/`branch.*`/
  `push.*`/`url.*` config, each remote's resolved push URLs, `GH_REPO`/`GH_HOST`), read through a deaf git and hashed by the harness one component at a
  time, no symlink followed, no blocking open. `Ask.fingerprint`; the bash tool binds every
  approved command and recomputes right before `spawn`: a mismatch runs nothing and asks again
  ("the world changed since approval", what moved), three re-asks at most. The short form on the
  TUI card (escaped), the terminal card, the CLI card and `gate/ask`'s `plan`; the result keeps
  the plan the act ran against and every stale one. Docs 02, 04, 05 (Sandbox).
- **Tests:** tools `fingerprint.rs` — diff A approved, diff B at the spawn: re-asked, never pushed;
  HEAD moved, target changed and a push rerouted by `set-url`, `pushurl` or `pushInsteadOf`
  (each a Write in the world) re-asked; an unchanged world runs once; a churning world never
  runs; a second approval binds anew. With the recheck disabled 6 of the 9 fail (the other three
  check the fingerprint alone or the unchanged world). TUI: the plan on the hostile card, escaped.
- **Learned:** a world inside a larger repository sees the empty files the harness makes for an
  approved outside path; the recheck reads them as the absence the human was shown.
  - Measured: the recheck reads this repository (550 tracked files, 21 MB) in about 0.1 s.

---

### [2026-10-27T17:09:09+01:00] rimuru — #92, reviewed: the plan binds git's executables and the publish files, and fails closed
- **Done:** a fifth part, **git**: the whole `config --list` (includes expanded) and the bytes of
  the git dir's `config`, `config.worktree`, `hooks/`, `info/` (common dir too) and a `.git`
  file. Any git command failing is no plan: the gate is not asked, or the approved command does
  not run (a quiet "no commit"/"detached" excepted). The diff binds `.npmrc`, `.yarnrc(.yml)`,
  `.env`, `.env.*`, `.cargo/config(.toml)` by name, ignored or not, and follows submodules (their
  HEAD and tree, depth 8). A file is read no further than its size at the open. Doc 05 names
  what stays unbound (ignored code an act runs, other shipped ignored output, the tripwire's
  concurrency hole) and the follow-ups: bind the shipped list, stat-bind files over a size cap,
  a cross-court lock from recheck to spawn (world, then attic).
- **Tests:** tools `fingerprint.rs` — a `pre-push` hook, `core.sshCommand`, `alias.push` and
  `info/exclude` planted after approval re-ask; an ignored `.npmrc`/`.env.local` change re-asks; a
  corrupt index refuses before the gate; a dirty submodule re-asks. Recheck disabled: 9 of 13 fail
  (the corrupt index refuses before any recheck; the other three need no recheck).
- **Learned:** the tripwire's `Snapshot` reads with `fs::read` (following, blocking) and covers
  `.mnemorph/`, which churns with every court: the plan walks the same git pieces with its own non-blocking reader.

---

### [2026-10-27T17:19:57+01:00] rimuru — #92, reviewed again: repositories inside the world are bound whole
- **Done:** a submodule, an untracked repository (`ls-files -o` lists it as its folder) and the
  repository any folder the command names lies in (`git -C vendor`, `cd vendor`,
  `--git-dir=vendor/.git`, ignored or not) are bound as the world's own: HEAD and tree in the
  diff, `git_pieces` in git, `target` in target; eight deep. Doc 05.
- **Tests:** tools `a_repository_inside_the_world_is_bound_whole` — `remote set-url` and a planted
  `pre-push` in a submodule, an untracked and an ignored repository each re-ask, nothing pushed;
  `ignored_yarnrc_and_cargo_config_are_part_of_the_plan`. Recheck disabled: 10 of 15 fail.
- **Learned:** a word naming a path inside `.git` has no work tree: git refuses `--show-toplevel`
  there, so it is resolved to the repository it belongs to.
### [2026-10-27T16:43:12+01:00] rimuru — #90: a guarded attic restore — an undo only while the files are as the act left them
- **Done:** mnemorph-tools `Attic::seal(before, why) -> Act` snapshots what an act left;
  `Attic::restore(&Act, paths) -> Result<Restore, String>` diffs the act's two trees, compares each
  changed path now (blob, exec bit, a link's target, absence) to what the act left, and refuses
  the whole undo (`Restore::Conflict`, each path with left/now `State`) on any difference; else
  removes what the act created and checks out the rest with literal pathspecs. A path beyond a
  symlink or a file the act did not leave, and a directory holding more than the undo removes,
  are conflicts; paths that are not plain and inside the world are errors. Outcomes go to the
  attic's log. No override. Commission's stray put-back seals the child's act and uses it.
  `mnemorph_tools::rollback`: `Deployment`, `Deployed`, `roll_back` — a deploy rolls back only
  while the deployed version is ours. Docs 05 (the attic), 02 (Kijin rollback).
- **Tests:** tools `attic.rs` — concurrent edit refused and named; later deletion, mode change, a
  new symlink, a symlinked directory each a conflict; an unchanged state restores to the very
  tree before; a partial conflict restores nothing; bad paths refused. With the comparison
  disabled, the six guard tests fail. `rollback` unit tests.
- **Learned:** `git checkout <tree> -- p` never follows a symlinked directory — it unlinks it and
  makes a directory, and with a directory in the way it removes the subtree: safe for the world's
  border, destructive for a later writer, so both are checked before git runs.

---

### [2026-10-27T17:05:26+01:00] rimuru — #90, reviewed (Opus, then Fable): the attic's undo owns its writes
- **Done:** `restore` no longer runs `git checkout` (its cached leading-path check then
  unlink/create through the parent could be led out by a swap). New private `pinned`: parents by
  `openat2(RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)`, else an `O_NOFOLLOW|O_DIRECTORY` walk; content
  from one `cat-file --batch` into an exclusive temp, `fchmod`, `renameat` (`NOREPLACE` where the
  act left nothing); `symlinkat`; `unlinkat` never on a directory; only empty dirs removed. Files
  hashed only when a regular file of the left size, opened `O_NONBLOCK|O_NOFOLLOW`, hashed from the
  fd. `Attic::hold`/`restore_held`; `remove_created` for commission's new-file removal; the act
  sealed when `child.ask` returns; a part-way failure journaled. The rollback module dropped: the
  deploy rule stays in 02 as M9 work. Docs 05, 02.
- **Tests:** racer hook swaps `src` for a symlink between check and write: nothing outside written
  or removed; fifo and oversized file are conflicts, unread; pinned removal test. Guards disabled
  (comparison off, `NOFOLLOW`/resolve flags off): 11 tests fail.
- **Learned:** re-checking metadata before a path-based write is still check-then-act; only a
  pinned handle closes it. Open (@?): repair's n restores read n diffs — batch later.

---

### [2026-10-27T17:18:51+01:00] rimuru — #90, Fable's last three: a planted temp name survives, EPERM walks, seal before the seat goes
- **Done:** `pinned::put` names its temporary from 128 bits of `/dev/urandom` and unlinks it on a
  failed fill only when the creation itself did not find the name taken; `openat2` falling back to
  the walk on EPERM too (seccomp); commission seals the act before `drop(seat)`.
- **Tests:** pinned `a_planted_file_at_the_temporary_name_survives`. `the_clock_reaches_a_running_command`
  failed once locally under load and passed on rerun (timing).

---

### [2026-10-27T16:50:55+01:00] rimuru — #45: a forgiving edit — one candidate only, and it says what it forgave
- **Done:** mnemorph-tools `forgive`: with no exact match, `edit` (and each `multiedit` edit) climbs
  line endings → trailing whitespace → inner whitespace → indentation, taking a rung only when exactly
  one region matches; two or more stop the ladder, name each line, write nothing (an edit miss); none
  matching keeps the closest-line error. Indentation matches level by level and re-indents the
  replacement in the file's tabs or spaces; line endings stay the file's. The result reads e.g.
  "forgiven: indentation (+4 spaces) at lines 2–4". No anchor matching. A rung stops past 20M line
  comparisons (strict). Docs: lessons.md, the edit tool's description.
- **Tests:** tools `each_forgiving_rung_takes_one_region_and_says_so` (CRLF and tabs kept),
  `a_forgiving_rung_with_two_places_names_both_and_writes_nothing` (with the uniqueness check
  disabled — first candidate taken — it fails), `an_exact_match_forgives_nothing`.
- **Learned:** the edit tool lives in files.rs, not patch.rs (patch applies unified diffs).

---

### [2026-10-27T16:59:44+01:00] rimuru — #236, reviewed: meaningful whitespace stays strict
- **Done:** the inner-whitespace rung refuses a changed run inside a quote (odd `"`, `'` or backtick
  count before it); Markdown files keep trailing whitespace strict (a hard break); indentation levels
  are read from `old_string` alone, a half-step replacement line keeps its extra spaces, and in a
  tab-indented file stays strict. Docs: lessons.md, the edit description.
- **Tests:** tools `meaningful_whitespace_is_never_forgiven`.

---

### [2026-10-27T16:41:09+01:00] rimuru — #101: an outward plan with no declared undo is destructive
- **Done:** mnemorph-config module `outward`: the Kijin's plan step typed — `Act` (command,
  `Undo::{Rollback, Revert, None}`, `Effect::{Once, Standing { expires }}`), `plan` classes it
  into a `Planned` (only `plan` makes one): an outward act with no undo, or a standing effect with
  no expiry or one more than a year out, is `destructive`; the classifier's patterns stay the
  first reading. An undo is checked one level deep: not empty, not the act again, not refused,
  outward, not destructive. `Envelope::covers` refuses a destructive act before time, count or
  patterns, and matches patterns word by word, never a command that chains, expands or globs.
  `Planned::ask` puts `no undo — destructive` first on the card. Docs 02 (Kijin, envelopes,
  classes). gate.rs untouched; the gate calls `covers` when it honours envelopes (M11).
- **Tests:** config `outward.rs` — no undo is destructive and a matching, in-force envelope still
  refuses it; a standing effect with no or a far expiry is destructive; an undone act is covered;
  malformed undos refused; the pattern list a second line; the matcher. With the classification
  removed three fail; with the envelope's destructive guard removed the same three; with the undo
  check removed the malformed test fails.
- **Learned:** the classifier calls a branch delete destructive, so a new branch's only undo is
  refused: a `feat/*` push envelope covers a push only with an outward undo (a revert), else asks.

---

### [2026-10-27T17:02:07+01:00] rimuru — #101, reviewed (Opus, Fable on Opus): the undo is the harness's; pushes and `gh api` read closer
- **Done:** a model-written undo string could be outward and fake (`gh help`), and the real undos
  (deletes) are destructive, so `check_undo` and `Effect` are gone. `Act` is typed:
  `PushNewBranch` (create-only lease), `AddLabel`, `Comment` — the harness renders the command
  and, from `Done` (pushed sha, comment id read only from this act's own URL), the undo; the repo
  pinned in the act. `Command(String)` outward is destructive, no envelope covers it. Envelope
  patterns are read against the act's words (printable ASCII, no `:` `'` `"` or shell character).
  Classifier: `git push` outward only as one plain branch push (unknown options, every ref outside
  `refs/heads/`, renames, multiple refspecs, `--all`, `--receive-pack`, `-o` destructive; the
  create-only lease is not); `gh api` writes on any non-GET method, field, `--input` or graphql.
  vet fills an in-word `*` with a plain name. Docs 02. Standing effects and remote-URL
  resolution: M11.
- **Tests:** config `outward.rs` (6), corpus rows for every refspec and `gh api` hole. Disabled
  one at a time — command class, envelope guard, refspec parse, `gh api`, word check, branch-name
  check — each fails its test.
- **Learned:** the classifier takes the highest class over every argument prefix, so a guard must
  read a flag with nothing after it yet as nothing, or every `-X GET` reads as a write.

---

### [2026-10-27T17:11:56+01:00] rimuru — #101, Fable's last read: a push destination is the source's own branch
- **Done:** `refspec` takes a destination only as the source itself, `refs/heads/<source>`, or
  any `refs/heads/` branch for `HEAD`; `feat/x:refs/heads/main` and `HEAD:main` are destructive.
- **Tests:** corpus rows for both, and the two accepted forms.

---

### [2026-10-27T17:18:44+01:00] rimuru — the wire: prose that reads as a tag rides escaped
- **Done:** a report's prose lines (`other`) and a commission's body were emitted as written, so a
  prose `@S` read back as a second status (CI's `no_field_forges_a_line`, shrunk to `other = "@S"`);
  `@$`, `@R`/`@H`, `@E`, `@CAP` and raw `S|…` forged the same way — an off-the-wire child answer's
  lines ride `other` up to the dispatcher. Prose is now trimmed and escaped behind one `\` when it
  opens with `@` or `\` or reads as a raw tag line; the reader drops it only then. Line breaks fold
  for every splitter (VT, FF, FS/GS/RS, NEL, U+2028/2029); a claim folds before its fence. Docs 04.
- **Tests:** wire `prose_that_reads_as_a_tag_cannot_forge_the_envelope`, the CI seed in
  `properties.proptest-regressions`; properties `every_field_keeps_its_own_line`,
  `a_commission_body_stays_prose` (hostile pieces, 2048 cases).
- **Learned:** no stored file is read back as wire — every parse is over a live answer — so older
  journals and courier files need no migration.

---

### [2026-10-27T17:34:57+01:00] rimuru — #238, reviewed (Opus, Fable): the wire's cheap hardening
- **Done:** leading whitespace and invisible format characters drop from a prose line before
  its escape (dropped, not `\u{…}`-escaped: one escape grammar, and an invisible lead means
  nothing); the first `@S` and `@E` win; never-cut fields bounded at 8192 bytes with ` …cut`;
  `check` folds its findings; `accept` and `file_triage` read `@V` with `Report::parse`. Docs 04,
  including the dispatcher's echoed `@V` as a model echo, not a forgery.
- **Tests:** wire `the_first_status_and_close_win`, `invisible_characters_cannot_hide_a_tag`,
  `a_never_cut_field_is_bounded`; `hostile()` gains zero-width, bidi, NBSP, U+3000, FS/GS,
  `@CAP`/`@ROOT`; `round_trip` at 2048 cases.

---

### [2026-10-27T18:01:54+01:00] rimuru — #93: an outward act's receipt is observed, never an exit code
- **Done:** `mnemorph_config::PostJournal` (`.mnemorph/kijin/<post>/post.jsonl`): rows
  `committed` → `pending observation` → `observed` | `reconcile`; a `Command` is `unobserved`.
  `Planned::pin` reads the push URLs (insteadOf applied), HEAD and the `gh` host with the plan;
  `Observer` runs fixed probes on the pin (`git ls-remote`, `gh api --method GET` labels /
  comment by id with the body's SHA-256), outside every repository, 30 s, inside the guard with
  only its scratch writable. `reconciling` feeds the hello's holes (`mnemorph status`);
  `Tally` is the hook M11 folds into `outward_clean_rate`, the envelope's suspension and the
  Oni fall. Docs 02, 05.
- **Tests:** config `receipt` (local bare repos, a fake `Probe` for `gh`): match → observed;
  moved or missing branch, missing label, edited or misplaced comment, exit ≠ 0, timeout →
  reconcile; `remote set-url` after the act still probes the pinned URL; a `-`-leading pushurl
  is not pinned; credentials never reach the journal; the guard keeps a probe from writing.
  server `an_act_in_reconcile_is_shown_to_the_human`. With the judgement disabled, 6 of 12 fail.
- **Learned:** nothing executes a typed act yet: the receipt is the API that executor calls
  (pin with the plan, commit, run, `ran`, `observe`).

---

### [2026-10-27T18:25:39+01:00] rimuru — #240, reviewed (Opus, Fable): receipts that cannot read a false `observed`
- **Done:** `pin` (now with a `Probe`) refuses what the act would make if it is already there: a
  branch on any push URL (`git push` with the empty lease exits 0, "Everything up-to-date", when
  the branch sits at HEAD's commit) and a label on the PR. A typed act exiting other than 0 is
  `reconcile` with no `Done`, so no undo. Rows witnessed under
  `$XDG_STATE_HOME/mnemorph/kijin/<world>-<fnv64a>/<post>.jsonl`; the tripwire rebuilds each
  post journal as it does the queue; `tally` reads the witness alone. `status` escapes a row's
  id and why; a ref's sha is shown only when it reads. `commented` takes the pinned host (GHE).
  Docs 02, 05: GH_TOKEN-only machines reconcile; observe before gating the next act on a target.
- **Tests:** config `receipt` 18 (7 fail with the judgement off); tripwire
  `a_post_journal_keeps_the_harness_rows_and_loses_the_forged_ones`; outward GHE `commented`.
- **Learned:** the reviewers' failed-push case was wider: a lease is never checked when there is
  nothing to send, so exit 0 proved nothing there either — the pin-time read closes it.

---

### [2026-10-27T18:27:41+01:00] rimuru — the TUI: the races' 256 px animations, and the welcome at the top
- **Done:** the sidebar plays the working body's clip from mnemorph-animations `ff8af6d`
  (`assets/anims/256px/`, v2 sprite sheets; the eleven idles and five ascensions, 7.8 MB in,
  the 32 px loops and the race stills, 2.25 MB, out). Real pixels as the player's `--native`
  shows them: Kitty stores each frame once by id and places it (16 fps), iTerm2 and Sixel send the
  frame again (8 fps); frames encode off the UI's thread into reels, a new frame covers the old
  with nothing erased. Without pixels, glyph cells from octants to half blocks
  (`MNEMORPH_TUI_GLYPHS`). The sheet decoder and glyph renderer are vendored (`kit::mnanim`,
  `kit::glyphs`): the repository is private, CI has no token for it. An ascension plays when the
  body shown is seen as the risen form of its race (`Model::ascend`; no event names one yet). A
  short transcript is top-anchored, so the welcome opens the conversation like a header and
  scrolls away. Docs 05.
- **Tests:** pictures `a_kitty_clip_is_stored_once_by_id_then_placed`,
  `the_frame_cache_never_encodes_an_unchanged_frame_again`, `a_clip_plays_at_the_protocols_pace`;
  `a_body_seen_ascending_plays_its_ascension_once_then_its_risen_loop`, `idle_wants_veldora_alone`;
  welcome `on_first_run_the_welcome_opens_the_conversation_at_the_top`,
  `the_welcome_scrolls_away_and_comes_back_at_the_top_of_the_history`; mnanim and glyphs units.
  Under a pty, `MNEMORPH_TUI_IMAGES=kitty|sixel|iterm` wrote `a=t`/`a=p` by id and freed every
  stored id on exit; Sixel and iTerm2 frames of 256 × 272 px, about 86 and 67 KB.
- **Learned:** image's Lanczos in a debug build took ~50 s to shrink the eleven clips for glyphs;
  an area mean over the palette indices does it in a fraction, with no colour bleeding from clear
  pixels.

---

### [2026-10-27T18:43:43+01:00] rimuru — #241, reviewed (Fable): Kitty's frames let go, a pause out of focus, the licence
- **Done:** a reel let go (over the few kept, or on a font zoom) frees its Kitty frames by id once
  none is on screen, so a long session never fills the terminal's image quota; a stored frame is
  sent again every 32 placements, since `q=2` hides an eviction. Focus reports on: the portrait
  pauses while the window is unfocused (no ticks, nothing sent), off again on exit and panic. The
  vendored files name their MIT licence. Docs 05.
- **Tests:** pictures `a_reel_let_go_frees_its_kitty_frames_and_a_stored_frame_is_refreshed`,
  `an_unfocused_window_pauses_the_portrait`; run `releasing_turns_every_mode_off…` checks
  `?1004l`.

---

### [2026-10-27T18:01:10+01:00] rimuru — #18 decision 1: each Kijin post's outward acts are compiled in
- **Done:** `mnemorph_races::OutwardKind` (fetch, push-branch, pull-request, comment, label,
  rerun-ci, push-tag, publish, deploy, rollback, write-outside, install, other) and
  `Post::acts`: ci fetch + rerun-ci; release fetch + push-tag + publish; deploy fetch + deploy +
  rollback + write-outside; forge fetch + push-branch + pull-request + comment + label (M2);
  scout fetch only. `Post::of` reads the post from the creature's name; `check_post` refuses
  an act the post lacks, and every act of a body that holds no post. The classifier tags each
  outward raise with its kind (`classify_acts`, the same walk); an untagged outward raise is
  `other`, and the kinds are the whole command's, never a prefix's. `kubectl` and `helm` are
  read (were `write`). The bash tool checks the post after the shelf, before the gate.
  Territory stays doc-declared. Docs 02 (the posts table, the kinds table).
- **Tests:** shell `a_kijin_does_only_its_posts_acts` (forge's deploy, deploy's push, scout's
  push refused unasked; each post's acts reach the gate; no post refused); corpus
  `outward_acts_by_kind`, `every_outward_row_names_its_acts`; law
  `the_posts_acts_as_the_book_lists_them`, `a_post_holds_only_its_acts`. With the check
  disabled in `shell.rs`, the shell test fails.
- **Learned:** a destructive raise is outward only sometimes, so it gets no default kind: each
  outward one (forced push, publishes, outside deletes, uninstalls) is tagged by hand. A plain
  `git push origin v1.0` of a tag still reads as a branch push (the classifier cannot tell).

---

### [2026-10-27T18:35:14+01:00] rimuru — #239, reviewed (Opus, Fable): a kind is read only from the command's own words
- **Done:** push-branch only for one `refs/heads/X` (or `HEAD:`/`X:` onto it) to a plain
  remote; a bare push, a short name, `--follow-tags`, `--tags`, two refspecs, `ext::`, any
  git global but `-C` into the world are `other`; `git -c … push` is destructive. A walk-wide
  taint — an assignment or `env` prefix, `export`/`read`/`set`…, a loop variable, a function,
  `source`, `cd`, a `git config`/`remote`/upstream write — makes every later outward act
  `other`. Fetch is allowlisted (git, curl, wget, aria2c, httpie). `sudo` is `other`;
  `kubectl` naming secrets is refused (secret-store). `write-outside` only under
  `Context::with_deploy_roots`, never a home dotfile; run-time paths `other`. Typed acts get
  `Planned::check_post`. Escalation goes to the Kijin of the act's sole post, else the human.
  The roster refuses a `kijin-` name on another race and a Kijin heading that is not
  `kijin-<dir>`. Docs 02.
- **Tests:** corpus `outward_acts_by_kind` (every hole as a row), shell
  `a_kijin_does_only_its_posts_acts`, commission `an_act_of_no_single_post_stays_with_the_human`,
  queue `a_kijin_name_matches_its_race_and_its_directory`, config
  `a_typed_act_is_checked_against_the_post`.
- **Learned:** what a command's words name is not what it runs: the shell line before it and the
  repository's config steer it. A persisted `remote.*.pushurl` or `push.followTags` from an
  earlier call is still read only by the gate's fingerprint, not by the kind.
- **Merged with #240:** `PostJournal` knows its post and `commit` refuses an act the post does not
  hold (`a_journal_commits_only_its_posts_acts`); the untyped-command receipt test uses a forge
  command.

---

### [2026-10-27T19:24:46+01:00] rimuru — #239, Fable's last three: where a push goes, a remote by name, a download's path
- **Done:** a branch or tag push's remote is resolved before the gate (repository config only):
  one push URL equal to the fetch URL, shown on the card (credentials hidden, escaped), else
  `other`; a push names a configured remote (`mnemorph_tools::remote_name`, shared with typed
  acts), a URL in its place is `other`; `curl`/`wget`/`aria2c` output paths are judged as
  writes. Docs 02.
- **Tests:** fingerprint `the_card_shows_where_a_push_goes`,
  `a_push_url_set_before_approval_is_no_posts_push`; corpus rows for URL remotes and outputs;
  each guard disabled fails its test.

---

### [2026-10-27T19:48:24+01:00] rimuru — #94: the Kijin act runner; a cut act is probed, never replayed; request keys
- **Done:** `mnemorph_config::Runner`, behind the Kijin's `outward` tool (a typed act:
  `push-new-branch {remote, name}`, `add-label {repo, pr, label}`, `comment {repo, pr, body}`,
  optional `key`; never a command string): post check → fingerprint → pin (key looked up, target
  vacant) → the human (no envelope before M11, said on the card) → recheck at the spawn → commit
  (intent and key) → the sandboxed command (`mnemorph_tools::shell::spawn_approved`, the bash
  tool's own path) → `ran` → `observe` → receipt. One act per post at a time (a lock beside the
  witness). A cut `committed`/`pending observation` act is probed before the post's next act:
  landed → `observed`, absent → new state `absent` and re-gated when asked again, unknown →
  `reconcile`. Keys and whole pins live in `<post>.intents.jsonl` beside the witness; same key
  and request → the stored receipt, another request → refused. Witness folder `0700`, files
  `0600`. `Runner::undo` removes a label only while the one label event since the pin is the
  act's own add by `gh`'s login; else `reconcile`. Docs 02, 03, 05, roadmap.
- **Tests:** config `runner` (full pipeline, re-ask on a moved world, post refused before the
  gate, denied, cut landed/absent/unknown, comment replay and key conflict, resume after a landed
  comment, label undo re-probe); each guard (post check, recheck, probe-before-re-ask,
  idempotency, undo re-probe) disabled fails its test.
- **Learned:** a test witness is not the tripwire's (that one is found under the human's state
  directory), so a test reads the witness, not the world's rebuilt journal. Still open: the undo
  is rendered, not yet run through the gate; a push or comment undo is no forge post's kind.

---

### [2026-10-27T20:04:24+01:00] rimuru — #242, Fable's review: the runner's six holes closed
- **Done:** cut-comment recovery and replay require `gh`'s login (and the id once known), so
  someone's identical comment is never the act's; a label refuses `,` and `"`; a comment is at
  most 64 KiB, with no bidi, zero-width or U+2028/9, and the card indents continuation lines; the
  post lock is `try_lock`, a second act refused while one is in flight; an observed request
  asked again is re-probed, `reconcile` if its effect is gone. Docs 02.
- **Tests:** config `runner`: foreign identical comment, label/body refusals and the card's
  indent, two runners, a replay after the branch was deleted, a spawn that fails after its
  intent (probed `absent`, lock let go). Author, comma and re-probe guards disabled fail theirs.

---

### [2026-10-27T20:28:54+01:00] rimuru — #103: receipt-based closure for rolled-back outward acts
- **Done:** `Runner::undo` (the Kijin's `outward {kind: "undo", receipt}`) runs the
  harness-rendered undo of an act `observed` or in `reconcile`: post check, fingerprint, the act's
  pinned push URLs or host read again (retargeted → refused), what it left still its own (label
  events, comment author and body; a branch leased on its commit), the human always
  (`Uncovered::Undo`), recheck, incident opened and the undo's intent journaled, the undo, `ran`,
  and a probe that must see the effect gone (a comment by its id). A failed undo is `reconcile`;
  an effect already gone runs nothing, once approved. New states `corrected` (the act's
  superseding row, `by` the undo) and `settled`. `mnemorph_config::closure`: incidents in
  `<post>.incidents.jsonl` beside the witness, closed only on four receipts each re-read from the
  witness and intents (undone, corrected, a `tells` comment observed, a lesson checked to exist
  with its SHA-256). While open: `suspended(world, envelope)` for M11's gate (unreadable → every
  envelope), and new acts on the same target refused but the telling comment. `mnemorph incident
  list|open|settle|lesson`; open incidents in the hello's holes (`mnemorph status`). Docs 02, 05,
  roadmap.
- **Tests:** config `runner`: undo only after approval (branch gone, receipts 1–2), lease failure,
  exit 0 with the effect left, exit 1 with it gone, open at 3 of 4 and closed at 4, forged world
  rows and unbacked entries, suspension lifted on close, target refused until close, comment
  edited, reconcile marked or settled, effect already gone, retargeted remote, lessons; mnemorphd
  `incident` under a temporary `XDG_STATE_HOME`. Eleven guards disabled one by one: each fails its
  test.
- **Learned:** an undo's "gone" probe is not the cut act's "landed" probe read backwards: an
  edited comment no longer matches by body, so "not ours" would have read as "gone". A guard
  behind another guard needs a test that isolates it (exit 0 leaving the effect; exit 1 removing
  it).

---

### [2026-10-27T20:45:25+01:00] rimuru — #243, Fable's review: closure's five holes closed
- **Done:** a telling comment always passes the target check — admitted only where its own
  incident is the one open there, on the act's pull request, or for a branch on a PR whose head is
  that branch in the push URL's repository (`pulls/<pr>` probed); its body opens with the
  harness's `Rolled back act <id>: `. A lesson must have changed since the commit HEAD named at
  the open (or a Thought be born after), the CLI shows the change and asks `y`, and it is
  re-hashed on every read. What an undo removes is re-read inside the spawn's last step.
  Receipt (2) is the act's latest row. Docs 02.
- **Tests:** config `runner`: telling on another incident's PR and on an unrelated PR refused,
  prefix and a stripped intent, an unchanged lesson refused and an edited one dropped, a comment
  edited during the card, a later row over the correction; `branch_pr`, `forge_repo`;
  mnemorphd lesson declined then confirmed. Eight new guards disabled: each fails its test.
