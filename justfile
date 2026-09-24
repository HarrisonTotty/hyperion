# HYPERION task runner — `just` lists recipes, `just ci` is the gate before a commit and
# `just ci-slow` adds the slow statistical tests.

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

# The `--` tells electron-vite that the rest of the line is the client's own command line.
# Run the bridge client (Electron) with hot reload, e.g. `just client --port 9000`.
client *args:
    pnpm --filter hyperion exec electron-vite dev -- {{ args }}

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

# Run the slow tests (`#[ignore = "slow: ..."]`) under the slow-test profile, with cargo-nextest
# (`cargo install cargo-nextest --locked`) so that every binary's tests share one pool of cores.
# Nextest runs no doctests, but no doctest is slow. `.config/nextest.toml` holds the `slow` profile.
[positional-arguments]
test-slow *args:
    cargo nextest run --workspace --cargo-profile slow-test --profile slow --run-ignored only "$@"

# Run the sim's and the testkit's tests, goldens and slow tests included, as wasm32-wasip1.
test-wasm:
    #!/usr/bin/env bash
    set -euo pipefail
    # A second architecture with a 32-bit `usize`, run under wasmtime. It needs
    # `rustup target add wasm32-wasip1` and wasmtime on the PATH (or `WASMTIME` set to it), so it
    # is not part of `ci`. Goldens are read at host paths fixed at compile time, so the guest is
    # given the repository, and the target directory if it lies elsewhere, at those same paths.
    dirs="--dir={{ justfile_directory() }}"
    if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
        dirs+=" --dir=$(realpath -m "$CARGO_TARGET_DIR")"
    fi
    export CARGO_TARGET_WASM32_WASIP1_RUNNER="${WASMTIME:-wasmtime} $dirs"
    cargo test --target wasm32-wasip1 -p hyperion-sim -p hyperion-testkit
    cargo test --target wasm32-wasip1 -p hyperion-sim -p hyperion-testkit --profile slow-test \
        -- --ignored

# Run the Criterion benchmarks, e.g. `just bench -- samplers`.
bench *args:
    cargo bench --workspace {{ args }}

# Regenerate the golden files after a deliberate GENERATOR_VERSION bump.
bless:
    HYPERION_BLESS=1 cargo test --workspace

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

# The gate before a commit: everything but the slow tests and the other architectures.
ci: fmt-check check lint test gen-protocol-check

# `ci` plus the slow statistical tests: the full gate.
ci-slow: ci test-slow
