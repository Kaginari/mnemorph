# Changelog

Every change that reaches users is written here, newest first. Changes merged since the last
release sit under `## [Unreleased]`; cutting a release renames that header to the version and its
date (`## [1.0.0] - 2026-11-02`) and opens a new, empty `## [Unreleased]` above it. The release
workflow drafts the GitHub release's notes from the tagged version's section, and refuses a tag
whose section is missing or undated ([releasing](docs/releasing.md)).

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

The notes for 1.0.0, the first release, until it is cut: the Rust core, the `mnemorph` terminal
app and `mnemorph run`, the `mnemorphd` server, the board and the admin commands, the TypeScript
SDK, and the book. Grouped by the [roadmap](docs/roadmap.md)'s milestones.

### M14 — 1.0

- The release-candidate smoke test: the built binaries, in a fresh HOME with realistic dotfiles
  and a world with history, against a scripted provider on loopback (`just rc-smoke`).
- `install.sh`: the release for this OS and architecture, verified against `SHA256SUMS` before it
  is unpacked, into `~/.local/bin`, no sudo, no profile edits; tested offline (`just install-test`).
- The release workflow: on a `v*` tag, archives for Linux and macOS on x86_64 and arm64, their
  checksums (and build provenance, once turned on), smoke-tested, then a draft release for
  review.
- This changelog.

### M13 — Integrations

- MCP client: race-scoped tools by class, three priced levels, lazy and health-checked servers,
  BM25 ranking with a fast-tier rerank, learned from a chained journal, a stable prefix and
  native deferral [#270] [#299] [#300] [#301].
- The index tool: a tree-sitter skeleton with line ranges; a large file read by range [#269].
- Relations between worlds: a registry, harness sightings, named-twice proposals, gluing [#271].
- Recorded provider cassettes for OpenRouter and Ollama, provider-reported cost, unknown usage
  estimated and unpriced [#285] [#303].

### M12 — Board

- `mnemorphd board`: a secure loopback server, keyed once, traded for a session cookie [#286].
- The board: shell, router, themes and safe text [#287]; the session protocol over WebSocket
  [#289]; worlds and the world overview [#291]; chat [#292]; memory and ontology [#293]; web
  approvals under the terminal's protections [#294]; metrics and integrations [#295]; bodies,
  race cards, queue, timeline and journal [#296]; polish and the M12 exit [#297].
- One escaping range list for Rust and the board: every format, separator and default-ignorable
  character shown, never obeyed [#290].

### M11 — Value, evolution, ascension

- Oni envelopes: signed, expiring, capped, revocable, and honoured by the gate [#108] [#110]
  [#118] [#262].
- Meters with their count and 95% interval; value rows stamped with their build [#166] [#260]
  [#267] [#268].
- Hash-chained trust journals, the head anchored in `log.md` [#261] [#267].
- Explainable falls and honest bad news [#263]; lifecycles explored whole, with reachability
  witnesses [#265].
- Evolution judged after the freeze, against a do-nothing control, on an exact staged file set
  [#264] [#268] [#298]; a claim graph for memory [#266].

### M10 — Dark Elf

- The specialities, attack classes, surfaces and route, compiled; the finding fields on the wire
  [#245] [#246] [#247].
- The finding record and its validator, the coverage ledger, the compiled playbooks [#251] [#252]
  [#254].
- Challenge routing at the gate; independent verification before a finding counts [#255] [#256].
- The seeded fixture repository, the ledger's fold per speciality, the law audit's checklist and
  the reviewers' calibration [#257] [#258] [#259]; Mind provenance a body cannot forge [#272].

### M9 — Kijin

- Each post's outward acts compiled in [#239]; a plan with no declared undo is destructive [#234].
- Receipts observed by a read-only probe, never by exit 0; a cut act probed, never replayed;
  receipt-based closure [#240] [#242] [#243].
- A guarded attic restore [#235]; an approval bound to the plan's fingerprint [#237].
- A typed pull-request act: a branch and its pull request under one approval [#248].

### M8 — Memory and toolbox

- Notes as events, recall with a seat for dissent, the write rule [#192] [#193] [#194] [#195].
- Reconsolidation, consolidation and forgetting, the unproven write, the replay loop and lock-in
  [#197] [#198] [#199] [#200] [#202].
- Session start, the compaction drain, the toolbox [#201] [#203] [#205].
- Citation by use, the accepted, corroborated and confirmed routes, a correction standing beside
  its note, recall's weights in config, the triage fixture [#206] [#207] [#208] [#213] [#215]
  [#216] [#217] [#218]; the ontology mesh live [#214].

### M7 — The world

- Creatures as data and ownership as one function; the closed graph, each body's soil, the shapes
  as findings [#138] [#143] [#144].
- Dispatch, the right author, the commission runner, territory facts [#145] [#146] [#149] [#150]
  [#151] [#152] [#153].
- The shared work queue, the wake, the journal's writer, grants bounded by worth, the top-up
  [#154] [#155] [#156] [#157] [#158] [#159].
- Hardening after three fails, triage, `housekeep --heal`, relief, the council, the clock-death
  memo, offices; a slice that runs out is never a failure [#160] [#161] [#162] [#163] [#164]
  [#165] [#167] [#168] [#169].
- Solo mode escalates on signals read by code; the solo-against-hierarchy bench, judged by why
  each run ended, with the law arm and paired intervals [#170] [#171] [#181] [#182] [#307].
- The verify tool, the report folder, PASS / FAIL / NOT_VERIFIED at the end gate, unrun checks
  blameless [#173] [#174] [#175] [#176] [#178].
- Never routed above the parent; ontology findings with codes; heirs and shared Minds [#177]
  [#179] [#180].
- Append-only requests; the provenance lint; refusals that name the owner; writes attributed per
  body [#187] [#188] [#189] [#190] [#191] [#231].
- Decisions: System One behind config, bounded, Jev first with the LLM answering when none is
  there [#112] [#113] [#116] [#119] [#122] [#305] [#308]; the offices and Metis [#117] [#121].
- The session lands through the end gate and suggests its verify line; step budgets per race
  [#172] [#186] [#219] [#220] [#302].

### M6 — Protocol, SDK, CLI

- The client protocol over stdio, generated for TypeScript, the SDK and a first terminal client
  [#123] [#133] [#134].
- `mnemorph`, the terminal app in Rust: sidebar, mouse, themes, the race portraits in real pixels
  and their animations, the welcome [#222] [#224] [#225] [#226] [#227] [#229] [#232] [#241].
- One `mnemorph`: the Rust binary is the command, `mnemorph run` for scripts, the TypeScript CLI
  retired; why there are two binaries [#250] [#253].
- `mnemorph resume <run>`: a stopped run picked up through the session's one opening [#233].
- Budget caps enforced, `/send`, an approval that starts unchosen, the milestone review's fixes
  [#135] [#136] [#137] [#139] [#142].

### M5b — Long runs

- Esc and the clock reach a running command; replay compaction and the absolute stress line
  [#82] [#83].
- The loop breaker, the bench tally, the prefix ceiling [#84] [#85].
- Structured compaction: a full context summarised and the run goes on [#209] [#211] [#212].

### M5 — One body loops

- The loop core, solo mode and the end gate, the session's queue and Esc, a live solo task whose
  spend matches the ledger [#74] [#75] [#76] [#77] [#79].
- The attic: what commands take away comes back, and it prunes itself [#80] [#81] [#185].
- A call with broken arguments is answered, not fatal; a solo answer keeps its claims [#210]
  [#223] [#228].

### M4a — Config

- Configuration: global and world-local, the holes said, the gate and sandbox only loosened by
  the global file [#70]; the end gate's verify limit [#221].

### M4 — Tools, sandbox, human gate

- The shell classifier, the file tools, the human gate and bash [#62] [#64] [#65].
- The sandbox and its escape suite; a kernel guard, not a jail — no guard, no command [#66] [#68]
  [#73]; credentials closed inside toolchains [#183] [#230]; the harness non-dumpable [#288].
- Files opened beneath the world, no check-then-open race [#71]; multiedit, move, delete, patch,
  a forgiving edit [#147] [#148] [#236]; the closing review's fixes [#72].

### M3 — Providers

- The Anthropic client and the OpenAI-compatible client, one driver for every provider [#50]
  [#51]; live OpenRouter tests and real cassettes [#54]; the conversation cached [#109].

### M2 — Ledger and meters

- The ledger: count, carve and stop before the first model call [#37]; a torn line counted high,
  unseen spend drawn from a hidden reserve [#140] [#141].

### M1 — Wire and races

- The wire's envelope, parsed and emitted [#33]; prose that reads as a tag rides escaped [#238].
- The race law, compiled and covered whole [#35].

### M0 — Foundations

- The workspaces, layered crates, `just check`, CI on Linux and macOS, the world seeded [#31].
- The book as a website; every diagram parsed in CI; no conflict marker anywhere tracked [#32]
  [#36] [#114].

### The book and the brand

- The book's pages: the lanes, MCP, memory, the ontology, permanent solo mode, isekai's self-heal,
  the audit against the code [#46] [#47] [#49] [#53] [#60] [#69] [#87] [#120] [#249].
- The name Mnemorph, the emblem, the banner, the race icons and cards, `@mnemorph/theme` [#55]
  [#56] [#57] [#58] [#59] [#61].

[Unreleased]: https://github.com/Kaginari/mnemorph/commits/main
[#31]: https://github.com/Kaginari/mnemorph/pull/31
[#32]: https://github.com/Kaginari/mnemorph/pull/32
[#33]: https://github.com/Kaginari/mnemorph/pull/33
[#35]: https://github.com/Kaginari/mnemorph/pull/35
[#36]: https://github.com/Kaginari/mnemorph/pull/36
[#37]: https://github.com/Kaginari/mnemorph/pull/37
[#46]: https://github.com/Kaginari/mnemorph/pull/46
[#47]: https://github.com/Kaginari/mnemorph/pull/47
[#49]: https://github.com/Kaginari/mnemorph/pull/49
[#50]: https://github.com/Kaginari/mnemorph/pull/50
[#51]: https://github.com/Kaginari/mnemorph/pull/51
[#53]: https://github.com/Kaginari/mnemorph/pull/53
[#54]: https://github.com/Kaginari/mnemorph/pull/54
[#55]: https://github.com/Kaginari/mnemorph/pull/55
[#56]: https://github.com/Kaginari/mnemorph/pull/56
[#57]: https://github.com/Kaginari/mnemorph/pull/57
[#58]: https://github.com/Kaginari/mnemorph/pull/58
[#59]: https://github.com/Kaginari/mnemorph/pull/59
[#60]: https://github.com/Kaginari/mnemorph/pull/60
[#61]: https://github.com/Kaginari/mnemorph/pull/61
[#62]: https://github.com/Kaginari/mnemorph/pull/62
[#64]: https://github.com/Kaginari/mnemorph/pull/64
[#65]: https://github.com/Kaginari/mnemorph/pull/65
[#66]: https://github.com/Kaginari/mnemorph/pull/66
[#68]: https://github.com/Kaginari/mnemorph/pull/68
[#69]: https://github.com/Kaginari/mnemorph/pull/69
[#70]: https://github.com/Kaginari/mnemorph/pull/70
[#71]: https://github.com/Kaginari/mnemorph/pull/71
[#72]: https://github.com/Kaginari/mnemorph/pull/72
[#73]: https://github.com/Kaginari/mnemorph/pull/73
[#74]: https://github.com/Kaginari/mnemorph/pull/74
[#75]: https://github.com/Kaginari/mnemorph/pull/75
[#76]: https://github.com/Kaginari/mnemorph/pull/76
[#77]: https://github.com/Kaginari/mnemorph/pull/77
[#79]: https://github.com/Kaginari/mnemorph/pull/79
[#80]: https://github.com/Kaginari/mnemorph/pull/80
[#81]: https://github.com/Kaginari/mnemorph/pull/81
[#82]: https://github.com/Kaginari/mnemorph/pull/82
[#83]: https://github.com/Kaginari/mnemorph/pull/83
[#84]: https://github.com/Kaginari/mnemorph/pull/84
[#85]: https://github.com/Kaginari/mnemorph/pull/85
[#87]: https://github.com/Kaginari/mnemorph/pull/87
[#108]: https://github.com/Kaginari/mnemorph/pull/108
[#109]: https://github.com/Kaginari/mnemorph/pull/109
[#110]: https://github.com/Kaginari/mnemorph/pull/110
[#112]: https://github.com/Kaginari/mnemorph/pull/112
[#113]: https://github.com/Kaginari/mnemorph/pull/113
[#114]: https://github.com/Kaginari/mnemorph/pull/114
[#116]: https://github.com/Kaginari/mnemorph/pull/116
[#117]: https://github.com/Kaginari/mnemorph/pull/117
[#118]: https://github.com/Kaginari/mnemorph/pull/118
[#119]: https://github.com/Kaginari/mnemorph/pull/119
[#120]: https://github.com/Kaginari/mnemorph/pull/120
[#121]: https://github.com/Kaginari/mnemorph/pull/121
[#122]: https://github.com/Kaginari/mnemorph/pull/122
[#123]: https://github.com/Kaginari/mnemorph/pull/123
[#133]: https://github.com/Kaginari/mnemorph/pull/133
[#134]: https://github.com/Kaginari/mnemorph/pull/134
[#135]: https://github.com/Kaginari/mnemorph/pull/135
[#136]: https://github.com/Kaginari/mnemorph/pull/136
[#137]: https://github.com/Kaginari/mnemorph/pull/137
[#138]: https://github.com/Kaginari/mnemorph/pull/138
[#139]: https://github.com/Kaginari/mnemorph/pull/139
[#140]: https://github.com/Kaginari/mnemorph/pull/140
[#141]: https://github.com/Kaginari/mnemorph/pull/141
[#142]: https://github.com/Kaginari/mnemorph/pull/142
[#143]: https://github.com/Kaginari/mnemorph/pull/143
[#144]: https://github.com/Kaginari/mnemorph/pull/144
[#145]: https://github.com/Kaginari/mnemorph/pull/145
[#146]: https://github.com/Kaginari/mnemorph/pull/146
[#147]: https://github.com/Kaginari/mnemorph/pull/147
[#148]: https://github.com/Kaginari/mnemorph/pull/148
[#149]: https://github.com/Kaginari/mnemorph/pull/149
[#150]: https://github.com/Kaginari/mnemorph/pull/150
[#151]: https://github.com/Kaginari/mnemorph/pull/151
[#152]: https://github.com/Kaginari/mnemorph/pull/152
[#153]: https://github.com/Kaginari/mnemorph/pull/153
[#154]: https://github.com/Kaginari/mnemorph/pull/154
[#155]: https://github.com/Kaginari/mnemorph/pull/155
[#156]: https://github.com/Kaginari/mnemorph/pull/156
[#157]: https://github.com/Kaginari/mnemorph/pull/157
[#158]: https://github.com/Kaginari/mnemorph/pull/158
[#159]: https://github.com/Kaginari/mnemorph/pull/159
[#160]: https://github.com/Kaginari/mnemorph/pull/160
[#161]: https://github.com/Kaginari/mnemorph/pull/161
[#162]: https://github.com/Kaginari/mnemorph/pull/162
[#163]: https://github.com/Kaginari/mnemorph/pull/163
[#164]: https://github.com/Kaginari/mnemorph/pull/164
[#165]: https://github.com/Kaginari/mnemorph/pull/165
[#166]: https://github.com/Kaginari/mnemorph/pull/166
[#167]: https://github.com/Kaginari/mnemorph/pull/167
[#168]: https://github.com/Kaginari/mnemorph/pull/168
[#169]: https://github.com/Kaginari/mnemorph/pull/169
[#170]: https://github.com/Kaginari/mnemorph/pull/170
[#171]: https://github.com/Kaginari/mnemorph/pull/171
[#172]: https://github.com/Kaginari/mnemorph/pull/172
[#173]: https://github.com/Kaginari/mnemorph/pull/173
[#174]: https://github.com/Kaginari/mnemorph/pull/174
[#175]: https://github.com/Kaginari/mnemorph/pull/175
[#176]: https://github.com/Kaginari/mnemorph/pull/176
[#177]: https://github.com/Kaginari/mnemorph/pull/177
[#178]: https://github.com/Kaginari/mnemorph/pull/178
[#179]: https://github.com/Kaginari/mnemorph/pull/179
[#180]: https://github.com/Kaginari/mnemorph/pull/180
[#181]: https://github.com/Kaginari/mnemorph/pull/181
[#182]: https://github.com/Kaginari/mnemorph/pull/182
[#183]: https://github.com/Kaginari/mnemorph/pull/183
[#185]: https://github.com/Kaginari/mnemorph/pull/185
[#186]: https://github.com/Kaginari/mnemorph/pull/186
[#187]: https://github.com/Kaginari/mnemorph/pull/187
[#188]: https://github.com/Kaginari/mnemorph/pull/188
[#189]: https://github.com/Kaginari/mnemorph/pull/189
[#190]: https://github.com/Kaginari/mnemorph/pull/190
[#191]: https://github.com/Kaginari/mnemorph/pull/191
[#192]: https://github.com/Kaginari/mnemorph/pull/192
[#193]: https://github.com/Kaginari/mnemorph/pull/193
[#194]: https://github.com/Kaginari/mnemorph/pull/194
[#195]: https://github.com/Kaginari/mnemorph/pull/195
[#197]: https://github.com/Kaginari/mnemorph/pull/197
[#198]: https://github.com/Kaginari/mnemorph/pull/198
[#199]: https://github.com/Kaginari/mnemorph/pull/199
[#200]: https://github.com/Kaginari/mnemorph/pull/200
[#201]: https://github.com/Kaginari/mnemorph/pull/201
[#202]: https://github.com/Kaginari/mnemorph/pull/202
[#203]: https://github.com/Kaginari/mnemorph/pull/203
[#205]: https://github.com/Kaginari/mnemorph/pull/205
[#206]: https://github.com/Kaginari/mnemorph/pull/206
[#207]: https://github.com/Kaginari/mnemorph/pull/207
[#208]: https://github.com/Kaginari/mnemorph/pull/208
[#209]: https://github.com/Kaginari/mnemorph/pull/209
[#210]: https://github.com/Kaginari/mnemorph/pull/210
[#211]: https://github.com/Kaginari/mnemorph/pull/211
[#212]: https://github.com/Kaginari/mnemorph/pull/212
[#213]: https://github.com/Kaginari/mnemorph/pull/213
[#214]: https://github.com/Kaginari/mnemorph/pull/214
[#215]: https://github.com/Kaginari/mnemorph/pull/215
[#216]: https://github.com/Kaginari/mnemorph/pull/216
[#217]: https://github.com/Kaginari/mnemorph/pull/217
[#218]: https://github.com/Kaginari/mnemorph/pull/218
[#219]: https://github.com/Kaginari/mnemorph/pull/219
[#220]: https://github.com/Kaginari/mnemorph/pull/220
[#221]: https://github.com/Kaginari/mnemorph/pull/221
[#222]: https://github.com/Kaginari/mnemorph/pull/222
[#223]: https://github.com/Kaginari/mnemorph/pull/223
[#224]: https://github.com/Kaginari/mnemorph/pull/224
[#225]: https://github.com/Kaginari/mnemorph/pull/225
[#226]: https://github.com/Kaginari/mnemorph/pull/226
[#227]: https://github.com/Kaginari/mnemorph/pull/227
[#228]: https://github.com/Kaginari/mnemorph/pull/228
[#229]: https://github.com/Kaginari/mnemorph/pull/229
[#230]: https://github.com/Kaginari/mnemorph/pull/230
[#231]: https://github.com/Kaginari/mnemorph/pull/231
[#232]: https://github.com/Kaginari/mnemorph/pull/232
[#233]: https://github.com/Kaginari/mnemorph/pull/233
[#234]: https://github.com/Kaginari/mnemorph/pull/234
[#235]: https://github.com/Kaginari/mnemorph/pull/235
[#236]: https://github.com/Kaginari/mnemorph/pull/236
[#237]: https://github.com/Kaginari/mnemorph/pull/237
[#238]: https://github.com/Kaginari/mnemorph/pull/238
[#239]: https://github.com/Kaginari/mnemorph/pull/239
[#240]: https://github.com/Kaginari/mnemorph/pull/240
[#241]: https://github.com/Kaginari/mnemorph/pull/241
[#242]: https://github.com/Kaginari/mnemorph/pull/242
[#243]: https://github.com/Kaginari/mnemorph/pull/243
[#245]: https://github.com/Kaginari/mnemorph/pull/245
[#246]: https://github.com/Kaginari/mnemorph/pull/246
[#247]: https://github.com/Kaginari/mnemorph/pull/247
[#248]: https://github.com/Kaginari/mnemorph/pull/248
[#249]: https://github.com/Kaginari/mnemorph/pull/249
[#250]: https://github.com/Kaginari/mnemorph/pull/250
[#251]: https://github.com/Kaginari/mnemorph/pull/251
[#252]: https://github.com/Kaginari/mnemorph/pull/252
[#253]: https://github.com/Kaginari/mnemorph/pull/253
[#254]: https://github.com/Kaginari/mnemorph/pull/254
[#255]: https://github.com/Kaginari/mnemorph/pull/255
[#256]: https://github.com/Kaginari/mnemorph/pull/256
[#257]: https://github.com/Kaginari/mnemorph/pull/257
[#258]: https://github.com/Kaginari/mnemorph/pull/258
[#259]: https://github.com/Kaginari/mnemorph/pull/259
[#260]: https://github.com/Kaginari/mnemorph/pull/260
[#261]: https://github.com/Kaginari/mnemorph/pull/261
[#262]: https://github.com/Kaginari/mnemorph/pull/262
[#263]: https://github.com/Kaginari/mnemorph/pull/263
[#264]: https://github.com/Kaginari/mnemorph/pull/264
[#265]: https://github.com/Kaginari/mnemorph/pull/265
[#266]: https://github.com/Kaginari/mnemorph/pull/266
[#267]: https://github.com/Kaginari/mnemorph/pull/267
[#268]: https://github.com/Kaginari/mnemorph/pull/268
[#269]: https://github.com/Kaginari/mnemorph/pull/269
[#270]: https://github.com/Kaginari/mnemorph/pull/270
[#271]: https://github.com/Kaginari/mnemorph/pull/271
[#272]: https://github.com/Kaginari/mnemorph/pull/272
[#285]: https://github.com/Kaginari/mnemorph/pull/285
[#286]: https://github.com/Kaginari/mnemorph/pull/286
[#287]: https://github.com/Kaginari/mnemorph/pull/287
[#288]: https://github.com/Kaginari/mnemorph/pull/288
[#289]: https://github.com/Kaginari/mnemorph/pull/289
[#290]: https://github.com/Kaginari/mnemorph/pull/290
[#291]: https://github.com/Kaginari/mnemorph/pull/291
[#292]: https://github.com/Kaginari/mnemorph/pull/292
[#293]: https://github.com/Kaginari/mnemorph/pull/293
[#294]: https://github.com/Kaginari/mnemorph/pull/294
[#295]: https://github.com/Kaginari/mnemorph/pull/295
[#296]: https://github.com/Kaginari/mnemorph/pull/296
[#297]: https://github.com/Kaginari/mnemorph/pull/297
[#298]: https://github.com/Kaginari/mnemorph/pull/298
[#299]: https://github.com/Kaginari/mnemorph/pull/299
[#300]: https://github.com/Kaginari/mnemorph/pull/300
[#301]: https://github.com/Kaginari/mnemorph/pull/301
[#302]: https://github.com/Kaginari/mnemorph/pull/302
[#303]: https://github.com/Kaginari/mnemorph/pull/303
[#305]: https://github.com/Kaginari/mnemorph/pull/305
[#307]: https://github.com/Kaginari/mnemorph/pull/307
[#308]: https://github.com/Kaginari/mnemorph/pull/308
