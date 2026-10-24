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
