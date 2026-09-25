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

# One lock, shared by every worktree of this clone (it lives in the common git directory), around
# the test runs that use every core. Parallel lanes build freely, but only one runs its tests at a
# time: two suites at once each take twice as long, and the load fails the timing-sensitive server
# tests (`ws`, `outbound`) and time budgets for no fault of the code. Builds happen before the lock.
heavy_lock := `git rev-parse --path-format=absolute --git-common-dir` / "hyperion-heavy-tests.lock"

# Run a command under the heavy-test lock, waiting for its turn (a crashed holder releases it).
[positional-arguments]
_locked +cmd:
    #!/usr/bin/env bash
    set -euo pipefail
    exec 9>"{{ heavy_lock }}"
    if ! flock -n 9; then
        echo "waiting for another heavy test run to finish (lock {{ heavy_lock }})..." >&2
        flock 9
        echo "lock taken, running: $*" >&2
    fi
    "$@"

# Run all tests (built first, then run under the heavy-test lock).
test:
    cargo test --workspace --no-run
    just _locked bash -c 'cargo test --workspace && pnpm test'

# Run the slow tests (`#[ignore = "slow: ..."]`) under the slow-test profile, with cargo-nextest
# (`cargo install cargo-nextest --locked`) so that every binary's tests share one pool of cores.
# Nextest runs no doctests, but no doctest is slow. `.config/nextest.toml` holds the `slow` profile.
# Then the fitted tables' check reruns every fast fit and compares bytes (plan 15, P15.T2).
[positional-arguments]
test-slow *args:
    cargo nextest run --workspace --cargo-profile slow-test --profile slow --run-ignored only --no-run
    cargo build -q -p hyperion-fit
    just _locked cargo nextest run --workspace --cargo-profile slow-test --profile slow --run-ignored only "$@"
    cargo run -q -p hyperion-fit -- check --rerun-fast

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
    cargo bench --workspace --no-run {{ args }}
    just _locked cargo bench --workspace {{ args }}

# The sim's GENERATOR_VERSION, read from its source for `--since`.
generator_version := `sed -n 's/^pub const GENERATOR_VERSION: GeneratorVersion = GeneratorVersion::new(\([0-9]*\));$/\1/p' crates/hyperion-sim/src/version.rs`

# Run an offline fit into the sim's tables at the current generator version, updating
# crates/hyperion-fit/tables.lock and tables::MANIFEST (plan 15), e.g. `just fit mge`.
fit task *args:
    cargo run --release -p hyperion-fit -- run {{ task }} --since {{ generator_version }} {{ args }}

# Fail if a fitted table is stale, hand-edited, ahead of the generator version or unregistered.
fit-check:
    cargo run -q -p hyperion-fit -- check

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
ci: fmt-check check lint test fit-check gen-protocol-check

# `ci` plus the slow statistical tests: the full gate.
ci-slow: ci test-slow
