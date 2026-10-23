# Lessons carried from isekai

Every row below is a failure isekai actually shipped, hit in use, or caught late. Tempest does not
carry them as advice: each row becomes either a test that fails if the mistake comes back, or a
design rule the code makes impossible to break, and the milestone column names the rung of the
roadmap where that test or rule must exist before the rung counts as done.

## Provider & streaming

| Lesson | Becomes | Milestone |
|---|---|---|
| The mock provider never encoded a request, so a tool schema that no real API accepted passed every mock test and broke every live call. | The mock provider serializes every request to JSON exactly as the real clients do. Test: every tool on every shelf round-trips through the request encoder as valid JSON Schema; a tool with no parameters sends `{"type":"object"}`. | M3 |
| Gemini attaches a `thought_signature` to each tool call and rejects the next turn without it: an OpenAI-compatible endpoint is not an OpenAI-identical one. | A tool call carries an opaque, provider-owned `extra` blob that is replayed byte for byte on the next request. Test: a recorded Gemini fixture replays the signature. | M3 |
| An upstream `finish_reason: "error"` (sent under HTTP 200) was mapped to a normal stop and taken as a finished answer. | The stop-reason enum has an `Error` variant that is a retryable failure, never an answer. Test: plain and streamed fixtures ending in `error`. | M3 |
| An Anthropic stream cut before `message_stop` was accepted as a whole answer. | A stream without its terminal event is an error. Test: truncated-stream fixture for every client. | M3 |
| A tool with zero arguments arrived as a malformed call, and redacted thinking lost its data on replay. | Test: a no-argument tool call decodes as `{}`; redacted and signed thinking blocks replay verbatim. | M3 |
| Free tiers answer 503 under load, Anthropic answers 429/529, only one client retried, and a gateway 404 for a renamed model ended the turn instead of trying the fallback. | One retry policy shared by every client: 429/5xx/529 retried, `Retry-After` honoured, else 1s/2s/4s, three times, never past the turn's deadline, backoff clock injectable; a "model not found" 404 advances the fallback chain. Test: mock sequences 503→200, 429+Retry-After and 404→fallback per client. | M3 |
| An error body came as an object or as an array, and the client reported "unreadable body" instead of the cause. | The error decoder accepts both shapes and always surfaces the raw text when it cannot parse. Test: fixtures for each shape plus garbage. | M3 |
| A stream-stall timer that started at request time killed vLLM/Ollama calls that send nothing during queueing or prefill. | Separate timeouts: connect, first byte (generous), then inter-chunk stall (opens on the first byte). Test: a slow-prefill mock is not killed; a stall after the first byte is. | M3 |
| Model ids were "corrected" from memory twice, in both directions, and the default model was wrong for a whole wave because the tests were written from the same guess. | Model ids live only in the registry table; no model-id literal outside it. Test: the configured default resolves in the registry, and the registry is checked against the provider's live model list in CI where a key exists. | M3 |

## Tools & classification

| Lesson | Becomes | Milestone |
|---|---|---|
| A deny rule held only against the literal command: `git -C . push`, `env git push`, `sh -c 'git push'`, `xargs`, `sudo`, subshells and absolute or escaped names (`/usr/bin/git push`, `\rm`) all slipped past. | The classifier parses the shell into an AST, unwraps wrappers (`env`, `sudo`, `xargs`, `sh -c`, `nohup`, `time`), skips git's global options, normalizes the command name, and classifies every inner simple command. Test: a block/allow corpus fixture, 100% required in CI. | M4 |
| A prefix allow such as `npm test*` also allowed `npm test && curl …`, `npm test & curl` and `npm test $(curl …)`. | An allow matches one simple command; every segment of a compound command and every substitution is classified on its own and needs its own allow. Test: chained forms in the corpus. | M4 |
| Brace expansion and naive quote skipping hid `/etc/{hosts,passwd}`, `{a,/etc/x}` and `ls /etc/` from path classification. | Expand braces as the shell does before path classification; only a quote-led token is a literal pattern. Corpus rows. | M4 |
| A classifier that read argv data (git args, custom-tool params) could downgrade a command, letting its own input overrule the read table. | Data-driven classification may only tighten (toward outward/destructive), never loosen unknown→write. Property test: adding argv never lowers the class. | M4 |
| `/dev/tcp` redirects and writes into `/etc`, `/usr`, `/opt` counted as inside the world, and `/library` was read as `/lib`. | Paths are matched by whole segments; `/dev/tcp|udp` is outward; system directories are outward for writes. Corpus rows. | M4 |
| The shell expands `~` before a hook sees the command, so `rm -rf /home/you` passed a list that only knew `rm -rf ~`. | The guard resolves the home directory to its real path and matches both forms. Test: both forms refused. | M4 |
| In-place `sed -i` edits failed silently (wrong anchor → exit 0, file untouched, no write recorded) and fed hundreds of unverified re-reads. | `sed -i` and its variants are refused by the guard with a pointer to the `edit` tool; `edit` errors on zero matches (naming the closest line) and on multiple matches (naming the line numbers). | M4 |
| A malformed native tool call ran the tool on input it misread. | Invalid JSON arguments are returned to the model as a named error ("the arguments to read are not valid JSON"); the tool is not run. | M4 |
| A builtin tool needed its seat in four places (map, struct, defaults row, shelf list), and a missing seat was found by a panic. | A tool is declared once (one trait impl plus a registry entry); config rows, schema and shelf membership are derived. Test: every registered tool has a schema, a default, and a shelf seat. | M4 |
| A timeout killed the command but not its descendants, waits hung on pipes a backgrounded child kept open, and bash's "Killed" notice leaked into the next command's output. | Every subprocess the harness spawns gets its own process group, killed whole at the deadline, with a bounded wait on output pipes; the persistent shell runs without job-control notices and frames each output with sentinels. Test: a backgrounded child holding stdout does not hang, and the next output is clean. | M4 |
| A guard that hashed answers read the read tool's "unchanged" pointer as a new answer. | Tool results carry structured metadata (`unchanged`, `wrote`, `class`); consumers read the metadata, never infer it from text. | M4 |

## Sandbox & guard

| Lesson | Becomes | Milestone |
|---|---|---|
| Symlinks escaped the world's border for read, write and edit because the border check compared literal paths. | Every border check canonicalizes the path (including the parent of a not-yet-existing target) first. Test: symlink-escape attempts through every file tool. | M4 |
| `bwrap --tmpfs /tmp` hid test temp dirs and helper binaries; a world under `/tmp` worked only by mount order. | Sandbox mount order is a tested contract; helpers a sandboxed process must reach live under the harness's own dir, never `$TMPDIR`. Test: a world rooted under `/tmp` and a helper both reachable. | M4 |
| The end gate ran model-added verify lines unsandboxed with the full environment. | Every command the harness runs on its own behalf (verify lines, hooks, rule checks, goal validation, custom tools) runs under the sandbox: no network, secrets scrubbed, process group killed at timeout. | M4 |
| The sandbox exposed the host's processes and the user's credentials (`~/.ssh`, `~/.aws`, `~/.netrc`, keyrings), and the env scrubber missed `AWS_SECRET_ACCESS_KEY` and password-bearing URLs. | Each command gets its own pid/ipc/uts namespaces; credential paths are masked empty while the world stays writable; the scrubber matches `KEY`, `TOKEN`, `SECRET`, `PASSWORD`, `CREDENTIAL` and any URL with userinfo. Test: probes inside the sandbox see none of it. | M4 |
| Before the guard corpus existed, 37 of 90 must-block commands (secret stores, disk tools, reflog purges, a fork bomb, `$HOME` paths) ran as silent reads or writes. | The guard is a denylist checked before any gate, with a block/allow corpus shipped as a test fixture; a new pattern lands with its corpus rows. | M4 |
| The benchmark smoke ran with sandbox off and gate off, so it proved plumbing, not containment. | Containment has its own attack suite, separate from benchmarks, run against the built binary. | M4 |
| A "trust the container" switch keyed on an environment marker that a process inside could also set. | Trust that loosens the gate never derives from an environment variable the model can write; it comes only from the harness's own launch argv, and the guard denylist still refuses first. | M10 |
| A macOS sandbox profile could be refused while the harness still looked sandboxed. | `status` reports the sandbox actually in force, or `none — <reason>`; containment is never claimed. | M4 |

## Human gate & permissions

| Lesson | Becomes | Milestone |
|---|---|---|
| The tracked project config could switch the human gate off or pre-approve outward and destructive acts for everyone who cloned it. | The tracked project layer may only tighten. `humanGate.enabled: false`, `approve: [outward|destructive]` and allows on outward/destructive acts are refused from tracked config and honoured only from the local or global layer. Test per key. | M4 |
| An env var or flag was treated as the human's word for the gate, and a bare `*` allow on outward or destructive classes was accepted. | The gate may be loosened only from a config file, never env or flags; a bare `*` on outward/destructive is refused at load (a rule loosens one named command); every standing approval is listed in `status`. | M4 |
| An agent restating "the human said it's fine" was offered as consent. | Consent enters only through the gate channel (TUI, web, CLI answer). Text from any model, subagent or tool result can never satisfy the gate. Test: a report claiming approval does not approve. | M4 |
| Answering the same approval from the web board and the terminal could race or hang. | First answer wins atomically; the loser gets "answered elsewhere". Test: concurrent answers. | M4 |
| A recommended option on an approval card nudges the human toward the risky choice. | Questions may carry a recommendation that pre-seats the cursor but never auto-picks; approvals over outward/destructive acts never carry one. | M4 |

## Loop, budgets & stress

| Lesson | Becomes | Milestone |
|---|---|---|
| The gate was blind to shell writes: only the write/edit tools triggered it. | Writes are detected by snapshotting the world tree at turn open, after each write-class step and at turn end, whatever tool made them. Test: `bash -c 'echo x > f'` reaches the gate. | M5 |
| An early-stopped turn lost the writes it had already made, so resume and the gate read a false transcript. | Writes are recorded in the transcript as they happen, not at turn end. Test: interrupt mid-turn, the writes are in the resumed transcript. | M5 |
| An ask after a stopped turn produced two consecutive user messages and the provider rejected it. | A message-sequence validator (role alternation, every `tool_use` paired with its `tool_result`) runs before every provider call. Property test over random interrupt/queue sequences. | M5 |
| Context occupancy was confused with spend, and a usage reader silently dropped cache-read and cache-write tokens. | Two meters, one implementation each: occupancy (the last assistant turn's `input + cache_read + cache_creation`) and spend (cumulative, all input classes). Terminal, board and stress check call the same functions. Test: fixtures with cache tokens per provider. | M2 |
| A continued session re-sends its whole history every turn, so cost keeps climbing despite high cache hit rates, and a convention's up-front reading cost (about 3× on a measured task) rides every later turn. | The fixed per-session prompt cost (law, cards, manifests) is metered on its own line and budgeted; a benchmark compares runs with and without it before any law text grows. | M2 |
| Stress "mode" cannot evict context; the real levers are: write what is not durable, push heavy work to a fresh body, tell the human to break. | Stress is an absolute-token threshold (model-independent, configurable), and crossing it triggers those three acts in code, not a prompt suggestion. | M5 |
| A doom-loop relief left its streak and valve armed, so the next turn checkpointed after one act and the session could not work. | Every relief resets its own trigger. Test: a session that slides again after relief must earn its doom afresh. | M5 |
| One body hit the same test bench 63 times in 20 minutes, and every health check stayed silent because piped output hid the exit status. | Per-bench tally (visits, red, green, muted); a piped bench never counts as green; thrash past a threshold speaks into the body's own tool result. Test: the write-between-every-run shape still triggers. | M5 |
| Tool-output replay dominated long runs (a 12-tool run sent 67 KB of replay). | Replay compaction is a pure function over a copy of the transcript: old tool outputs fold to captions, thinking blocks bound to a `tool_use` are kept. Test: byte counts before and after, and the raw transcript untouched. | M5 |
| A plan from an earlier session held the end gate open and showed on the rail as live. | Session-scoped state is stamped with the session's birth; a stale plan is memory, never a gate-holder. | M5 |

## World: territory, gate, dispatch

| Lesson | Becomes | Milestone |
|---|---|---|
| A body could rewrite the verify line it was judged by, because the gate reloaded the doc after the turn; checks a model runs can be skipped or weakened by the turn they judge. | The orc's checks are code the harness runs at the end of the authoring turn, from a verify set captured at turn open; a line added in the same turn is a hole, not a reading. Test: a rewritten verify fails the gate. | M7 |
| The turn-watch charged every write in the open window to the session whose turn was open, so sibling bodies failed each other's gates. | Write attribution is per body (per sandbox, or file events tagged by process). Test: two concurrent bodies writing distinct zones each pass their own gate. | M7 |
| "Tests intact" measured against git HEAD would count the human's own uncommitted edits. | The baseline for any before/after check is the harness's own snapshot at turn open. Test: lost tests and new skips fail; new tests pass. | M7 |
| A machine-wide agent file named like the throne was imported as a foreign creature, so every session ran as an elf; separately, the throne carried an office whose model route could re-route the session. | The throne's name is reserved and discovery never imports under it; the throne holds no office. Tests run with a populated fixture HOME, not an empty one. | M7 |
| A body reporting `@S FAIL` ran its lifecycle to `DONE` and judges read the lifecycle; a body that wrote nothing never met the gate, so no verdict reached the log. | The verdict is a distinct type parsed from the report's wire, never the lifecycle status; every dispatch lands exactly one log row, written or not. | M7 |
| 42% of one session's tokens were higher ranks relaying a lower rank's text word for word. | Reports above a size floor are filed on disk; the parent receives an envelope (verdict, path, checksum, wrote set, short digest). Test: envelope rides, the full text is filed and intact. | M7 |
| A wire report ran 70% over its byte cap. | `@CAP` is enforced by the encoder, cutting the least important lines first and never `@S`, `@?` or `@U`. Test: oversize report keeps those lines. | M1 |
| Bodies re-derived "who owns this file" from scratch at every boundary hit and burned a day on it. | An out-of-territory refusal names the owner and carries the three arms (widen, birth, route); one dispatch-card read gives owner, rank, strain and domain. | M7 |
| Territory as a file list left new files ownerless; one wide flat directory made a single keeper a bottleneck that could only retire, not split; a retired seat kept claiming soil its successors overlapped. | Territories are directory prefixes; founding chunks wide flat directories into bounded zones; a one-prefix zone splits by its own subtree; a retired creature derives no territory. Test: ontology check clean after split, retire and ascension. | M7 |
| A disciplinary body reverted and deleted files it had not authored. | No body deletes or reverts what it did not write; delete moves to an attic with a ledger line. Test: a foreign untracked file survives a cleanup turn. | M7 |
| Background reports from mid-chain bodies were stranded because the wake channel fed only the session. | A report lands in its own parent's turn; depth is tested from the throne down to a slime. | M7 |
| A nested dispatch's end never reached its card, leaving zombie "thinking" cards with a clock that kept running. | Every dispatch start emits exactly one terminal event, at any nesting depth; a done card's elapsed is frozen. Test: three-deep nesting closes every card. | M6 |
| A rank with no reportees could still be handed the dispatch tool by its tools list. | Dispatch is granted by the race's compiled powers, not by config. | M1 |
| Ascension counts must survive restarts and must never count a body's own `@S FAIL` as a pass. | The ascension tally is an append-only ledger replayed at start. Test: FAIL reports never raise the pass rate. | M11 |
| Mid-turn steering and the human's queued lines shared one inbox, so a queued line rode the running turn and broke its task. | Two lanes in the core: `send` reaches a running body at its next tool step, `queue` only at the turn's end. Test: a queued line never appears in the running turn's context. | M5 |
| `@$` orders lived in one process's memory: a restart, a second session or the terminal (which never drained them) lost them, and nobody could see what was waiting. | The shared work queue on disk holds every order (relay-hold) until the wake claims it; the status line shows what is held. Test: a restarted session dispatches the orders the last one left. | M7 |
| Two sessions could claim the same order. | Every move is a revisioned snapshot line; the fold seats only the line that follows the item's last revision, the other reads back as a typed refusal. Test: two concurrent claimants, one claim. | M7 |
| A move on shared bookkeeping during a body's turn was billed to that body's writes. | The shared shelf is harness soil: no body writes it by hand, the tree watch never stamps it. Test: a queue move mid-turn leaves the body's written set empty. | M7 |

## Memory & toolbox

| Lesson | Becomes | Milestone |
|---|---|---|
| The stress relief told a session to write to the log before clearing, but nothing told the next session to read it back; after a clear everything was forgotten. | Session start always injects the law and the recent log through the harness itself, not through a file the host may or may not load. Test: a fresh session after a clear carries the last entries in its first prompt. | M8 |
| The long-memory index went stale on every write and was rebuilt inline on the model call's critical path (the visible "remembering" stall), then ran a whole day stale when that was switched off. | Recall never rebuilds inline; boot heals a stale index once before any body runs and says so. Test: boot after a rotted index rebuilds and notes it. | M8 |
| Recall on greetings and menu answers (`yo`, `1`, `B`) produced noise and two "nothing to rank" lines per turn. | Recall is silent when the query has no content words. | M8 |
| Dispatched bodies re-read the chronicle instead of their own notes because the log won ties on term mass. | Ranking boosts the body's own and its race's notes and demotes the episodic log. Test: ranking fixture. | M8 |
| A skill's cost was shown as its file size, but only its description sits in context until worn, and a worn skill cannot be unloaded except by the body's context dying with its task. | The toolbox meters cost in tokens of exactly what is injected; heavy skills are worn by dispatched bodies, not the throne; the full, stable tool list sits in the cached tools tier and never changes between turns; only the manifest text that fits the turn varies, in the uncached tail. | M8 |
| A hint told models to run a JS tool the released binary did not ship, because the port copied its words byte for byte. | Every hint is generated from the installed capability registry. Test: every command named in a prompt or hint exists in the build. | M8 |
| Living documents collected dates and "added on X's order" notes that cost tokens every session and taught nothing. | Provenance lives only in `log.md`; a lint refuses dates and change notes in law, canon and creature docs. | M7 |

## Config

| Lesson | Becomes | Milestone |
|---|---|---|
| `config explain` named the wrong origin three ways: a list appended onto the default kept "default", a merged map kept the lower layer's file, a section file was labelled by the layer's main file. | Every leaf value carries its origin (layer and exact file). Test: explain is correct for scalars, lists, maps and section files. || M4a |
| A section file holds the bare value while a full config nests it, one section in two files had no defined winner, and an empty scaffold list could clobber a lower layer. | Both shapes validated loudly; one section in two files of a layer is a load error; an empty list or map never overrides a lower layer. || M4a |
| Appending block keys after a flow-style `{}` document wrote an unreadable file, so every theme save in such a world failed silently. | Config is TOML and writes go through `toml_edit` (comments kept), are re-parsed before they replace the file, and a failed save stays visible to the human. Test: save into a comments-over-`{}` file. || M4a |
| The scaffold resolved env, home and cwd differently from the loader and minted into the void. | One path-resolution function shared by every config reader and writer. || M4a |
| A test with an empty environment wrote sessions into a literal `~` directory in the source tree, which then blocked two releases. | Home is resolved through the platform API, never the string `~`; CI fails when tests leave files in the tree. | M0 |
| The strict loader rejected new keys written into live config before the binary that knew them was installed. | Unknown keys are reported as holes naming the minimum version; `config check` reports drift without failing the session. || M4a |
| A custom provider's key was only probed via `apiKeyEnv`, unlike built-in names. | One key-resolution rule for every provider; `status` shows where each key came from (never the value); a literal token in tracked config is refused. | M3 |
| Package managers each read registry logins their own way (npm expands `${VAR}`, old pip needs the login in the index URL, go reads netrc over https and matches host:port). | The registry section generates per-tool auth in each tool's own form. Test per manager. | M13 |

## TUI / CLI

| Lesson | Becomes | Milestone |
|---|---|---|
| `run "<ask>"` read stdin whenever it was not a TTY, so an inherited open pipe (CI, cron, another agent) hung it forever. | Arguments win; stdin is read only for `run` with no ask or `run -`. Test: an open pipe does not block. | M6 |
| A TUI that failed to start wrote its error into a redirected buffer nobody drained and exited silently. | Redirect-then-fail restores the writers. Test: a start failure reaches the terminal. | M6 |
| A piped `/cmd` then `/quit` could quit first because the command's turn rode a queue that raced the next input line. | Input is processed strictly in order. Test: piped command scripts. | M6 |
| `--help` started a session, and the CLI's own hint said "resume with …" while bare `resume` errored. | `--help`/`--version` answer before any world lookup; every command the CLI suggests is executable as printed. Test: hints are run. | M6 |
| Mixing a full-height live frame with scrollback prints desynced the renderer; resizes left ghosts; a shrinking pane stopped following; the first render assumed 80 columns and an SSH client reported 0×0. | One rendering mode per surface (a full-screen frame never prints into scrollback); the transcript is reprinted per width on resize; follow state is computed before resize; no width before the first size event; 0×0 keeps the last size. Test in a real terminal emulator (one that rewraps). | M6 |
| Two print commands batched in one update raced and reordered the scrollback. | One FIFO writer owns the scrollback. | M6 |
| A capital `C` vanished from typed words because it was bound to an action. | No printable character is a binding while the input has focus. | M6 |
| The UI thread read plan files from disk five times a second and re-rendered the whole transcript per token. | No I/O on the render path; each block renders once per width and theme and is cached. | M6 |
| A built-in `/review` shadowed the world's own `/review` command. | A world's own commands win over built-ins, as its config wins over defaults. Test. | M6 |
| A host-side buffer held the queued line, and a turn that died owned it forever; a line queued just as a turn finished was lost. | The queue lives in the core only; the turn-end drain reads it on every exit path, interrupts included. Test: queue a line at every point of a turn's end and after Esc; each runs exactly once. | M6 |
| A fresh session re-asked, re-discovered and repeated what the last one had learned. | `/handoff` writes a fixed template from gathered facts; a new session is told one waits, and reading it verifies every claim before acting. Offered at the stress zone. | M8 |

## Board (web)

| Lesson | Becomes | Milestone |
|---|---|---|
| Markup tests passed while lanes sorted wrong, a pointer printed as text and labels were missing; only a rendered page showed them. | A page is proven by render: screenshot tests at phone and desktop widths, light and dark, with a pinned browser (an old system Chromium rendered the board unstyled). | M12 |
| A line chart over one day of data drew nothing, and a linear axis over an 80,000× spread hid every series but one. | Chart tests include a single-point fixture and a wide-range fixture; a log axis is labelled when used. | M12 |
| Hard-coded dark-only hex made borders, pills and edges invisible on the light theme, and the rare-rank colours failed colour-blind distance. | Components use only theme tokens re-stepped per mode; a lint refuses raw hex outside the palette; contrast and CVD checks run in CI; every coloured node also carries a text label. | M12 |
| A board read without a loopback `Host` check was a DNS-rebinding hole, and file routes risked serving a path off the URL. | Every route, reads included, checks Host and origin; files are served only through a fixed name map. Test: foreign Host and traversal attempts are refused. | M12 |
| Graph code built its lookup before all node types existed and read the `map` index as a coordinate; both were valid code. | The graph model is built and tested in the data layer, separate from drawing. | M12 |
| The event tap created directories on publish and raced board teardown. | Publishers only write to already-open sinks. | M12 |

## Claude Code integration

| Lesson | Becomes | Milestone |
|---|---|---|
| Under Claude Code each hook is a fresh process, so the Stop gate read the model's own edits as pre-turn state, and a unit test passed for the wrong reason. | Turn-open state (verify set, tree snapshot) is captured at `UserPromptSubmit` and persisted. Test: an end-to-end hook sequence driven by the real binary. | M13 |
| `CLAUDE.md` (Claude Code) and `AGENTS.md` (OpenCode) are the only files auto-loaded after a clear, and neither was wired. | Init writes `CLAUDE.md` and symlinks `AGENTS.md` to it, one file for both hosts; a `SessionStart` hook injects the law and recent log. | M13 |
| Claude Code is the loop and the harness is the law: hooks must be deterministic and need no API key. | `tempest hook <event>` for SessionStart, PreToolUse (guard + classifier), Stop (end gate) and PreCompact (drain), all offline. | M13 |
| Installing the guard into Claude Code must not clobber the user's other hooks. | Hook install merges into `settings.json`, keeps foreign hooks, and is idempotent. Test against a scratch HOME. | M13 |
| Claude Code's file tools and its bash tool are gated by different mechanisms. | The guard and classifier cover every write surface the host exposes, not only bash. | M13 |

## Process (how to build)

| Lesson | Becomes | Milestone |
|---|---|---|
| A commit landed with a red suite because the script ran the tests and then committed regardless. | Commit is chained on success (`test && commit`); main is protected by required CI — pending the decision on the repository plan (#34): branch protection needs a public repository or GitHub Pro. | M0 |
| Two "flakes" were product bugs, and one test was never green since it was written. | A flake is a bug until root-caused; no quarantine without an issue naming the cause. | M0 |
| Released binaries run in a plain world found two defects the suite missed, because fixture homes were empty. | Every release candidate is smoke-tested as the built artifact in a fresh HOME with realistic dotfiles. | M14 |
| A mechanical rename turned vendored names (`--bs-body-*`, HTTP `res.Body`) into broken ones with the tests renamed alongside, and a forked second engine needed every fix ported by hand. | One engine; a distribution is a lexicon file plus defaults, never a fork or a rename. A leak scanner (skipping binary files) checks each distribution's output. | M6 |
| A subagent's PASS was wrong more than once (claimed missing retries that existed; a bisect verdict that failed re-verification). | A dispatched body's claim is re-run before it is acted on or logged. | M7 |
| A check whose output went to `/dev/null` failed silently. | Every check prints what failed. | M0 |
| `git add -A` swept stray files into history, and foreign work-in-progress in the tree tripped the world's own gates. | Add by path; tests and the harness never stage or commit on their own; worktrees isolate parallel work. | M0 |
| An old system formatter that could not parse the language's newer syntax ran in the release script, and a stale binary was judged as the new one. | The toolchain is pinned in the repo (M0); `status` prints the build's version and commit, and checks run against a freshly built binary (M6, where `status` is born). | M6 |
| A gap named once and left was usually named again later. | A gap named twice is fixed, not noted; the log records the first naming so the second is recognizable. | M7 |
