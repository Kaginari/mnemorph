# Overview

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
    classDef human fill:#f8efcf,stroke:#D4AF37,stroke-width:2px,color:#3a2e05
    class V human
    classDef throne fill:#dbeefe,stroke:#3FA9F5,stroke-width:2px,color:#0b2540
    class R throne
    classDef elf fill:#dcf3e6,stroke:#2FA36B,stroke-width:2px,color:#0b2a1a
    class E elf
    classDef orc fill:#f8e8d2,stroke:#C9822B,stroke-width:2px,color:#3a2408
    class O orc
    classDef slime fill:#d6f5f1,stroke:#19C3B0,stroke-width:2px,color:#06302b
    class S slime
    classDef kijin fill:#fbdde1,stroke:#D7263D,stroke-width:2px,color:#3d0710
    class K kijin
    classDef dark fill:#ebe2fb,stroke:#7B4FD6,stroke-width:2px,color:#22114a
    class D dark
    classDef outside fill:#eceff3,stroke:#6b7280,stroke-width:1.5px,color:#111827
    class OUT outside
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
  unless you have signed a standing permission for it (honoured from M11).
- **Dark Elves** are the adversaries. They attack the code that the others wrote: security
  reviews, known vulnerabilities in dependencies, leaked secrets, unsafe patterns. They cannot
  fix anything themselves. A finding only counts if it comes with proof: a failing test or a
  scanner output.

Reports flow back up the same chain, each rank compressing what the rank below said, until
Rimuru answers you in plain language.

Everything that matters is written to the world directory `.mnemorph/` — creature documents, the
journal `log.md`, the ledgers. Sessions forget; the world remembers.

## Technical detail

### The processes

```mermaid
---
config:
  look: handDrawn
  theme: neutral
  flowchart:
    wrappingWidth: 360
---
flowchart LR
    subgraph user["your machine"]
      CLI["mnemorph<br/>Rust · Ratatui<br/>the core in-process"]
      BOARD["board<br/>TypeScript · React<br/>in the browser"]
      CORE["mnemorphd<br/>Rust core, served"]
      SB["sandboxed tools<br/>landlock + seccomp / Seatbelt"]
      W[(".mnemorph/<br/>docs · log.md · ledgers")]
    end
    LLM[("model providers<br/>Anthropic · OpenAI-compatible")]

    BOARD <-->|WebSocket, loopback + token| CORE
    CLI --> SB
    CLI <--> W
    CLI <-->|HTTPS streaming| LLM
    CORE --> SB
    CORE <--> W
    CORE <-->|HTTPS streaming| LLM
    classDef ts fill:#fde7f3,stroke:#db2777,stroke-width:1.5px,color:#500724
    class BOARD ts
    classDef rs fill:#ffedd5,stroke:#ea580c,stroke-width:1.5px,color:#431407
    class CLI,CORE rs
    classDef gate fill:#fef3c7,stroke:#d97706,stroke-width:2px,color:#451a03
    class SB gate
    classDef store fill:#e8eaf6,stroke:#5c6bc0,stroke-width:1.5px,color:#1a237e
    class W store
    classDef outside fill:#eceff3,stroke:#6b7280,stroke-width:1.5px,color:#111827
    class LLM outside
    style user fill:#f8fafc,stroke:#94a3b8
```

- **The core** (Rust crates) owns everything that must be correct: the loop, the races, the gate,
  the sandbox, the ledgers, the provider clients. It has no UI, and every surface links the same
  crates.
- **`mnemorph`** (Rust, Ratatui) is the command you run. With no subcommand it is the terminal
  app; `mnemorph run "<ask>"` asks once without it, for scripts. It hosts the session in its own
  process — no protocol between the screen and the core — and draws the same events the server
  sends ([the terminal UI](05-architecture.md#the-terminal-ui)). It renders; it decides nothing.
- **`mnemorphd`** (Rust) is the core as a server: JSON-RPC 2.0 over stdio for the TypeScript SDK,
  and the world's admin commands (`mnemorphd runs`, `mnemorphd memory status`, `mnemorphd q list`, …).
- **The board** (TypeScript, React) is the web cockpit, served by `mnemorphd` on loopback. It
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
configurable rank table, and the line Mnemorph keeps.

### What stays from isekai

The nine natures, the crest, the wire register, the gate's four checks (Mnemorph adds two: tests
intact and no seal), territories, Minds,
memory tiers, the value ledger, evolution (split, mutate, rebirth) and ascension. What changes is
where they live: in typed Rust instead of prose plus configuration, with the docs in this book
saying exactly what the code does.
