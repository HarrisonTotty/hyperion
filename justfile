# HYPERION task runner — `just` lists recipes, `just ci` runs everything CI runs.

set shell := ["bash", "-euo", "pipefail", "-c"]

_default:
    @just --list

# Install frontend dependencies.
install:
    pnpm install

# Install the git pre-commit and pre-push hooks into this clone.
hooks:
    uvx pre-commit install --install-hooks

# Run every pre-commit hook against all files.
pre-commit:
    uvx pre-commit run --all-files

# Run the game server.
server *args:
    cargo run -p hyperion-server -- {{ args }}

# Run the bridge client (Electron) with hot reload.
client:
    pnpm dev

# Typecheck Rust and TypeScript.
check:
    cargo check --workspace --all-targets
    pnpm typecheck

# Lint Rust (clippy) and TypeScript (oxlint, type-aware).
lint:
    cargo clippy --workspace --all-targets -- -D warnings
    pnpm lint

# Format everything in place.
fmt:
    cargo fmt --all
    pnpm format

# Verify formatting without changing files.
fmt-check:
    cargo fmt --all -- --check
    pnpm format:check

# Run all tests.
test:
    cargo test --workspace
    pnpm test

# Regenerate the TypeScript protocol bindings from the Rust protocol crate.
gen-protocol:
    rm -rf packages/protocol/src/generated
    cargo test -p hyperion-protocol export_bindings

# Fail if the checked-in protocol bindings are stale.
gen-protocol-check:
    #!/usr/bin/env bash
    set -euo pipefail
    fresh="$(mktemp -d)"
    trap 'rm -rf "$fresh"' EXIT
    TS_RS_EXPORT_DIR="$fresh" cargo test --quiet -p hyperion-protocol export_bindings
    diff -r "$fresh" packages/protocol/src/generated \
        || { echo "protocol bindings are stale: run 'just gen-protocol'" >&2; exit 1; }

# Build release artifacts.
build:
    cargo build --workspace --release
    pnpm build

# Everything CI runs.
ci: fmt-check check lint test gen-protocol-check
