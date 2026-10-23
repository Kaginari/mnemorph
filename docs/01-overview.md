# Overview

## In one picture

```mermaid
---
config:
  look: handDrawn
  theme: neutral
---
flowchart TD
    V(["🐉 Veldora — the human"])
    R["👑 Rimuru — the throne<br/>talks to the human, plans, never writes code"]
    E["🌿 Elf — the lane's planner and voice"]
    O["🪓 Orc — rules a domain, holds the gate"]
    S["💧 Slime — writes the code of its zone"]
    K["👹 Kijin — the only hands that reach outside<br/>CI · release · deploy · forge · scout"]
    D["🌑 Dark Elf — challenges the code<br/>security · CVEs · secrets · attacks"]
    OUT[("🌍 the outside world<br/>GitHub · CI · registries · clusters · advisories")]

    V <-->|human words| R
    R -->|commission| E
    E -->|commission| O
    O -->|commission| S
    O <-.->|sideways contracts| O
    R -->|commission| K
    R -->|commission| D
    K <-->|every act through the human gate| OUT
    D -.->|witnessed findings| R
    S -.->|report| O -.->|verdict| E -.->|lane verdict| R
```

| <img src="../assets/portraits/veldora@64.png" width="64" alt="Veldora"> | <img src="../assets/portraits/rimuru@64.png" width="64" alt="Rimuru"> | <img src="../assets/portraits/elf@64.png" width="64" alt="Elf"> | <img src="../assets/portraits/orc@64.png" width="64" alt="Orc"> | <img src="../assets/portraits/slime@64.png" width="64" alt="Slime"> | <img src="../assets/portraits/kijin@64.png" width="64" alt="Kijin"> | <img src="../assets/portraits/dark-elf@64.png" width="64" alt="Dark Elf"> |
|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| **Veldora** | **Rimuru** | **Elf** | **Orc** | **Slime** | **Kijin** | **Dark Elf** |

## In plain words

You talk to **Rimuru**. Rimuru never touches the code; it understands what you want, asks you
when something is unclear, and hands the work down.

The work flows down a fixed chain. An **Elf** plans a lane of work and decides which domains are
involved. An **Orc** owns a domain: before any code is written it writes the test that will judge
the code, then orders its **Slimes**. A Slime is the only one allowed to edit the code of its own
small zone, and it must update that zone's document in the same change.

Two races stand beside the chain, both answering to Rimuru:

- **Kijin** are the hands that reach outside: pushing branches, opening pull requests, watching
  CI, publishing releases, deploying. Nobody else can. Every outside act still asks you first,
  unless you have signed a standing permission for it.
- **Dark Elves** are the adversaries. They attack the code that the others wrote: security
  reviews, known vulnerabilities in dependencies, leaked secrets, unsafe patterns. They cannot
  fix anything themselves. A finding only counts if it comes with proof: a failing test or a
  scanner output.

Reports flow back up the same chain, each rank compressing what the rank below said, until
Rimuru answers you in plain language.

Everything that matters is written to the world directory `.tempest/` — creature documents, the
journal `log.md`, the ledgers. Sessions forget; the world remembers.

## Technical detail

### The processes

```mermaid
---
config:
  look: handDrawn
  theme: neutral
---
flowchart LR
    subgraph user["your machine"]
      CLI["tempest CLI<br/>TypeScript · Ink"]
      BOARD["board<br/>TypeScript · React<br/>in the browser"]
      CORE["tempestd<br/>Rust core"]
      SB["sandboxed tools<br/>landlock + seccomp / Seatbelt"]
      W[(".tempest/<br/>docs · log.md · ledgers")]
    end
    LLM[("model providers<br/>Anthropic · OpenAI-compatible")]

    CLI <-->|JSON-RPC over stdio| CORE
    BOARD <-->|WebSocket, loopback + token| CORE
    CORE --> SB
    CORE <--> W
    CORE <-->|HTTPS streaming| LLM
```

- **`tempestd`** (Rust) owns everything that must be correct: the loop, the races, the gate, the
  sandbox, the ledgers, the provider clients. It has no UI.
- **`tempest`** (TypeScript, Ink) is the terminal client. It starts `tempestd` as a child process
  and speaks JSON-RPC 2.0 over stdio. It renders; it decides nothing.
- **The board** (TypeScript, React) is the web cockpit, served by `tempestd` on loopback. It
  speaks the same protocol over a WebSocket.
- One protocol, defined once in Rust; the TypeScript types are generated from it. A client can
  never drift from the core.

### What is fixed and what is yours

| Compiled in (code) | Configured (data) |
|---|---|
| the six races and five ascended forms, their tree, shelves, powers | which creatures exist and their territories |
| the gate's checks, the wire envelope | which model rides each race |
| ascension conditions, privileges and falls | budgets: amounts per session, day, month |
| score formulas and meters | provider credentials (by reference, never inline) |
| the nine natures' enforcement points | Minds (skills): know-how a body can load |
| what is `read`, `write`, `outward`, `destructive` | Oni envelopes: standing outward permissions you sign |

Races are behaviour; creatures, Minds and models are data. That is the line isekai blurred with a
configurable rank table, and the line Tempest keeps.

### What stays from isekai

The nine natures, the crest, the wire register, the gate's four checks (Tempest adds two: tests
intact and no seal), territories, Minds,
memory tiers, the value ledger, evolution (split, mutate, rebirth) and ascension. What changes is
where they live: in typed Rust instead of prose plus configuration, with the docs in this book
saying exactly what the code does.
