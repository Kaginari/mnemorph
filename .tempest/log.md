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
