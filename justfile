# One entry point for every task. `just` alone runs the full check.

set shell := ["bash", "-euo", "pipefail", "-c"]

default: check

# Everything CI runs: Rust, TypeScript, the layering, and a clean tree after the tests.
check: rust ts layers tree-clean

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

# Tests must not leave files behind.
tree-clean:
    @test -z "$(git status --porcelain --untracked-files=all)" || { echo "the tree changed during the check:"; git status --short; exit 1; }

# Format everything in place.
fmt:
    cargo fmt --all
    pnpm exec biome check --write .

# Set up a fresh clone.
setup:
    pnpm install --frozen-lockfile
