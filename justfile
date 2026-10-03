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

# The `--` tells electron-vite that the rest of the line is the client's own command line, which
# electron-vite hands to Electron without the separator, so Chromium sees a switch there as its own:
# `--hyperion-gpu-timing` (lift timestamp quantization, for performance runs only) is given like any
# other client option, `just client --hyperion-gpu-timing`. A Wayland session gets
# `--ozone-platform=x11` here, because the client's own relaunch through XWayland would end
# `electron-vite dev` and leave the new window on a dead dev server (R01 Design note 3).
# Run the bridge client (Electron) with hot reload, e.g. `just client --port 9000`.
client *args: gen-surface
    #!/usr/bin/env bash
    set -euo pipefail
    x11=()
    if [[ "${XDG_SESSION_TYPE:-}" == wayland ]]; then
        x11=(--ozone-platform=x11)
    fi
    pnpm --filter hyperion exec electron-vite dev -- "${x11[@]}" {{ args }}

# A client of a running server, like the bridge: it sends R03's `scene_ship`, since no console
# does until sessions exist, and the server starts the ship at the galactic centre, in no system,
# where `VIEW` shows only its kept scene under `TRAINING`. The setting is not saved, so run it again
# after each server start. `just place-ship --help` lists the options (universe, system, target,
# distance, time, rate; `--port` or `HYPERION_SERVER_PORT` as for the client).
# Place the ship stand-in in a generated system on the running server, e.g. `just place-ship --rate 10`.
[positional-arguments]
place-ship *args:
    cd apps/hyperion && node scripts/placeShip.mjs "$@"

# The descent's demand record (R05.T13.a, Design note 19): the scripted descent at a fixed step
# through `selectPatches` with ranges from real bakes, in cells of {hard, min(hard, 4σ_n)} ×
# {ridges off, on} × {high, low}, written to `docs/measurements/descent-spike/`. Options:
# `--rules hard,calibrated`, `--ridges off,on`, `--settings high,low`, `--rate 64` (Hz),
# `--cap-hours 2` (wall time, shared by the cells; a cell cut short says so), `--out <dir>`;
# `--write-fixture` writes the unit test's ranges instead. CPU only; not part of `ci`.
# Record the descent's patch demand, e.g. `just descent-demand --rules calibrated`.
[positional-arguments]
descent-demand *args: gen-surface
    cd apps/hyperion && node --no-warnings scripts/descentDemand.mjs "$@"

# Build the client and run the headless smoke harness on SwiftShader, once per capability path
# (R01.T9, Design note 17): every catalogued shader offline, then the engine's checks on read-back
# frames. Not part of `ci` (R01.T9.e); every task touching `view/engine/`, `src/smoke/` or a
# catalogued shader runs it. Arguments go to `apps/hyperion/scripts/testRender.sh`
# (`--variant=`, `--fixture=broken-wgsl|external-fetch`, `--drop-adapter-switches`).
test-render *args:
    pnpm --filter hyperion build
    just _locked bash apps/hyperion/scripts/testRender.sh {{ args }}

# Replay a descent-spike capture natively (R05.T15, Design note 22): `tools/gpu-replay`, outside
# the workspace, so `ci` never builds wgpu. It validates the capture's WGSL with naga, replays its
# frames offscreen on the default adapter and writes a results file into
# `docs/measurements/descent-spike/` (`--out` elsewhere). `--present` replays in a window with FIFO
# presentation instead: a visible window, so by hand only. `--setting high|low` where the capture
# records none.
# Replay a capture, e.g. `just replay /path/to/capture --present`.
[positional-arguments]
replay *args:
    cargo run --release --manifest-path tools/gpu-replay/Cargo.toml -- replay "$@"

# Typecheck Rust and TypeScript.
check: gen-surface
    cargo check --workspace --all-targets
    pnpm typecheck

# Lint Rust (clippy) and TypeScript (oxlint, type-aware).
lint: gen-surface
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
test: gen-surface
    cargo test --workspace --no-run
    just _locked bash -c 'cargo test --workspace && pnpm test'

# Run the slow tests (`#[ignore = "slow: ..."]`) under the slow-test profile, with cargo-nextest
# (`cargo install cargo-nextest --locked`) so that every binary's tests share one pool of cores.
# Nextest runs no doctests, but no doctest is slow. `.config/nextest.toml` holds the `slow` profile.
# Then the fitted tables' check reruns every fast fit and compares bytes (plan 15, P15.T2). It uses
# every core too, so it runs in the same hold of the lock, from the binary built beforehand, and is
# timed.
[positional-arguments]
test-slow *args:
    cargo nextest run --workspace --cargo-profile slow-test --profile slow --run-ignored only --no-run
    cargo build -q -p hyperion-fit
    just _locked bash -c '{{ slow_then_check }}' test-slow "$@"

# The body of `test-slow`'s locked step: the slow tests with the recipe's arguments, then the timed
# check.
slow_then_check := 'cargo nextest run --workspace --cargo-profile slow-test --profile slow --run-ignored only "$@" && start=$SECONDS && cargo run -q -p hyperion-fit -- check --rerun-fast && echo "hyperion-fit check --rerun-fast: $((SECONDS - start)) s" >&2'

# The WebAssembly checks (plan R04). Two targets: `wasm32-wasip1` under wasmtime, a second
# architecture with a 32-bit `usize`, for base, the surface crate, the testkit and the sim; and
# `wasm32-unknown-unknown`, the client's, under wasm-bindgen-test (R04.T8). `just wasm-tools`
# installs what they need. Every recipe checks its tools first and fails, never skips, when one is
# missing: a gate that passes by skipping is not a gate (R04 Design note 11).

# wasmtime, as `just wasm-tools` installs it.
wasmtime_version := "49.0.1"

# wasm-bindgen-cli, as `just wasm-tools` installs it: the version of `wasm-bindgen` that
# `Cargo.lock` names, which wasm-bindgen-test's runner must match exactly.
wasm_bindgen_version := `sed -n '/^name = "wasm-bindgen"$/{n;s/^version = "\(.*\)"$/\1/p;}' Cargo.lock | head -n 1`

# The crates whose suites run under wasm32-wasip1.
wasip1_crates := "-p hyperion-base -p hyperion-surface -p hyperion-sim -p hyperion-testkit"

# cargo-nextest is installed only if missing; `test-slow` needs it as well.
# Install the WebAssembly checks' tools: both targets, wasmtime, wasm-bindgen-cli and nextest.
wasm-tools:
    rustup target add wasm32-wasip1 wasm32-unknown-unknown
    cargo install wasmtime-cli --version {{ wasmtime_version }} --locked
    cargo install wasm-bindgen-cli --version {{ wasm_bindgen_version }} --locked
    command -v cargo-nextest >/dev/null || cargo install cargo-nextest --locked

# Fail, naming `just wasm-tools`, unless each tool group named is present. `wasip1`: the target,
# wasmtime (or `WASMTIME` set to it) and cargo-nextest. `browser`: the target and
# wasm-bindgen-test's runner; `bindgen`: the target and `wasm-bindgen`, each at `Cargo.lock`'s
# version.
[positional-arguments]
_wasm-preflight +tools:
    #!/usr/bin/env bash
    set -euo pipefail
    missing() {
        echo "error: $1 is missing: run \`just wasm-tools\` to install the WebAssembly checks' tools" >&2
        exit 1
    }
    installed="$(rustup target list --installed)"
    for tool in "$@"; do
        case "$tool" in
            wasip1)
                grep -qx wasm32-wasip1 <<<"$installed" || missing "the rustup target wasm32-wasip1"
                found="$("${WASMTIME:-wasmtime}" --version 2>/dev/null || true)"
                [[ "$found" == "wasmtime {{ wasmtime_version }}"* ]] \
                    || missing "wasmtime {{ wasmtime_version }} (${WASMTIME:-wasmtime}: '${found:-none}')"
                cargo nextest --version >/dev/null 2>&1 || missing "cargo-nextest"
                ;;
            browser)
                grep -qx wasm32-unknown-unknown <<<"$installed" \
                    || missing "the rustup target wasm32-unknown-unknown"
                runner="$(wasm-bindgen-test-runner --version 2>/dev/null || true)"
                [[ "$runner" == "wasm-bindgen-test-runner {{ wasm_bindgen_version }}" ]] \
                    || missing "wasm-bindgen-test-runner {{ wasm_bindgen_version }} (found '${runner:-none}')"
                ;;
            bindgen)
                grep -qx wasm32-unknown-unknown <<<"$installed" \
                    || missing "the rustup target wasm32-unknown-unknown"
                cli="$(wasm-bindgen --version 2>/dev/null || true)"
                [[ "$cli" == "wasm-bindgen {{ wasm_bindgen_version }}" ]] \
                    || missing "wasm-bindgen {{ wasm_bindgen_version }} (found '${cli:-none}')"
                ;;
            *)
                echo "error: _wasm-preflight: unknown tool group $tool" >&2
                exit 2
                ;;
        esac
    done

# `cargo <args>` with wasmtime as the runner for wasm32-wasip1. Goldens are read at host paths fixed
# at compile time, so the guest is given the repository, and the target directory if it lies
# elsewhere, at those same paths. The runner is set here, not in `.cargo/config.toml`, so that a
# bare `cargo test --target wasm32-wasip1` names none.
[positional-arguments]
_wasip1 +args:
    #!/usr/bin/env bash
    set -euo pipefail
    dirs="--dir={{ justfile_directory() }}"
    if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
        dirs+=" --dir=$(realpath -m "$CARGO_TARGET_DIR")"
    fi
    export CARGO_TARGET_WASM32_WASIP1_RUNNER="${WASMTIME:-wasmtime} $dirs"
    cargo "$@"

# The wasip1 suites run under cargo-nextest, one wasmtime process per test across every core:
# wasip1 has no threads, so `cargo test` ran each binary's tests one at a time, the fast suite in
# 975 s against nextest's 206 s (R04.T7.c, under shared load). wasmtime caches compiled modules, so
# a binary is compiled once, not once per test.
wasip1_nextest := "nextest run --target wasm32-wasip1 " + wasip1_crates

# Fail unless a build of the surface crate with relaxed SIMD stops on a crate's guard (R04 Design
# note 10). Its own target directory keeps the flagged build from evicting the unflagged one.
_relaxed-simd-refused:
    #!/usr/bin/env bash
    set -euo pipefail
    log="$(mktemp)"
    trap 'rm -f "$log"' EXIT
    if RUSTFLAGS="-C target-feature=+relaxed-simd" \
        CARGO_TARGET_DIR="{{ justfile_directory() }}/target/relaxed-simd-check" \
        cargo build --target wasm32-wasip1 -p hyperion-surface >"$log" 2>&1; then
        echo "error: built with +relaxed-simd: the guards of hyperion-base and hyperion-surface are gone" >&2
        exit 1
    fi
    if ! grep -q "relaxed SIMD is banned in hyperion-" "$log"; then
        cat "$log" >&2
        echo "error: the +relaxed-simd build failed, but not on the relaxed-SIMD guard" >&2
        exit 1
    fi
    echo "the +relaxed-simd build is refused by its guard" >&2

# Built first, then run under the heavy-test lock. The doctests wait for `test-wasm-slow`.
# The fast WebAssembly checks, run by `ci`: relaxed SIMD refused, wasip1, Clippy and the browser.
test-wasm-fast: (_wasm-preflight "wasip1" "browser") _relaxed-simd-refused _browser-clippy
    just _wasip1 {{ wasip1_nextest }} --no-run
    just _locked just _wasip1 {{ wasip1_nextest }}
    just test-wasm-browser

# Clippy for the browser target over the crates that run there, so that code compiled only there
# (the testkit's embedded golden arm, the surface crate's `src/wasm.rs`) is linted under the
# crates' own `clippy.toml` files, which `just lint`, run on the host, never applies to it, and so
# that the bans of the relaxed intrinsics bind (R04 Design notes 10 and 12).
_browser-clippy: (_wasm-preflight "browser")
    cargo clippy --target wasm32-unknown-unknown -p {{ replace(browser_crates, " ", " -p ") }} --all-targets -- -D warnings

# Built first, then run under the heavy-test lock: the slow tests at the slow-test profile with
# `test-slow`'s nextest profile, then the doctests, which nextest does not run and `cargo test` runs
# one at a time (88 s under shared load, so not in `ci`; they are built in the locked step, since
# cargo cannot build doctests without running them).
# The slow WebAssembly checks, run by `ci-slow`: the slow suites and the doctests under wasip1.
test-wasm-slow: (_wasm-preflight "wasip1")
    just _wasip1 {{ wasip1_nextest }} --cargo-profile slow-test --profile slow --run-ignored only --no-run
    just _locked just _wasip1 {{ wasip1_nextest }} --cargo-profile slow-test --profile slow --run-ignored only
    just _locked just _wasip1 test --target wasm32-wasip1 {{ wasip1_crates }} --doc

# The client's surface module: the surface crate built for wasm32-unknown-unknown in release, then
# `wasm-bindgen --target web` into the renderer's `generated/surface/`, which git, Prettier and
# oxlint ignore (R04.T10.b). `check`, `lint`, `test`, `client` and `build` make it first, since the
# client's TypeScript needs its `.d.ts`, and so do the git hooks that run pnpm (`_with-surface`).
# Build the surface crate's WebAssembly module and its glue for the client.
gen-surface: (_wasm-preflight "bindgen")
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{ justfile_directory() }}"
    cargo build -q -p hyperion-surface --lib --target wasm32-unknown-unknown --release
    target="${CARGO_TARGET_DIR:-target}"
    out=apps/hyperion/src/renderer/src/generated/surface
    rm -rf "$out"
    wasm-bindgen --target web --out-dir "$out" \
        "$target/wasm32-unknown-unknown/release/hyperion_surface.wasm"

# Run a command once the surface module is built: the git hooks' pnpm entries, since a fresh
# worktree has no generated module and the client's typecheck, lint and tests need it.
[positional-arguments]
_with-surface +cmd: gen-surface
    "$@"

# Both the fast and the slow WebAssembly checks.
test-wasm: test-wasm-fast test-wasm-slow

# The crates whose suites run on wasm32-unknown-unknown, the client's target: those the client ships
# or tests with. Not the sim, which no browser loads (R04 Design note 11).
browser_crates := "hyperion-base hyperion-surface hyperion-testkit"

# How long one test binary may run on the browser target, in seconds: wasm-bindgen-test's Node mode
# has no timeout of its own.
browser_timeout := "600"

# Run the browser target's suites (`--lib --tests`, at the slow-test profile, since debug wasm is
# slow) on the V8 that Electron ships, through `tools/electron-node/node` (R04 Design note 13).
# First it proves, through the shim, that `node` is Electron; then it builds, compares each crate's
# test list with the native one less its `native_only` tests, since a plain `#[test]` compiles there
# and is silently dropped (Design note 12); then it runs the tests under the heavy-test lock.
# The browser target's suites, on Electron's V8: base, the surface crate, the testkit.
test-wasm-browser: (_wasm-preflight "browser")
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{ justfile_directory() }}"
    electron="$(pnpm --silent --filter hyperion exec node -p "require('electron')" | tail -n 1)"
    expected="$(pnpm --silent --filter hyperion exec node -p "require('electron/package.json').version" | tail -n 1)"
    if [[ ! -x "$electron" ]]; then
        echo "error: Electron's binary ($electron) is missing: run \`just install\`" >&2
        exit 1
    fi
    export HYPERION_ELECTRON="$electron"
    export PATH="{{ justfile_directory() }}/tools/electron-node:$PATH"
    version="$(node -p process.versions.electron 2>/dev/null || true)"
    if [[ "$version" != "$expected" ]]; then
        echo "error: \`node\` on PATH is not Electron $expected through tools/electron-node (it printed '$version')" >&2
        exit 1
    fi
    echo "wasm-bindgen-test on Electron $version, V8 $(node -p process.versions.v8)"
    export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER="timeout {{ browser_timeout }} wasm-bindgen-test-runner"
    packages=()
    for crate in {{ browser_crates }}; do packages+=(-p "$crate"); done
    browser=(--target wasm32-unknown-unknown --profile slow-test --lib --tests)
    cargo test "${browser[@]}" "${packages[@]}" --no-run
    cargo test --lib --tests "${packages[@]}" --no-run
    # The names of the tests in a `--list` output, sorted; `grep` finding none is not an error.
    tests() { { grep -E ': test$' || true; } | sed -e 's/: test$//' | sort; }
    for crate in {{ browser_crates }}; do
        # Each listing runs on its own, so that a failed build or runner stops the recipe here with
        # its own message rather than showing as a difference between the lists.
        native_list="$(cargo test -q --lib --tests -p "$crate" -- --list)"
        browser_list="$(cargo test "${browser[@]}" -p "$crate" -- --list)"
        native="$(tests <<<"$native_list" | { grep -v 'native_only::' || true; })"
        on_browser="$(tests <<<"$browser_list")"
        if [[ "$native" != "$on_browser" ]]; then
            echo "error: $crate's tests on wasm32-unknown-unknown differ from its native ones less native_only::" >&2
            echo "(a test file or module without the \`wasm_bindgen_test as test\` import, or a test compiled out outside a native_only module):" >&2
            diff <(echo "$native") <(echo "$on_browser") >&2 || true
            exit 1
        fi
        echo "$crate: $(grep -c . <<<"$native") tests on the browser target, as natively less native_only"
    done
    just _locked cargo test "${browser[@]}" "${packages[@]}"

# Run the Criterion benchmarks, e.g. `just bench -- samplers`.
bench *args:
    cargo bench --workspace --no-run {{ args }}
    just _locked cargo bench --workspace {{ args }}

# GENERATOR_VERSION, read from its source in `hyperion-base` (re-exported by the sim) for `--since`.
generator_version := `sed -n 's/^pub const GENERATOR_VERSION: GeneratorVersion = GeneratorVersion::new(\([0-9]*\));$/\1/p' crates/hyperion-base/src/version.rs`

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
build: gen-surface
    cargo build --workspace --release
    pnpm build

# The gate before a commit: everything but the slow tests, with the fast suites on WebAssembly.
ci: fmt-check check lint test fit-check gen-protocol-check test-wasm-fast

# `ci` plus the slow statistical tests, natively and on WebAssembly: the full gate.
ci-slow: ci test-slow test-wasm-slow
