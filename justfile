# One entry point for every task. `just` alone runs the full check.

set shell := ["bash", "-euo", "pipefail", "-c"]

default: check

# Everything CI runs: Rust, TypeScript, the layering, the book, the workflows — and the tree
# must read the same after the check as before it (tests leave nothing behind).
check:
    #!/usr/bin/env bash
    set -euo pipefail
    before=$(git status --porcelain --untracked-files=all)
    just rust law ts layers docs workflows bench
    after=$(git status --porcelain --untracked-files=all)
    if [ "$before" != "$after" ]; then
        echo "the check changed the tree:"
        diff <(echo "$before") <(echo "$after") || true
        exit 1
    fi

# Rust: format, lints as errors, tests, licences and advisories.
rust:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    # The protocol's TypeScript is regenerated whole, so a type removed from the Rust goes too.
    rm -rf packages/protocol/src/generated
    cargo nextest run --workspace --no-tests=pass
    cargo deny check

# The race law is covered whole: every region of mnemorph-races runs under its tests.
law:
    # Old profile data merges into a new run and can read a covered function as missed.
    cargo llvm-cov clean --workspace
    cargo llvm-cov -p mnemorph-races --fail-under-regions 100 --summary-only

# TypeScript: Biome, types, tests.
ts:
    pnpm lint
    pnpm typecheck
    # The SDK's tests speak to the real core.
    cargo build -q -p mnemorphd
    pnpm test

# Every crate depends only on lower layers (docs/05-architecture.md).
layers:
    node tools/check-layers.mjs

# The bench, with no model: its report's rules, and every task real and answer-free.
bench:
    PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s bench -p 'test_*.py' -q
    PYTHONDONTWRITEBYTECODE=1 python3 bench/run.py --check

# The GitHub workflows parse and hold together.
workflows:
    actionlint

# The Dark Elf's paid evaluation: real models hunt the seeded fixture, and each speciality's
# recall and precision are scored against its key. Never part of `check`. A free model by
# default; the key travels only through OPENROUTER_API_KEY and is never printed.
seeded-eval model="nvidia/nemotron-3.5-lightning:free":
    test -n "${OPENROUTER_API_KEY:-}" || { echo "seeded-eval: set OPENROUTER_API_KEY"; exit 1; }
    MNEMORPH_LIVE=1 MNEMORPH_LIVE_OR_MODEL='{{model}}' cargo nextest run -p mnemorph-loop --test seeded --run-ignored only --no-capture seeded_eval

# The reviewers' calibration: a reviewer — `dark-elf` (each case by its own speciality),
# `dark-elf-<speciality>` or `fable` — judges every case of the gold set, and its recall,
# precision, false accepts and rejects and severity agreement are written to a report under
# `$XDG_STATE_HOME/mnemorph/calibration/`, outside the world. Never part of `check`. `models` is a
# comma-separated panel, free by default; the key travels only through OPENROUTER_API_KEY and is
# never printed. Free models may log or train on what they are sent: each case's claim and its code
# — the seeded fixture's files and gold.toml's reconstructions, never the live tree — and, for a
# Dark Elf, its compiled playbooks. It measures judging claims on the gold set, not detection in a
# live repository.
calibrate reviewer="dark-elf" models="nvidia/nemotron-3.5-lightning:free":
    test -n "${OPENROUTER_API_KEY:-}" || { echo "calibrate: set OPENROUTER_API_KEY"; exit 1; }
    echo "calibrate: sends to {{models}}, which may log or train on it: each gold case's claim and code (fixture files and reconstructions, never the live tree){{ if reviewer == "fable" { "" } else { ", and the compiled playbooks" } }}."
    MNEMORPH_LIVE=1 MNEMORPH_CALIBRATE_REVIEWER='{{reviewer}}' MNEMORPH_CALIBRATE_MODELS='{{models}}' cargo nextest run -p mnemorph-loop --test calibrate --run-ignored only --no-capture calibrate_live

# Build the book as a website (fails on a dead link or a diagram Mermaid cannot parse).
docs:
    # No merge left half-done: a conflict marker anywhere tracked fails the check.
    ! git grep -nE '^(<<<<<<<|>>>>>>>)( |$)' -- .
    node tools/check-diagrams.mjs
    pnpm --filter @mnemorph/site build

# Serve the book locally while editing it.
docs-dev:
    pnpm --filter @mnemorph/site dev

# Format everything in place.
fmt:
    cargo fmt --all
    pnpm exec biome check --write .

# Set up a fresh clone.
setup:
    pnpm install --frozen-lockfile
