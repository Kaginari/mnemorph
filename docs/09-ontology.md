# The ontology

The ontology is the world's map of itself: which creatures exist, who answers to whom, who owns
each path, what creatures have learned about their territory, and how this world relates to its
neighbours. Its core is decisive — every path has exactly one owner, decided by code. Its outer
layer holds what is plural by nature, facts and relations, and keeps disagreement visible instead
of letting the last writer win. That outer layer takes its rules from Yucong Duan's
[DIKWP-MESH](https://github.com/YucongDuan/DIKWP-MESH-); the
[research page](research/dikwp-mesh.md) has the mathematics.

## In one picture

```mermaid
---
config:
  look: handDrawn
  theme: neutral
  flowchart:
    wrappingWidth: 360
---
flowchart TB
    subgraph core["⚖️ the core — one answer"]
      DOCS["creature docs"] -->|derive| G["graph<br/>races · bonds · minds"]
      G -->|"infer to a<br/>fixed point"| GS["closed graph"]
      GS --> SH{"shapes"}
      GS --> OWN["own(path)<br/>exactly one owner"]
    end
    subgraph plural["🌐 the outer layer — many voices, kept apart"]
      F["facts<br/>by · source · counters"]
      R["relations<br/>between worlds"]
    end
    U["@U territory"] --> F
    F -->|"supporters vs<br/>counters"| CF{"contested?"}
    S["sightings,<br/>named twice"] --> R
    R -->|"glued across<br/>the dimension"| DO{"obstruction?"}
    SH --> FIND["findings<br/>code · severity"]
    CF --> FIND
    DO --> FIND
    FIND -->|"critical, high"| GATE["⛔ the gate"]
    classDef step fill:#e0f2fe,stroke:#0284c7,stroke-width:1.5px,color:#082f49
    class DOCS,G,GS,OWN,F,R,U,S step
    classDef gate fill:#fef3c7,stroke:#d97706,stroke-width:2px,color:#451a03
    class SH,CF,DO,GATE gate
    classDef truth fill:#dcfce7,stroke:#16a34a,stroke-width:2px,color:#052e16
    class FIND truth
```

## In plain words

The core is read from the creature docs every time they change: the races, the chain of
command, the Minds each creature wears, and each creature's territory. From those, the ontology
works out everything that follows — a Slime under an Orc under an Elf is also *below* the Elf —
and checks the shapes the law demands: a Slime answers to exactly one Orc, no two creatures claim
the same ground, the chain of command never loops. Ownership is one function with a fixed order:
ask it about any path and it names one creature, or says nobody, and Rimuru must route a birth.

Around the core sit two things that are not one person's to decide. **Facts**: what creatures
learned about their territory, each remembered with who said it and on what evidence. When one
creature says the opposite of another, the ontology does not pick the newer one; it marks the
fact *contested*, shows both sides, and waits for a check, the Orc or the human. **Relations**:
how this world connects to others — it deploys one, reads secrets from another. A relation is
learned only when it is seen twice, proposed to the human, and dropped again if it stops being
seen.

Everything the ontology finds wrong is a finding with a code and a severity. The gate blocks a
landing only on the serious ones, in the territory it touches. There is no overall health score.

## Technical detail

### The graph

| Class | What | Read from |
|---|---|---|
| a race (`Slime`, `Orc`, `Elf`, `Kijin`, `DarkElf`, the throne and the ascended) | a creature | its doc under `.mnemorph/<race>/<name>/` |
| `Mind` | worn know-how | `.mnemorph/minds/`, and a doc's Minds field |
| `Doc` | a creature's document | the file itself |
| `Fact` (`Territory`) | something learned about a path | `@U territory` lines, as events |
| `World` | this world and its neighbours | `.mnemorph/dimension.toml` |

| Bond | From → to | Properties |
|---|---|---|
| `above` | creature → creature | transitive; `truth` (Slime → Orc), `verdict` (Orc → Elf) and `reports` (→ throne) are sub-bonds; `below` is its inverse |
| `owns` | creature → path prefix | from the doc's Territory field only |
| `wears` / `wornBy` | creature ↔ Mind | inverse pair |
| `heir` · `retired` | creature → creature · flag | relief and evolution |
| `about` · `by` · `counters` | fact → path · creature · fact | see *Facts* |
| `relates` | world → world | typed: `deploys`, `provisions`, `reads-secrets-from`, `depends-on`, or a learned type |

The derived graph is never written back; docs are the truth. Asserted facts and sightings are
append-only event files, folded in $(\text{time}, \text{event id})$ order like
[memory notes](08-memory.md#the-two-layers).

**Inference.** Subclass and sub-bond closure, domain and range typing, inverses and the
transitivity of `above`, applied until nothing new appears: the least fixed point
$\mathcal{G}^{*} = \bigcup_n R^n(\mathcal{G}_0)$. The rules only add triples and the set of
possible triples is finite, so $\mathcal{G}^{*}$ is unique and reached in finitely many steps.
It is rebuilt when the docs' fingerprint moves, before a dispatch, never on a model call.

**Claims.** A territory is read from the doc's structured Territory field only: directory
prefixes, normalised. A path mentioned in prose is never a claim; it raises a low `prose-claim`
finding that the field and the prose disagree. isekai read claims from prose too, and the rules
that kept prose mentions from becoming false claims grew into the largest and most fragile part
of its ontology; a field cannot be misread.

### Ownership: one function

$\mathrm{own} : \text{Path} \to \text{Creature} \cup \{\bot\}$ takes the first rule that matches:

1. **Harness soil** — the shared queue, instruments, memory and ontology event files: no body;
   only the harness writes them.
2. **Scratch** — `.mnemorph/tmp/<body>/`: the body that owns the nest.
3. **A seat** — a living creature's directory under `.mnemorph/<race>/<name>/`: that creature,
   even inside a broader claim; the deepest seat wins.
4. **The compiled soils** of [the races](02-races.md#soil--who-may-write-what): the workspace, a
   post, the proving ground, the plan and the dimension manifest.
5. **The longest territory prefix** among living authoring creatures (the Slimes' zones).
6. **The longest domain prefix** among living gate-holders: an Orc's doc names its domain as
   prefixes, and a path inside it that no Slime's zone covers is the Orc's to route.
7. Otherwise $\bot$: unowned soil, and Rimuru routes its birth.

$\mathrm{own}$ returns one creature per path. A Slime's doc is the Slime's; the gate's line-range
split, which gives the verify block to its Orc, is a refinement the *right author* check applies
to that one file ([the end gate](03-loop.md#the-end-gate)). Retired creatures own nothing: their
seats and prefixes are skipped. When two creatures tie on the same prefix — itself a critical
finding — the tie breaks by creature name, so the answer stays deterministic while the finding
blocks.

The order is the rule: a creature that claims `.mnemorph` itself does not swallow every seat, the
queue and scratch, because rules 1–3 are asked first. The gate's *right author*, the policy, the
dispatch card and `mnemorph onto owner <path>` all call this one function.

**Overlap.** Two creatures whose prefixes are equal, or one inside the other, are a critical
finding — except a seat doc inside a broader claim, which rule 3 settles. Two prefixes of one
creature never collide.

### Facts: who said what

A territory fact is

$$
f = (\text{about},\ \text{text},\ \text{by},\ \text{source},\ \text{status})
$$

where `about` is the path it is about, `by` the creature whose body said it, and `source` the
evidence anchor (a `file:line`, a journal entry, a commit). A fact's id is the hash of its path
and its normalised text, so every assertion of the same fact is one more *support* event on the
same id. `<about>` is always a path — it contains a `/` or starts with `.` — so the first word of
a fact's text is never mistaken for one. On the wire:

    @U territory <about> <fact>                  # asserts a fact
    @U territory <about> !#<fact-id> <reason>    # counters one, naming it

A creature *supports* a fact by asserting it, and *counters* it with the second form. For a
fact, count distinct creatures — $p$ supporters and $q$ counters — and read its status in order:

| Status | Rule |
|---|---|
| **settled** | a settle event with its proof (below) — final until a newer counter |
| **single** | $p = 1$, $q = 0$ — one voice |
| **agreed** | $p \ge 2$ and $\frac{q}{p+q} \le 0.35$ — named twice; one dissenter in three is tolerated |
| **countered** | $q \ge 2$ and $\frac{p}{p+q} \le 0.35$ — the counters carry it the same way |
| **contested** | any other case with $q > 0$ — a `contested-fact` finding |

The rows partition every case. The 0.35 tolerance is DIKWP-MESH's gluing threshold, applied to
creatures instead of tags and measured on the losing side, so more counters move a fact towards
*countered*, never back towards agreement.

**Recency never settles a fact.** A contested fact stays contested until a *settle* event with a
proof: a landing by the path's owner that passed its gate and cites the fact, the Orc's verdict
naming it, or the human. A creature's projection (`@ONTO`,
[communication](04-communication.md#the-wire-envelope)) shows a contested fact with both sides,
the way recall always seats one dissenting note.

`@U law` and `@U colony` are not ontology facts; they land as [memory notes](08-memory.md).

### Relations between worlds

**Gluing across a dimension.** Each world's ontology is a local section; the worlds of a
dimension overlap wherever two of them state relations about the same pair. Every type has an
inverse — `deploys` ↔ `deployed-by`, and `<type>-by` for a learned type unless one is declared —
and a pair may lawfully carry several types at once. For a pair $(A, B)$, let $R_A$ be the set A
states and $R_B$ the set B states, each read from A's side through inverses:

| Case | Result |
|---|---|
| $R_A \cap R_B \ne \emptyset$ | glued on what they share; the rest shown as one-sided |
| only one end states anything | partial: lawful, shown as one-sided |
| both ends state relations and $R_A \cap R_B = \emptyset$ | a `dimension-obstruction` finding, reported to Rimuru |

`mnemorph onto check --dimension` reads the neighbours' manifests (on this machine, inward) and
never edits them; a fix goes to the other world through its own throne, one hop at a time.

**Learning a relation.** A body that sees a cross-world reference — an image another world
deploys, a secret path another world provisions — records a sighting
$(\text{from}, \text{to}, \text{type}, \text{anchor}, \text{body})$. The ledger proposes a
relation to the human, as a manifest diff, only when:

1. **named twice** — two sightings, by different bodies, from different anchors;
2. **not a synonym** — when most pairs a new type $t$ names already carry an existing type
   $t'$, by containment
   $\frac{|\text{pairs}(t) \cap \text{pairs}(t')|}{|\text{pairs}(t)|} \ge 0.8$, the proposal
   offers $t'$ first and the human chooses; a type seen on one pair is judged on that pair alone;
3. **it carries its way back** — every learned relation records the session it was last sighted
   in; unseen for 30 sessions of this world, its removal is proposed the same way.

Only the human's yes writes it, through Rimuru, whose soil the manifest is.

### Findings

Every check returns findings, each with a code and a severity. The end gate's *right author*
check blocks a landing on a critical or high finding that touches it: the finding's path lies
under a path the landing wrote, or the finding is about a creature that owns a written path (so
an `above-cycle` blocks landings by the creatures in the loop). Medium and low are reported, never
blocking. Findings are never summed into a score.

| Code | Severity | Read as |
|---|---|---|
| `overlap` | critical | two creatures' territories equal or nested |
| `above-cycle` | critical | the chain of command loops |
| `slime-truth` | high | a Slime without exactly one `truth` edge |
| `dead-bond` | high | an `above` target with no doc |
| `heir-dead` | high | an heir that is retired or missing |
| `dimension-obstruction` | high | two worlds state relations that cannot both hold |
| `unowned-soil` | medium | a tracked path where `own` returns $\bot$ |
| `orphan-mind` | medium | a Mind nobody wears and not marked shared |
| `contested-fact` | medium | a territory fact with supporters and counters past the threshold |
| `stale-relation` | low | a learned relation unseen for 30 sessions |
| `prose-claim` | low | a doc's prose names a path its Territory field does not claim |

`mnemorph onto check` prints `@S PASS|FAIL <n> findings` with each finding's code, severity and
`file:line`; evolution's overlay must add none.

### Not taken from DIKWP-MESH

Plural ownership, the aggregate leakage score, DIKWP typing of every node, the 25 operators, the
path router and the hard-coded catalogue of new axes. The [research page](research/dikwp-mesh.md) says why.
