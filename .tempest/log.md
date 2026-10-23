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

### [2026-10-24T21:15:00+02:00] rimuru — M0 foundations: the workspaces, the check, the world seeded
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
