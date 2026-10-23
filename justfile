# One entry point for every task. `just` alone runs the full check.

set shell := ["bash", "-euo", "pipefail", "-c"]

default: check

# Everything CI runs: Rust, TypeScript, the layering, the book, the workflows — and the tree
# must read the same after the check as before it (tests leave nothing behind).
check:
    #!/usr/bin/env bash
    set -euo pipefail
    before=$(git status --porcelain --untracked-files=all)
    just rust ts layers docs workflows
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
    cargo nextest run --workspace --no-tests=pass
    cargo deny check

# TypeScript: Biome, types, tests.
ts:
    pnpm lint
    pnpm typecheck
    pnpm test

# Every crate depends only on lower layers (docs/05-architecture.md).
layers:
    node tools/check-layers.mjs

# The GitHub workflows parse and hold together.
workflows:
    actionlint

# Build the book as a website (fails on a dead link or a diagram Mermaid cannot parse).
docs:
    node tools/check-diagrams.mjs
    pnpm --filter @tempest/site build

# Serve the book locally while editing it.
docs-dev:
    pnpm --filter @tempest/site dev

# Format everything in place.
fmt:
    cargo fmt --all
    pnpm exec biome check --write .

# Set up a fresh clone.
setup:
    pnpm install --frozen-lockfile
