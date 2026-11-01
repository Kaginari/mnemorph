# The loop

## In one picture

<a href="./canvas/loop-light.svg">
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="./canvas/loop-dark.svg">
  <img src="./canvas/loop-light.svg" alt="The loop: perceive, recall, plan, act, verify, record; back to perceive while tool calls remain, else the end gate: a pass reports up, a failure retries or goes up as FAIL">
</picture>
</a>

<details>
<summary>The sketch in Mermaid, the text this canvas is drawn from</summary>

```mermaid
---
config:
  look: handDrawn
  theme: neutral
  flowchart:
    wrappingWidth: 360
---
flowchart LR
    P["👁️ perceive<br/>budget · clock · context"] --> RC["📚 recall<br/>memories · tool manifest"]
    RC --> PL["🧠 plan<br/>one model call"]
    PL --> A["✋ act<br/>one tool call"]
    A --> VF["✅ verify<br/>did it work?"]
    VF --> REC["📝 record<br/>journal · ledger"]
    REC -->|more tool calls| P
    REC -->|model answered| G{"🚪 end gate<br/>(if files written)"}
    G -->|pass| DONE(["report up"])
    G -->|fail, retries left| P
    G -->|fail, none left| FAIL(["FAIL up"])
    classDef step fill:#e0f2fe,stroke:#0284c7,stroke-width:1.5px,color:#082f49
    class P,RC,PL,A,VF,REC step
    classDef gate fill:#fef3c7,stroke:#d97706,stroke-width:2px,color:#451a03
    class G gate
    classDef good fill:#dcfce7,stroke:#16a34a,stroke-width:2px,color:#052e16
    class DONE good
    classDef bad fill:#fee2e2,stroke:#dc2626,stroke-width:1.5px,color:#450a0a
    class FAIL bad
```

</details>

## In plain words

Every body — Rimuru, an Elf, a Slime — runs the same six beats, over and over, until it has an
answer:

1. **Perceive** — check the fuel gauge: how much budget, time and context is left. If it is
   running out, save what matters and stop cleanly instead of crashing.
2. **Recall** — pull in the few memories and tools that fit the question, as pointers, not
   pasted text, always including one that disagrees if any does ([memory](08-memory.md)).
3. **Plan** — ask the model what to do next.
4. **Act** — run one tool the model asked for, after checking that this race may use it and,
   for anything that leaves the machine or destroys something, after asking you.
5. **Verify** — did the tool work? A failure counts against the body.
6. **Record** — write what happened to the journal and the ledger.

When the model stops calling tools and answers, the **gate** judges any files the body wrote.
If it fails, the body gets one chance to fix it. Then the report goes up.

## Technical detail

### One act, in order

```mermaid
---
config:
  look: handDrawn
  theme: neutral
  flowchart:
    wrappingWidth: 360
---
sequenceDiagram
    autonumber
    participant M as model
    participant L as loop
    participant C as classifier
    participant R as race law
    participant T as territory
    participant H as human gate
    participant X as sandboxed tool
    rect rgba(2,132,199,0.10)
    M->>L: tool_use(bash, "git push origin feat/x")
    L->>C: classify
    C-->>L: outward
    L->>R: is outward on this race's shelf?
    end
    alt not a Kijin
      rect rgba(220,38,38,0.10)
      R-->>L: refused — "route to kijin-forge"
      L-->>M: tool_result(error, the route)
      end
    else Kijin
      L->>T: inside its post?
      rect rgba(212,175,55,0.22)
      L->>H: plan of the act (or a covering Oni envelope)
      H-->>L: yes
      end
      rect rgba(22,163,74,0.13)
      L->>X: run, secrets injected, outputs redacted
      X-->>L: result
      L-->>M: tool_result
      end
    end
```

The order is fixed and cheap-first: classify → shelf → territory → permission rules → human
gate → run. A refusal never reaches the sandbox, and every refusal names the lawful route.

### Budgets are readings

Before every model call the loop reads four meters from the core, never from the model:

| Meter | Default | Past it |
|---|---|---|
| steps | per race, passed down whole, never split (Rimuru 40, Elf 30, Orc 30, Slime 40, Named Slime 60, Kijin 20, Dark Elf 40), or the human's `steps.<race>` — `steps.slime`, `steps.named-slime`, … — carried by the commissioner to every body of that race; the session's own body working solo — no Elf to dispatch to — gets the shortest chain it stands for, Elf, Orc and Slime, 100, or `steps.solo` | checkpoint |
| wall clock | what the parent has left, passed down whole (session default 60 min) | checkpoint, one salvage resume of ≤ 4096 tokens to land `@U`/`@F` |
| context | an absolute line, `stress.context_tokens` (150k), never past 90% of the window — the same whatever the model | at 60%: old tool outputs fold; at 75%: the body is told to write down what is not durable; at the line: a structured compaction, twice an ask, and the run goes on; after that a drain turn, then checkpoint, telling the human to break |
| cost | the commission's `@BUDGET`, in cache-weighted cost — the only budget that is carved | checkpoint with `@? budget` to the parent ([Consumption](06-consumption.md)) |

A checkpoint is an honest stop: the journal holds every beat, and `mnemorph resume <run>` picks
up from it. An outward act cut mid-flight is never replayed: before its post's next act is
gated, its probe says whether it landed (settled, not asked again), never did (gated again as a
new act) or cannot be told (`reconcile`, for the human) — [races](02-races.md).

**Resume.** A session's run stops at a checkpoint, an interrupt, or a turn the process died in;
`mnemorphd runs` lists such runs, newest first — the id, when its journal was last written, why it
stopped, the last ask — and the session says the command when it stops at a checkpoint.
`mnemorph resume <run>` (or `last`, the newest; `mnemorph --resume <run>` is the same;
`mnemorphd --resume <run>` for an SDK client) opens a session on it:

- **One door.** A resumed session opens exactly as a fresh one does — the configuration, the
  provider and its key, the sandbox, the end gate, the observer, the commissioner when the world
  has lanes. Only the run differs: rebuilt from its journal instead of new, so its next answer
  lands through the end gate like any other, and a failing verify line sends it back.
- **Nothing to resume is refused,** before any key is asked for: a run whose last ask answered,
  failed or escalated; a journal that is missing, or holds a line that does not read — the
  conversation could not be rebuilt as it was; a commissioned body's run, which is its
  dispatcher's.
- **One session per run.** A live session holds its run — a lock beside its journal that the
  operating system lets go of when the process ends, however it ends — and a resume of a held run
  is refused; a run whose lock cannot be made opens no session at all. `mnemorphd runs` only probes
  the lock, and a resume waits that instant out. When a session ends with nothing left to resume,
  its lock file is removed while still held. What `runs` prints from a journal — the last ask, why
  it stopped — has every character `mnemorph_loop::hidden` names (controls, format and bidi
  characters, separators, default-ignorables) escaped.
- **The model it rode.** The journal says which model the run rides, and a resumed run keeps it.
  When the configuration now names another, the run switches and the body is told as a context
  update appended after what was already sent: the prefix stays append-only.
- **Its meters.** The steps already taken and the spend already ledgered are read back; the
  journal's last entries, said with a fresh session's first ask, are not said again.

**The journal** is one append-only JSON line per beat — the ask, each model turn with its blocks
kept whole (thinking included, so a resume sends them back unchanged), each tool result, each
note the loop added, the ending. Resume rebuilds the conversation from it and counts the steps
already taken. A tool call the journal shows no result for may not have run: it is answered
with an error saying so, and the body asks again if it still needs it. Each answered model call
also writes a priced row to the usage ledger, and the cost meter reads that sum.

**Replay compaction** is a pure function over a copy of the conversation: old tool outputs fold
to a one-line caption — "folded: 100 lines of earlier output, call the tool again if you need
it" — while every assistant turn (its text, its tool calls, the thinking bound to them), every
small result and the last four exchanges stay whole. A fold rewrites the cached conversation
from its first folded message on, so it comes in rare, large batches: past 60% of the stress
line, and only when it takes out a fifth of the line or more. A jump past the line from under
60% — one huge output — or a resume above it folds first, whatever the batch: the checkpoint comes
only when folding cannot bring the next request under the line. Between folds the conversation
sent only grows, and each turn reads it from the provider's cache
([Consumption](06-consumption.md)). The journal is never touched: compaction shapes only what is sent. A 40-step run whose
history would be 150 KB sends at most about 55 KB a turn, and with the cache it is billed a third of its input.

**A full context compacts**: at the stress line, once folding cannot bring the next request
under it, the body writes the summary that will replace the conversation — one turn, told to
call no tool and sent with the same tools and cache marks as any turn, so the cached prefix is
read rather than rebuilt; capped at 4096 tokens, in fixed sections: *Goal*, *Instructions*, *Discoveries* (one
`@U <law|colony|territory> <path> <fact>` line each), *Accomplished*, *Files* (one `@F <path> …`
line each), *Todo*. The conversation becomes the ask — with the Minds' manifest, when this ask did not carry it —
and that summary, then the newest exchanges
verbatim — from an assistant turn, so every result kept has its call, up to 40,000 bytes — and
the run goes on. The discoveries are kept as the body's proposed notes, as a drain's are. The
journal keeps everything and records the compaction as a beat — with its head and the context it
was estimated to carry — so a resume rebuilds the same conversation and reads that estimate, not
the summary turn's reading of the full one. An ask compacts at most twice (`Limits::compactions`); a summary that does not
come back drains instead.

**A full context drains** once its compactions are spent: the body gets one more turn —
told to call no tool but sent with the same tools as any turn, so the cached prefix is read, capped at 4096 tokens, within what is left of the clock up to two minutes — to write down what is not durable, `@U <law|colony|territory> <fact>`
for each thing it learned that is written nowhere, then what is left. Each `@U law` or `@U colony`
line is kept as the body's proposed note (tagged `drain`, beside its home); each `@U territory`
fact is filed to the ontology, as a report's is ([Memory](08-memory.md)). The checkpoint carries
the turn as its salvage, and an escalation keeps it before the report it hands up. A run drains
once — a resume reads that from its journal — and a jump that leaves less than two such turns of
room in the window checkpoints without one.

**A clock death salvages**: one more turn, told to call no tool, sent with the tools and capped at 4096 tokens, in a grace of
a tenth of the clock (2–60 s), to write down what the body did, learned and left. The
checkpoint carries it.
A tool the body calls anyway on a drain or salvage turn never runs: only its words are kept.

**Changes mid-session append.** The system prompt and the tools are built once, when the run is
made. A model fallback, a new date, working directory or mode, or a reloaded configuration is
said to the body as a `[context update]` line before its next request, and journaled as a note,
so a resume rebuilds the same conversation. Only a fold or a compaction rewrites what was sent; a
`prefix` beat records each such move ([Consumption](06-consumption.md)).

### The session: queue and interrupt

A session is one body's run and the lines the human types while it works. **Enter** during a turn
queues the line; when the turn ends, the next queued line becomes the next ask. **Esc** interrupts
the turn wherever it is: during a model call the call is dropped and nothing half-streamed enters
the conversation (an answer that has already arrived is kept, and Esc is honoured before any of
its acts runs); during a command, the command's whole process group stops within a poll, and
what it printed so far is kept in its result; between tool calls the calls not yet run are
answered "interrupted by the human before it ran", and none of them runs. The run's clock reaches
a running command the same way — including the end gate's verify commands — so no command
outlives the deadline. Either way the session pivots to the next queued line.

The step budget and the stress instruments are each ask's own — a session of questions is not
thrash; the clock, the context and the cost run for the session, and a resumed run reads what
it already spent and its last context from the ledger. An Esc that landed after an answer is
dropped when the next ask starts.

A client watches a run through its **observer**: every beat at the moment it is journaled, and
every call's spend — a model turn or a decision — with the run's running total and its context
against the stress line. It is how the terminal app and the board show a run live
([the client protocol](04-communication.md#the-client-protocol)) without reading the journal
back. `open(world, gate, observer, env)` builds the session a client talks to — solo, and with the
`commission` tool and an envelope from the human's caps when the world has Elves under a living
throne: the configuration, the model through the human's backend, the tools under the kernel
guard, the journal, the ledger and the decisions.

What the model returns is cleaned before it enters the conversation or the journal: empty text
blocks dropped, a repeated tool call id made unique, an empty reply stated as such. An empty
reply, a reply cut by the output ceiling or a paused turn is not an answer: the body is told and
goes on, bounded by the step meter. The provider's warnings are journaled for the human, never
fed to the model.

Before every request the conversation is **validated**: it starts and ends with the user, roles
alternate, no turn is empty, every tool call is answered by the very next user turn with its
results first, and no result answers a call nobody made. A broken conversation fails the run with
its reason instead of being refused by the provider. A property test throws random answers, tool
calls, interrupts and queued lines at a session and checks every request it sends.

### Stress, in four instruments

| Instrument | Trigger | Response |
|---|---|---|
| **guts** | rolling act score over the last 5 acts falls below −2 (a landed write +2, a failed act or an unchanged re-read −1) | one warning inside the conversation; a new window starts |
| **doom checkpoint** | guts warned 3 windows running | run stops; parent gets *retire / split / reshape* |
| **escalation** | more than 2 consecutive failed acts | turn ends `ESCALATE`, one hop up; a body with no one above — answering the human — is told to change approach instead, and goes on |
| **council** | strain (failed acts + re-asks + 1 per failed run) reaches 5 | the parent must answer as manager: `@INSTRUCT`, `@SPLIT` or `@ESCALATE`; at 7, relief splits or retires the creature |
| **stall** | 3 re-reads of unchanged windows back to back (`repin`); 40 turns with nothing landed (`actBudget`), or the run's steps when it has more, so its steps can be spent; the same call with the same input 5 times in a row (`loop`, warned at 3); one test bench run 12 times with no green and nothing landed between, or 8 times muted (`bench`) | checkpoint: the run is busy, not working |

All are code. None of them asks the body how it feels.

**A re-read of an unchanged window** is answered once with a pointer to the earlier result,
saying how long the file is and the offset that reads on; asked again, the window is shown — the
body is never left behind a pointer to text it cannot find. A fold or a compaction forgets what was
read, since that text is no longer in the conversation. Only the pointer counts as an unchanged
re-read for `repin`, so a body asking the same window again and again meets the identical-call
`loop` stall and the guts first.

The **bench tally** keeps, per test command — the command before any pipe or `;`, its
`VAR=value`, `timeout` and runner words (`uv run`, `npx`, …) set aside — its visits, reds,
greens and muted runs. A **verification script** counts as a test run too: an inline check whose
script asserts (`python3 -c "…assert…"`, a heredoc `python3 - <<'EOF'`, `node -e`, `bash -c`,
`sh -c`), one bench per interpreter since its text changes from run to run, or a script file named
for testing, checking or verifying, with a `.py`, `.js`, `.mjs` or `.sh` name (`python3
check_run.py`, `bash verify.sh`). Bodies often check their work this way, and a tally that knew only
test runners would read them as never testing. An inline script's own `|` and `;` are its own, not
the shell's: only one outside the script mutes the run. A run that hides its exit status (`cargo test | tail`,
`cargo test; echo ok`) is muted, never green. Red runs with fixes landing between them are
test-driven work; twelve with nothing landed stop the run, and so do eight muted runs in a row,
however much landed between them: the body that ran one bench 63 times in twenty minutes, every
run piped, is the reason. A run built with decisions (`decisions()`, `Run::with_decisions`) and `decide.bench` on reads
a piped run's output instead of muting it — only when the shell saw it finish with a zero exit: a
non-zero exit or a kill at the deadline is never read, so no printed summary turns a seen failure
green. Decisions are a feature the human switches on (`decide.enabled`). Jev answers first, the
System One provider `decide.via` names (or jeff, on your own machine); when none is named, its key
variable is not set or it fails, the LLM `decide.llm` answers in its place, its basis stated and
held to the stated cut-off. The decider answers green, red or unknown, and a sure green or red counts in the
tally as if the exit status had been seen. A read moves only the tally: a read red is not a failed
act, so the guts and escalation are untouched. An unsure answer, a failed decision, output the
shell cut short, stdout or stderr (its summary is in the part dropped), an Esc during the command or the decision,
or the run's clock running out leaves the run muted, as it was. Each decision is journaled in
`.mnemorph/instruments/decisions.jsonl`, priced into the usage ledger, and named in a warning in
the beat journal (for the human, never replayed to the model).

**The guts journal.** Every act writes a `guts` beat to the run's journal: the act's score, the
guts window's sum and whether it warned, whether the act failed, and its instruments, the readings
a learned value model ([M11](roadmap.md)) will train on:
- how many files it wrote, and the **lines** it added and removed, counted as a multiset against
  what the files held before, so a line moved within a file is neither (a file that was not text
  had none to remove);
- an **edit miss**: an edit whose `old_string` matched nowhere in the file — not exactly, and not
  in one place on a forgiving rung ([lessons](lessons.md)) — the body editing text it had not read (or, in a multiedit, text an earlier edit of the call had changed);
- the **bench** it ran, if any, and how that run counted: green, red, muted, read green or read red.

The beat is for the record only: the conversation rebuilt from the journal ignores it, and only
its classes reach a surface.

**The guts classes.** Each guts beat also names the act's **classes**, isekai's words for what an
act was, read by harness code from the act's structured outcome and the run's acts before it
(`mnemorph_loop::guts`):

| Class | The act |
|---|---|
| `fail` | errored: an error result, a non-zero exit, a kill at the deadline, a refusal by the envelope, the sandbox or the shelf. A human's denial at the gate is the human's choice, never a fail |
| `red-pin` | ran a trusted bench that exited non-zero: TDD's red, in place of a fail |
| `positive` | ran a trusted bench that exited 0 |
| `repeat` | had the same tool and input as the act just before |
| `echo` | was the third or later `read` of one window, or `bash` of one command head (its first two words), among the last 12 acts, clean, with nothing written since its last visit and not a repeat: a walk through a file's pages is no echo, the debug rhythm of edit and re-run is none either |
| `thrash` | ran one trusted bench for the sixth time or more in the run, every visit from the sixth |

**A trusted bench** is a verify line from a source no body can write, and the act's command,
trimmed with its runs of whitespace made one, must equal it exactly: no prefix, no substring.
A check the body chose is a plain command, whatever it is named: `python3 -c "assert True"`, a
`verify.sh` it wrote, `cargo test --no-run`, `cargo test -p x | tail`. Passing one earns nothing;
failing one is a `fail`, never a red pin. **No such source exists yet.** A creature's verify lines
are on its card, in the world, which a body can reach, so the harness trusts none: no act is a
bench, and `red-pin`, `positive` and `thrash` do not occur until verify lines are kept out of a
body's reach.

No text is read. The classifier is given the tool, its input, whether the act failed or the
human denied it, whether it wrote, and its exit code — never the output. A command that prints
"test result: ok" or "verify passed" earns no `positive`; one that prints a red bench's words
earns no `red-pin`.

The classifier spans the run, every ask of it, as isekai's spans its session: the act before, the
echo's ring and each bench's visits carry from one ask to the next, and a resumed run goes on
from its journal's acts. Replaying a run journal's beats (`guts::replay`) gives the classes the
run gave. Two things differ from isekai, both because a body can forge text: isekai reads the red
pin and `positive` from the output's words, and counts any command that looks like a test runner
as a bench; here only the exit code of a trusted verify line counts. isekai's `neg-lexeme`, a
class read from words alone, is not kept.

The classes are kept twice: on the beat, in the run's journal inside the world; and as one row per
act — the run, the body, the tool call, its classes and the build that classed it — in a journal
the harness alone writes, outside the world: `$XDG_STATE_HOME/mnemorph/guts/<world>-<fnv64a>.jsonl`,
chained by hash and anchored with the world's other chained journals
([chains](05-architecture.md#configuration)). Only that record is shown. `mnemorph status` counts the
last run's acts per class from it, while its chain holds:

```text
guts     last run slime-wire-186f0a2b3c4d5e6f-3 (slime-wire): 14 acts · fail 1 · red-pin 2 · positive 3 · repeat 0 · echo 1 · thrash 0 — signals, never a verdict
```

and says why when it does not (`the classes are not believed: their chain is broken at line 7`).
A process walks the record whole once, then only the rows appended since, reading its last row
again each time; another file, a shorter one, a new anchor or a last row changed in place walks it
whole again. It keeps the newest 100,000 rows.
The board's timeline shows a chip per class on each act of a commission, the newest 200, through
`<Safe>`.

**They are signals, never verdicts.** No bar, rise, fall, warning or stop reads a class: the guts
score, the stalls and escalation are unchanged by them. They are the record a learned value
([M11](roadmap.md), [races](02-races.md#ascension--earned-granted-revocable)) will train on, and
the red pin's weight in any such model is pinned at 0, left out of every update: a red test
written before the change is the right process, and a model that learned to punish it would teach
bodies to skip the test.

The human's configuration reaches a run through `configure`: the attic's limits, the stress
line, the guard rules that ask instead of refusing, and whether commands may run unguarded.

The council's keys are `stress.escalate_at` (5), `stress.relieve_at` (7) and `stress.max_consults`
(3): after three warrants the report is forced to `@S ESCALATE` and reaches the human. Strain is
read from the body's own first `@S`, never from how the loop ended — a court that reports
`@S FAIL` still finishes its loop cleanly. A clean landing clears it; the human's denials and
interrupted or dry runs never add to it. Every relief resets its own trigger, or the next run dies
on its first act. The tally is `.mnemorph/instruments/council.jsonl`, sat after every commission that answered: a
run whose report did not say `DONE` adds its failed acts and one more — a gate denial is the
human's, never a failed act — and a `DONE` clears it. Relief acts only after a warrant, and a
relief that cannot act escalates instead, the tally kept; re-asks are not counted yet. The warrant reaches the parent as a hole in the child's report —
`@? council: <body> strain <n> — answer as its manager: @INSTRUCT …, @SPLIT …, or @ESCALATE` — the
last warrant forces the report's `@S` to `ESCALATE`, the body's own status kept beside it, and relief's act is a note in the report.

### Self-heal

The world repairs itself in small, mechanical steps, each one written to the ledger:

- **Relief** splits a strained creature's zone in two young of the same race — by its prefixes,
  or by the directory tree under a one-prefix zone — or, when the zone cannot split, retires the
  seat: it keeps its memory, claims no files, and stays visible. A `relief` row records the act.
  Relief is skipped for a body that just evolved. It relieves Slimes and Orcs: the two young are
  `<name>-a` and `<name>-b`, of the old one's base race, seated beside it, reporting to the same
  parent, with the same verify lines and Minds and a `Heir of` line naming the old one, each
  taking half the claims sorted — a claim
  another covers set aside — or half the entries under a one-directory ground, descending while
  that directory holds one entry (hidden entries go with the first, a link counts as a file). An
  Orc's living Slimes report to the young whose ground covers theirs. The old seat gains a
  `Retired: yes` line, and on a split a `Heirs` line naming its young, and nothing else changes;
  its open queue orders are named, to re-route.
  Every check runs before any write, and a failed write is undone. The
  human can act it with `mnemorphd relief <name> [--apply]` — without `--apply` it says what it
  would do; never from a body's command.
- **A clock death leaves a memo**: a court killed by its deadline gets its one salvage turn, then
  its lane goes into the instruments as a `death` row (`.mnemorph/instruments/deaths.jsonl`, with
  the body named next) and the next-fit lane is suggested on the wire, as a hole — the living
  sibling of the same base race under the same parent, not stuck on its files, with the best
  worth (a tie to the one that spent least), or, with none, a smaller order or a longer clock —
  advice to the parent, never a change on its own. A salvage turn that wrote nothing is still a
  clock death.
- **The roster heals from the docs**: the world fingerprints its creature docs, and when the
  fingerprint moves — another session's relief, a hand edit — the roster is rebuilt before the
  next dispatch, so no court is created for a retired seat. A heal never moves trust: a body's
  verify lines change only through its own gate.
- **`mnemorphd housekeep --heal`** fixes what is mechanical — a creature without a doc, a broken
  *Reports to* link — and reports everything else. A seat with no doc gets a retired stub, so its
  memory stays visible and it claims nothing; a *Reports to* that names no living creature is
  relinked to the nearest living ancestor of the race that may command it (an Orc for a Slime, an
  Elf for an Orc) — never past the race law; with none above, it is left. Each repair is one journal
  entry; only the field's own line changes, and nothing is written through a link. A doc that
  cannot be read, two seats with one name, a loop in the chain are said and left to judgment.
  Without `--heal` it says what it would fix. It exits 0 when the roster is whole, 1 while anything
  is left or a repair waits for `--heal`, 2 on an error or inside a body's command. With
  `--verify` it walks the chained journals instead and anchors their heads in `log.md`
  ([the chain](06-consumption.md#the-usage-row)).
  It is also the board's routine: with `--heal` it forgets from the board's list the worlds missing
  longer than `board.prune_after_days` (14; 0 never), and without it names them — the list
  alone, never a directory or a journal ([missing worlds](05-architecture.md#the-board-server)).
- **One live court per seat**: a second commission to a busy seat waits in its lane.

### The verify pins

A creature's card lives in the world, and nothing read from inside a world grants trust: a body
can reach a card through routes no tool guard sees — a commit made with git's plumbing and checked
out later, a merge, a pull — and a cloned repository brings every card from a stranger. So the end
gate never runs a verify line because a card says it. It runs the lines **pinned** for the
creature in a witness kept outside the world, `$XDG_STATE_HOME/mnemorph/verify/<world>-<fnv64a>.jsonl`
(else under `~/.local/state`), hash-chained and anchored as `verify` with the world's other chained
journals ([storage](05-architecture.md#storage-files-are-the-truth)), and only while the card
still says the same set of lines.

- **A card that differs from its pin** — a line changed, added or removed — runs nothing. The
  landing fails closed at once, NOT_VERIFIED with no retry, since no retry of the body's can pin a
  line: "the verify line in `<card>` differs from the pinned one: run `mnemorphd verify pin
  <creature>` to accept it".
- **A card with lines and no pin** — a fresh world, a cloned one, a card born since — runs nothing
  the same way, with the same way on. A card with no line and no pin runs no check, as before.
- **A witness whose chain does not walk whole** — a row edited, a tail dropped against its anchor —
  trusts no pin: every landing fails closed until the human looks (`mnemorphd housekeep --verify`
  says where it breaks).

Three writers, none of them a body's command: the human's `mnemorphd verify pin <creature>`, which
shows the card's lines and asks at the terminal (`/dev/tty`; piped input is refused), or pins
without asking under `--yes` for a script the human runs — a benchmark harness that writes the
card itself; the harness, for the line an Orc codes on its own Slime with the `verify` tool; and
relief, which pins each young to the lines its parent's pin held, never to the card's. Every write
is refused inside a body's command (`MNEMORPH_BODY` is set there), and the sandbox grants no
command the state directory.

The first run in a fresh or cloned world asks once. Before the terminal app takes the screen, it
lists every living creature whose card holds lines the pins do not — the card, the lines, whether
it is unpinned or differs — and asks one question; `y` pins exactly the lines shown, anything else
pins nothing and says the landings fail closed until `mnemorphd verify pin`. `mnemorph run` never
asks: its gate fails closed with the hint. A pin is the human's word on one world, named by its
real path: a copy of the world is pinned again.

### The end gate

Runs on the body's own writes (minus what its Courts already landed on their own account) — a
commissioned body's, and the session's own: Rimuru's verify lines are the `Verify:` line of its
card (`.mnemorph/rimuru/rimuru/README.md`), read when the session opens and run only as
[pinned](#the-verify-pins), and in solo the whole repository is its soil, so right author holds
by construction. A card with no verify line runs no
check, and then no claim of a solo session is ever grounded: `mnemorphd memory status` and
`onto check` (`no-session-verify`, low) say so and suggest the line — the command the project's
manifest names (Cargo, Python, Node, Go, a Makefile's `test`), picked by the harness and never a
model's words, since the human may paste it — for the human to add. A Cargo project is suggested
whole, `cargo test --workspace`, with no name read from its manifest. For a compiled
project (Cargo, Go, TypeScript) the suggestion also says that a cold build may outrun the limit
on each verify command, and names the key that raises it. The gate runs:

1. **Right author** — every path lies in the writer's own soil ([soil](02-races.md#soil--who-may-write-what));
   for a Slime's doc the gate diffs by line range: the verify block is the Orc's, the rest is the
   Slime's. The records under `.mnemorph/` are outside the tree's snapshot: the tripwire refuses
   or reverts a tool's write there, beyond the shared queue, scratch and the advisory mirror.
   Writes are found by snapshotting the tree, whatever tool made them — a shell
   redirect counts the same as the write tool. A path's owner is read with `own` on the roster
   as the commission found it, and the failure names the owner and the route (`` `lib/x.rs` is
   slime-b's (zone lib/), not slime-a's — ask slime-b ``). A critical or high ontology finding
   that touches the landing fails this check, read on the tree as the landing leaves it, so a
   landing that ends a finding is not blocked by it ([ontology](09-ontology.md#findings)). Writes are
   attributed per body: under a kernel guard a commissioned body's commands write only what it is
   granted — its soil, its build output, its scratch, its plan — so a change outside that, a
   second session's on the same world, is not its to answer for, a failed landing's put-back
   never touches it, and the commission's report names it as another writer's. The grant is the
   one the body ran with, taken before it starts. With no guard, writers cannot be told apart, and every change in the tree is
   judged the body's.
2. **Traits hold** — the verify lines that stood *before* the turn, as pinned, run green, sandboxed, each
   within `gate.verify_secs` (120 s unless the configuration says; 10 to 600, the most the
   `bash` tool grants a command), and within the ask's own clock. A cold build of a large compiled
   project can take longer than 120 s; killed at the limit, it gives no result, so the landing is
   NOT_VERIFIED until the key is raised. The tracked file may set it: the project knows its build,
   and a longer limit runs the same classified, sandboxed command, only for longer. A line changed this turn is a hole and runs from the next turn (a Named Slime's own
   additions excepted). A check never writes: a verify run that changes the tree beyond what
   git ignores — a test run's build output — fails the landing whole, naming what it changed, with
   no second try (a retry would write nothing and pass what the first wrote as the body's work).
   What a check leaves behind goes to `.mnemorph/report/`, prepared when a session opens: the
   folder, and the common test leftovers (`__pycache__/`, `.pytest_cache/`, `.mypy_cache/`,
   `.ruff_cache/`, and at the root `/.coverage`, `/htmlcov/`, `/coverage/`, `/.nyc_output/`), are
   ignored through git's local exclude file, never committed, so the project's `.gitignore` stays
   the human's. The gate's own view of the tree does not read that file: what the harness ignores
   for checks never hides a body's change.
3. **Duties done** — a commissioned body answered with a non-empty `@S`.
4. **Doc truthful** — the owner's doc moved in the same change.
5. **Tests intact** — no test deleted or skipped. In solo, where the human is at hand and may
   have asked for exactly that, a landing that fails on this alone is put to the human on the
   approval gate — allowed, it lands; with no human attached, it fails as ever — and a failure
   on this alone never reconsolidates memory: it says nothing of what the body knew.
6. **No seal** — the territory is not sealed, or this landing is the fix: it carries the frozen
   witness into the zone's tests, unchanged by hash.

The gate says one of three words, never more. **PASS**: every check ran and held. **FAIL**: a
check ran and did not hold — a crash included. **NOT_VERIFIED**: nothing failed, but a check gave
no result — a verify line that did not run (no kernel guard and none allowed unguarded, or the
command refused or denied) or was killed at its deadline. A partial pass never reads as a full one: NOT_VERIFIED is no pass, and the
journal, the `gateVerdict` event and the transcript carry the word. A line killed at its deadline
gets the same one chance as a failure — the body's tests hung — and so does a line refused or
denied on a guarded machine. With no kernel guard in force and none allowed unguarded, no line can
run: that is the machine's doing, so the landing ends at once, journaled `NOT_VERIFIED, unrun`,
with no second try and no mark on the body's record, and the human is told to make the sandbox
work or allow commands unguarded.

**In solo mode** the gate judges what the run changed against the tree as it was when the ask
arrived — so a shell redirect, a `rm`, a generated file all count — and runs traits hold (the
verify commands, through the same classified and guarded `bash`, `gate.verify_secs` each), duties done (a
non-empty answer) and tests intact: no test file deleted, and no test file — read whole, whatever
its size — with more skip markers than before (`#[ignore`, `.skip(`, `.only(`, `.todo(`,
`t.Skip(`, `t.Skipf(`, `@pytest.mark.skip`, …) or fewer tests (`#[test]`, `func Test`, `def
test_`, `it(`, `test(`, `@Test`). Right author holds by
construction: the whole repository is solo's soil, and the tripwire keeps `.git` and
`.mnemorph` out. An answer that changed nothing passes untouched. A first failure goes back to
the body as a note; a second ends the run as FAIL, every verdict journaled.

A landing on a Dark Elf surface passes as `PASS (challenge pending)`: its challenge opens at the
gate (or, if it cannot be recorded, it reads `PASS (challenge unrecorded)` and waits all the
same), the pass credits nothing, and the throne's harness commissions the routed specialities at the
end of its ask, from the challenge reserve, at most `gate.challenge_reviewers` at once. The
throne's answer says each challenge it moved — pending, verifying, blocked, unstaffed, queued or
cleared — with each finding's verdict, and the landing counts as landed only once its challenge
clears or the human waives it. A reviewer's finding of medium or higher is sent in the same sweep
to a verifier of another speciality, given the record alone
([the challenge](02-races.md#-dark-elf--the-adversary)).

The verdict goes to `log.md` through the append-only writer: one entry per verdict, stamped,
naming the body and the verdict — `PASS`, `FAIL` or `NOT_VERIFIED`, with `, handed back` on a
first one that did not pass and `, unrun` when no kernel guard let any check run — with
the paths it changed and what failed; a pass that changed nothing is not journaled. The writer
only appends, whole entries in one write, each on lines of its own — a control character in a
command's output or a file name becomes a space — and never writes through a link; a world
without `.mnemorph/` has no journal. The same verdict is kept again outside the world, by the
harness alone, in the hash-chained gate journal (`mnemorph_world::verdicts`, anchored as `gate`),
stamped with the build that gave it; cards and the stuck rule read the verdicts from it, held to
its newest anchor, and from `log.md` only in a world that keeps none yet (the cards say so). A
verdict journal that does not verify trusts no body: dispatch waits for the human. Three fails running for the same
body on the same files harden into the lawful next acts: rewrite the commission, split, escalate.
The journal is the record — the body's last three verdicts all `FAIL` (a failure handed back to
be fixed does not count), and the files every one of them changed — and an order that names one of
those files again — as a word of the order, `./` and a `:line` dropped — is refused with the three
routes, and a held order on the queue naming one waits instead of being claimed; an order rewritten without them goes through,
and one `PASS` clears the streak.
