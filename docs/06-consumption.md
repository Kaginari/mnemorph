# Consumption

## In one picture

```mermaid
---
config:
  look: handDrawn
  theme: neutral
  flowchart:
    wrappingWidth: 360
---
flowchart TD
    CAP["💰 your caps<br/>month · day · session"] --> SES["session envelope"]
    SES --> R["👑 Rimuru"]
    R -->|"@BUDGET slice"| E["🌿 Elf"]
    R -->|"@BUDGET slice"| K["👹 Kijin"]
    R -->|"@BUDGET slice"| D["🌑 Dark Elf"]
    E -->|"@BUDGET slice"| O["🪓 Orc"]
    O -->|"@BUDGET slice"| S["💧 Slime"]
    S -. "spend" .-> L[("📒 usage ledger")]
    O -. "spend" .-> L
    E -. "spend" .-> L
    R -. "spend" .-> L
    L --> M["📊 meters<br/>status line · board"]
    classDef human fill:#f8efcf,stroke:#D4AF37,stroke-width:2px,color:#3a2e05
    class CAP human
    classDef store fill:#e8eaf6,stroke:#5c6bc0,stroke-width:1.5px,color:#1a237e
    class SES,L store
    classDef throne fill:#dbeefe,stroke:#3FA9F5,stroke-width:2px,color:#0b2540
    class R throne
    classDef elf fill:#dcf3e6,stroke:#2FA36B,stroke-width:2px,color:#0b2a1a
    class E elf
    classDef kijin fill:#fbdde1,stroke:#D7263D,stroke-width:2px,color:#3d0710
    class K kijin
    classDef dark fill:#ebe2fb,stroke:#7B4FD6,stroke-width:2px,color:#22114a
    class D dark
    classDef orc fill:#f8e8d2,stroke:#C9822B,stroke-width:2px,color:#3a2408
    class O orc
    classDef slime fill:#d6f5f1,stroke:#19C3B0,stroke-width:2px,color:#06302b
    class S slime
    classDef step fill:#e0f2fe,stroke:#0284c7,stroke-width:1.5px,color:#082f49
    class M step
```

## In plain words

Budget flows down like context; spend flows up like memory.

You set caps — per month, per day, per session. A session gets an envelope. Rimuru carves a slice
of it for each order it gives, each Elf carves slices for its Orcs, and so on. No body can spend
more than its slice: before every model call the core checks whether the call could fit, and if
not, the body stops cleanly and asks its parent for more instead of overspending.

Every model call writes one line to the usage ledger: who spent it, on which model, how much was
cached. The terminal status line and the board show it live: tokens, cost, cache hits, burn rate,
what is left.

The biggest saving is designed in, not bolted on: every prompt is laid out so its start never
changes between calls, which lets the provider's prompt cache serve most of it at a fraction of
the price.

## Technical detail

### The usage row

One JSON line per model call in `.mnemorph/instruments/usage.jsonl` — decisions included: a
System One read writes its own row under its provider, with `usd` null for a model the price
tables do not know, so its `wt` still reaches the meters and the budget:

```json
{"ts":"2026-10-23T19:07:00Z","session":"…","run":"…","body":"slime-wire","race":"slime",
 "commission":"c-17","model":"…","provider":"anthropic","input":5580,"output":112,
 "cacheRead":4096,"cacheWrite":0,"prefix":3900,"wt":6700,"usd":0.0041,
 "envelope":{"left":54888,"of":60000}}
```

`prefix` is the part of the input that was the fixed per-session prompt (law, race card,
creature card, manifests): the cost every turn carries before it does any work, folded on its own
line (`prefixShare`) so a growing law shows up as a number, not a feeling. The journal is
append-only: one row per line, appended and synced before the call returns (a new journal's
directory is synced too). A torn last line is closed before the next row is written, and is
skipped and reported when the journal is read back, never fatal; a byte-order mark is ignored.

**The chain.** The usage ledger is a chained journal (`mnemorph_ledger::chain`), so an edit of
past spend is found, not believed. Each row carries the chain's fields around its own, in a
fixed order:

```json
{"prev":"<sha-256 hex>","ts":"…","session":"…", … ,"digest":"<sha-256 hex>"}
```

- `digest` is the SHA-256 of every byte of the line before `,"digest"` — the bytes as written,
  never a re-serialisation — and `prev` is the previous row's `digest`. The row's own fields are
  recovered byte for byte, so a reader parses a chained line as it parses a plain one.
- **Genesis.** A journal becomes chained from its first chained row, marked `"genesis":true`,
  whose `prev` is the SHA-256 of every byte the file held before it: rows written before the
  chain read as before, and an edit of them breaks the genesis link.
- **The head** — the newest row's digest — is kept in memory by the writer and found on open by
  walking the file once; another writer's rows are walked on under the file's lock before the
  next row is sealed, so concurrent writers keep the chain linear. Every writer of a chained
  journal chains: an unchained row after the genesis is a break, and reading the journal back
  skips it.
- **A torn last line** (a crash mid-write; it ends without its newline) is a truncation, not
  tampering: the next row closes it, links to the last whole row, and records it as
  `"torn":<line>`. Only a partial line is ever torn: a line that is whole JSON breaks the chain at
  once, whatever comes after it.
- **The head is anchored outside the file**: by `mnemorphd housekeep --verify`, in `log.md` and
  in a copy under the human's state directory (`$XDG_STATE_HOME/mnemorph/anchors/<world>-<fnv64a>.jsonl`),
  out of every command's reach; in the handoff when solo escalates (and the run's journal beside
  it); and in an ascension, probation or fall entry when one is written. Dropping the last rows,
  or stripping the chain's fields and rooting it again, leaves a chain whose links hold; the walk
  is held to the newest kept head (the state directory's, else `log.md`'s), which must still be a
  row at its line or later, or the chain breaks at the anchor. A walk that does not hold anchors no
  new head.
- **`mnemorphd housekeep --verify`** walks each chained journal once and says, escaped: OK with
  the head, a truncation with the head the chain holds to, or the first broken link with its line
  and why (an edited row, a removed or inserted one, a line no row recorded torn, a lost genesis,
  a kept head the chain no longer holds), and whether `log.md`'s newest anchor and the state
  directory's disagree. The Dark Elf's law audit calls the same walk
  (`mnemorph_world::chains::verify`).
- **The tripwire** puts the usage ledger back after each command from what it held plus what its
  witness (`$XDG_STATE_HOME/mnemorph/usage/<world>-<fnv64a>.jsonl`, every byte the harness
  appended, written there first) gained meanwhile: a command's edit is undone, and a parallel
  run's rows are kept.

Chained today: the usage ledger, the Dark Elf's value ledger (its challenge records) and the
evolution ledger. Gate
verdicts, seals, envelopes and Kijin posts are chained as each is opened with
`Journal::chained` and named in `mnemorph_world::chains::journals`.

Two meters, one implementation each, never confused:

- **occupancy** — how full the window was at the last call: that call's `input + cacheRead +
  cacheWrite`. A reading, not a sum; the stress zone reads it.
- **spend** — every class of every call, summed. Budgets and caps read it, in weighted tokens.

Prices are `[models."<id>"]` tables in TOML — `input`, `output`, `cache-read` and `cache-write`,
all four required, in dollars per million tokens — layered model by model from the binary's table
and config (a config row replaces the whole row). The cache prices are never derived from the
input price: the read multiple differs by model, a tenth on some and a twentieth or less on the
newest. Mnemorph marks only 5-minute cache breakpoints; a reply that reports 1-hour cache writes
(priced at 2× input) carries a warning on its response, since the ledger prices every write at the
5-minute rate. A missing or misspelt key or a negative price refuses the table. A model with no
price records `usd: null` and the meter says *unpriced*, never zero; its weighted tokens still
count, with an output ratio of 5.

The binary ships one table per provider, from the provider's published prices — never guessed.
Anthropic's (`crates/mnemorph-provider/prices/anthropic.toml`) prices Claude Opus 5.5, Claude
Sonnet 5.5 and Claude Fable 5.1; a model whose four prices are not all published, such as Claude
Haiku 5.5's cache prices, stays unpriced until they are.

### The build on value rows

Every row the value ledger folds — a Dark Elf finding found, ruled or decided — carries the build
that wrote it, as `build: { version, commit }`: the crate version and the commit `status` prints,
from one source (`mnemorph_ledger::Build::this`, stamped at build time; a tree with uncommitted
changes to tracked files stamps its commit `-dirty`, and a build without git `unknown`). The
evolution ledger's swap and run rows carry it the same way, and M11's ascension rows will. A row written before rows were stamped reads as `unknown`.
The row is written whole, in one write, stamp included. A fold groups or filters by build, so a
replay can tell a rate earned under old gate rules from one earned under new; the scoring reads
every build alike.

### The unit: weighted tokens

A budget counts what a call really costs, not how long its context is. Re-sent context served
from the cache costs about a tenth of fresh input (less on the newest models), so counting raw
context would kill a body after a few calls while
it was spending almost nothing. The unit is the **weighted token** (wt):

```
wt = uncached_input + 1.25 × cache_write + 0.1 × cache_read + r × output
```

where `r` is the model's output-to-input price ratio from the price table (5 when the model is
unpriced, or priced with free output — output is never free in weighted tokens). The weights are
fixed on purpose: wt is a budget unit, the same for every model, and dollars are the exact view.
It is computed in integers and rounded up.
Currency caps convert through the same table; a model with no known price is still bounded in wt.

### Envelopes

- **Caps:** `budget.session`, `budget.day`, `budget.month`, in weighted tokens, from the human's
  own files only — a cloned repository neither raises nor sets one. They are per world: the
  day and the month are read from the world's usage ledger, so they hold across the sessions
  that share it (a ledger across worlds comes with the board, M12). Before every model call the
  run rereads the ledger and stops at a checkpoint (`budget: the day cap is spent — …`) once the
  session, the day or the month is spent; at 80% of one the human is warned, once. A call is
  never cut in half, so a run can pass a cap by the one call that crosses it; every call — the
  salvage turn after a clock death included — is metered and ledgered. A ledger that cannot be
  read opens no capped session and stops a capped run: spend it cannot see is spend it cannot
  stop. A line it cannot read — a torn last write after a crash, a hand edit — is counted high, as
  the largest call the ledger knows (20 000 wt at least), against every cap, since it cannot say
  its day or its session; against each cap it weighs at most a hidden reserve of 15% of the cap,
  so by itself it never stops a session, and with real spend it brings the stop that much nearer.
  The session opens telling the human which lines to repair or remove to count them true. Weighted tokens count every call — priced, unpriced or free — so no model slips under a
  cap; a dollar cap could not promise that, and the dollar keys (`budget.*_usd`) are a view for
  the board (M12), never a limit.
- **Only cost is carved.** Steps and the wall clock pass down whole, per race
  ([the loop](03-loop.md#budgets-are-readings)); splitting them would leave the Slime — the only
  body that writes code — a handful of steps.
- **Carving follows the plan.** A parent keeps a 10% reserve of its own envelope; splitting the
  rest across its plan's open items by their declared weight comes once plans carry weights. Each race has a
  compiled **maximum** share of what the parent has left when it carves (Elf 60%, Orc 60%,
  Slime 70%, Kijin 30%, Dark Elf 40%) — a ceiling, never a default — and the slices together
  never exceed what the parent holds.
- **Who gives.** Every slice comes out of the giver's own envelope: what a parent gives a child it
  can no longer spend itself, and what the child spends counts as the parent's. Work no body owns
  — an order the wake runs, one the human added with `mnemorphd q` — draws from the session's
  shared pool, the envelope itself, under the same clamps.
- **The grant is the giver's judgment, inside the harness's bounds.** The parent proposes its
  child's slice (`budget_wt`, default half of what it has left, within the ceiling); the harness
  grants at most
  `left × 90% × share(race) × worth` — the parent's reserve kept, the race's maximum share, the
  child's worth — and the child's report tells the parent when it granted less than asked. Whole,
  the slice is `max(min(asked, ceiling), min(20 000, left × 90%))`: never below 20 000 wt, about
  one call, so a floor outranks worth but never the parent's reserve. An ask under it is raised,
  and the report says so; an ask of 0 stays 0, and is refused. With no cap set there is no envelope to
  carve: a commission's `budget_wt` is not read, and the child's slice is unbounded.
- **The top-up.** A parent may also set aside a top-up out of its own envelope (`topup_wt`): its
  further trust, so slice and top-up together stay within what it may give at all (90% of what it
  has left) and within twice the child's ceiling — a parent can stretch its trust, never ignore
  worth. It is given once, when the child's slice runs out, and the child is told; it finishes
  instead of stopping at its budget checkpoint, and the human's caps are read all the same. With
  no cap set there is no slice to run out, and no top-up. What the child spends of it counts as the
  parent's, like the slice.
- **Worth, from the child's card.** Before it decides, a dispatching body reads one card per body
  below it: that body's record and the record of every body below it — weighted tokens spent
  (the usage ledger, rounded to the thousand), gate passes and fails (only the entries the end
  gate wrote in the journal; a `NOT_VERIFIED` counts as a fail, so not verifying never buys
  worth; one the machine would not run, `NOT_VERIFIED, unrun`, counts neither way). Worth is `0.5 + 0.5 × (passes + 1) / (passes + fails + handed
  back / 2 + 2)`: a failure handed back to be fixed counts half; a body with no record starts at
  0.75, a clean record nears 1, a failing one nears 0.5. **A slice that runs out is never a failure
  on the card:** a budget checkpoint ends the run before its body answers, and only an answer reaches
  the end gate, which alone writes the verdicts a card is read from. Counting it would let a thinly
  funded body fail for want of budget, lose worth, get a thinner slice and fail again; in
  [the research's simulation](https://github.com/Kaginari/mnemorph-research/blob/main/experience-value.md)
  that loop cost 2 to 10 landing points. A test holds the rule. A child's record counts in its parent's card, so a
  parent that funds failing children reads worse to the body above it. The learned value model
  ([M11](roadmap.md)) replaces this score; the card and the clamp stay.
- **The wake's draw.** One wake draws at most 25% of what the pool has left, and runs at most 3
  orders (a refused one does not count); with no cap set there is no pool to share, and the order count alone holds.
- **The challenge reserve.** 10% of the session envelope is reserved for the automatic Dark Elf
  challenges; it cannot be carved for anything else. Each challenge draws a quarter of the
  reserve's full size (or what remains), and what it does not spend goes back to the reserve.
  Once it is spent, a routed reviewer waits queued for a later sweep; what each review spent is
  booked against its challenge.
- **Pre-flight**, for a carved slice (dispatch, M7). Before each model call the core estimates
  the call in wt (`estimate`: cached
  prefix at the cache-read weight, the rest uncached, `max_output` at `r`). If it does not fit
  what is left, the loop checkpoints with `@? budget: need ~N wt, have M wt`. The parent can top
  up the child's slice from its own envelope (`@INSTRUCT` with a new budget), split the work, or
  escalate to the human.
- **A budget refusal is explained** (`BudgetShort::refusal`, `Shortfall`): it names what is
  needed, what is left, the least top-up that lets it fit, and where the top-up comes from — the
  human's cap that sets what is left, raised under `budget.session`, `budget.day` or
  `budget.month`, or the commander that carved the envelope:
  `refused: needs 41k wt, 12k left this session; top up >= 29k wt (budget.session)`. From ten thousand up, weighted tokens read in
  thousands — the need and the top-up rounded up, what is left rounded down — so the top-up named
  always suffices.
- **Spend is recorded, never refused after the fact.** Pre-flight is the guard; when a call
  costs more than its estimate, the real cost is booked and the envelope reads *overspent*.
- Unspent slices return to the parent when the child reports; what the child spent counts as the
  parent's spend. A slice settles once, and only with the envelope that carved it — a replayed or
  stray report is refused and leaves the books alone. Envelopes and open slices serialise whole, so
  a checkpointed run resumes with the same books.
- Once dispatch carves (M7), a spent cap also stops new commissions while the turn in flight
  finishes its report.

### Cache-first prompt layout

The provider renders tools, then the system prompt, then the messages. Changing any tool
definition invalidates everything after it, so the tools tier comes first and never varies.
Anthropic allows at most four cache breakpoints. Most of a long run's input is its
conversation, not its prompt, so one breakpoint always rides the conversation's tail:

```
[ tools: the race's full, stable tool list ]  ← identical for every body of a race:  breakpoint 1
[ crest + nature law + race card           ]  ← identical for every body of a race:  breakpoint 2
[ creature card + its rules + Minds        ]  ← identical across its turns:          breakpoint 3
[ recall anchors · toolbox manifest text · commission · conversation ]
                                              ← the last block sent:                breakpoint 4
```

- The conversation's breakpoint moves forward every turn, so each request reads everything the
  last one sent at the cache-read price and writes only what the turn added. The conversation
  takes the slots the stable tiers leave, the tail first; a spare slot marks the previous
  request's tail too, so a turn that appends many blocks still lands within the provider's
  lookback of the entry the last one wrote.
- Compaction rewrites the conversation from its first folded message on, so it folds only in
  rare, large batches ([The loop](03-loop.md)): between folds, the history is read, not resent.
- Every request only appends to the one before, between folds. The system prompt and the tools
  are built once, when the run is made, and sent byte-identical for the whole session. Anything
  that changes mid-session rides as a new message, never a rebuilt prefix:
  - the model falling back, the date, the working directory, the mode or a reloaded
    configuration is a `[context update]` line appended before the next request
    (`Run::update`, `Run::switch_model`; a new model changes only the request's model field);
  - a Mind worn is the `wear` call's result;
  - the toolbox manifest, the journal's opening, recall and the ontology ride in the ask's own
    message;
  - a steering line, a gate failure handed back and a stress warning are notes appended after
    the turn's results;
  - the summary, drain and salvage turns send the tools as any turn does, and tell the body in
    words to call none.
- The run checks the prefix each turn: the system prompt and the tools, then the conversation
  block by block, its moving breakpoints aside. A fold or a compaction may rewrite what was sent,
  and a `prefix` beat in the journal records where the prefix moved and why. A rewrite with
  nothing to explain it is a bug:
  - it is said in a warning and a `prefix` beat (`unexplained`), in every build, and the run goes
    on;
  - once a session, the run strips every thinking block from what it sends, the turn in flight's
    included. Under preserved thinking an edit invalidates every thinking block after it, and a
    request that still carries one is refused. The provider rebuilds its cache from there
    anyway, so it is rebuilt smaller.
  - tool calls and words stay; a turn that held only thinking says "(reasoning omitted)", so no
    turn vanishes. The journal keeps everything.
- Behind an OpenAI-compatible gateway, Claude caches only what is marked: the request carries
  the gateway's one automatic breakpoint, which moves with the conversation. The other models
  there (OpenAI, DeepSeek, Gemini, Grok, …) cache a stable prefix on their own.

- The toolbox's level-1 manifest is **text** in the changing tail, never tool definitions — so
  offering different tools per turn never breaks the cache.
- A test runs a session of eight turns through a steering line, a Mind worn, a model fallback,
  a reloaded configuration and a fold. It asserts the system prompt and the tool JSON are
  byte-identical in every request, and that each request only appends to the one before except
  at the fold. Any timestamp, map ordering or counter that leaks into the stable prefix fails CI.
- Each provider and model has a minimum cacheable prefix; a test asserts each breakpoint's
  prefix meets the minimum of every configured model, or the breakpoint is merged into the next.
- Caches are per model: "identical for every body" pays off among bodies on the same model.
- `cacheHitRate` is a signal in the value ledger, per tier; the board shows it per race.

### Meters

| Meter | Reads | Where |
|---|---|---|
| spend | session tokens and cost, per body | status line, board |
| left | envelope left for the active body and the session | status line |
| cache | cache read ÷ input, rolling | status line, board |
| burn | tokens per minute over the last 5 minutes | board |
| per race | spend and pass rate by race over 28 days | board, `mnemorph ledger` (M12) |

From M12, `mnemorph ledger [--by race|body|model|day]` prints the same folds in the terminal.

### Building Mnemorph within your plan

The roadmap is cut into rungs small enough to finish in one or two focused sessions each. On a
subscription plan what binds is not money but the usage windows (the 5-hour and weekly limits),
so after every rung the session's **tokens per model** and the **share of the usage window** it
took are read with `npx ccusage` (`daily` and `blocks`, from Claude Code's local logs) and
written to the rung's entry in `log.md`. The dollar figure ccusage prints is the API-equivalent
price, not what the plan costs: it is recorded only as a comparison. From M6 Mnemorph runs part of
its own work in solo mode, and its own ledger becomes the second instrument.
