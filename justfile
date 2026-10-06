# HYPERION task runner — `just` lists recipes, `just ci` is the gate before a commit and
# `just ci-slow` adds the slow statistical tests.
#
# The recipes and their scripts run on Linux and on stock macOS, and on Windows from WSL or Git
# Bash. macOS's bash is 3.2 and its tools are BSD's, so they use neither bash 4 (`exec {fd}<`,
# `declare -A`, `mapfile`, `${x,,}`) nor GNU-only options (`cp --reflink`, `realpath -e`, `sed -i`
# with no suffix, `date +%N`, `stat -c`), and an array that may be empty is expanded as
# `${a[@]+"${a[@]}"}`, since bash before 4.4 calls `"${a[@]}"` of an empty array unbound under
# `set -u`. The util-linux and coreutils tools that macOS lacks are in `tools/portable`.

set shell := ["bash", "-euo", "pipefail", "-c"]

# Last on PATH, so that a system's own tools come first and Linux runs util-linux's and
# coreutils': stand-ins for `flock`, `setsid` and `timeout`, which stock macOS lacks, in Perl,
# which it ships. Trimmed before it is appended, so that a recipe that runs `just` adds it once.
portable := justfile_directory() / "tools" / "portable"
path_separator := if os_family() == "windows" { ";" } else { ":" }
export PATH := trim_end_match(env("PATH"), path_separator + portable) + path_separator + portable

_default:
    @just --list

# Install frontend dependencies.
install:
    pnpm install

# A new worktree starts with no `target/`, and its first `just ci` builds every dependency for
# every profile and target. This gives it another worktree's build directory as a copy-on-write
# clone: instant, and taking no space until either copy changes. On Linux it clones with
# `cp --reflink=always`, which btrfs and XFS support; on macOS with clonefile(2), which APFS
# supports, called through Python's ctypes (the Command Line Tools ship Python 3), since macOS's
# `cp -c` makes a full copy where it cannot clone. Dependencies from the registry are then fresh;
# the workspace's own crates are rebuilt, since their paths differ. It refuses when `target/`
# exists, and when the filesystem cannot clone it says so and copies nothing, since a plain copy
# would be tens of gigabytes. It holds each of the source's cargo build-directory locks while
# copying, so that no build of the source is caught half-written; a build there waits for the copy,
# and the copy for a build.
# Seed this worktree's `target/` from another worktree's, e.g. `just seed-target ../agent-x`.
seed-target source:
    #!/usr/bin/env bash
    set -euo pipefail
    here="{{ justfile_directory() }}"
    src="$(CDPATH='' cd -- "{{ source }}" && pwd -P)/target"
    if [[ -e "$here/target" ]]; then
        echo "error: $here/target exists; seed-target only seeds a worktree that has none" >&2
        exit 1
    fi
    if [[ ! -d "$src" ]]; then
        echo "error: $src is not a directory: name a worktree that has built" >&2
        exit 1
    fi
    if [[ "$src" -ef "$here/target" ]]; then
        echo "error: the source is this worktree" >&2
        exit 1
    fi
    if [[ "$(uname -s)" == Darwin ]]; then
        # clonefile(2) clones a directory's whole tree, timestamps included, or fails.
        clone=(python3 -c '
    import ctypes, os, sys
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.clonefile(os.fsencode(sys.argv[1]), os.fsencode(sys.argv[2]), 0) != 0:
        sys.exit("clonefile {} {}: {}".format(sys.argv[1], sys.argv[2], os.strerror(ctypes.get_errno())))
    ')
        how="clonefile(2)"
    else
        clone=(cp -r --reflink=always --preserve=timestamps)
        how="cp --reflink=always"
    fi
    probe="$here/.seed-target-probe"
    trap 'rm -rf "$probe" "$here/target.seeding"' EXIT
    if ! "${clone[@]}" "$here/justfile" "$probe" 2>/dev/null; then
        echo "error: this filesystem cannot clone files ($how failed), so nothing was copied;" >&2
        echo "a plain copy would take as much space again: build from scratch with \`just ci\` instead" >&2
        exit 1
    fi
    locks=()
    while IFS= read -r -d '' lock; do
        locks+=("$lock")
    done < <(find "$src" -maxdepth 3 -name .cargo-lock -print0)
    # Each lock on a descriptor of its own, numbered from 20, since bash 3.2 cannot allocate one
    # (`exec {fd}<`).
    fd=20
    for lock in ${locks[@]+"${locks[@]}"}; do
        eval "exec $fd<\"\$lock\""
        if ! flock -n "$fd"; then
            echo "waiting for a build in the source to finish ($lock)..." >&2
            flock "$fd"
        fi
        fd=$((fd + 1))
    done
    start=$SECONDS
    "${clone[@]}" "$src" "$here/target.seeding"
    mv "$here/target.seeding" "$here/target"
    echo "seeded $here/target from $src in $((SECONDS - start)) s" >&2

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
    pnpm --filter hyperion exec electron-vite dev -- ${x11[@]+"${x11[@]}"} {{ args }}

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

# The descent spike (R05.T13.c): builds everything, starts a local server with `--num-workers 2`
# and runs the client's `--descent-spike` with the spike's options (`--setting high|low`,
# `--seed <u64>`, `--smoke`, `--out <dir>`, `--workers <n>`, `--vertex-path`, `--normals`,
# `--ridged on|off`, `--dawn-safety on|off`, `--capture <dir>`, `--trace-profile on|off`) and the
# recipe's own: `--companion-load <threads>` (Design note 20), `--cold-cache` (an empty GPU shader
# cache) and `--hidden` (the window never shown). The trace is a Perfetto protobuf stream over CDP
# on the spike window's own debugger, taken in windows of script time (R05.T14.e, T14.i), with V8's
# CPU profiler and `gpu` only under `--trace-profile on`, a diagnostic run that is never judged.
# `--smoke` runs 10 s hidden, its trace in three windows, each decoded and checked frame by frame
# against the renderer's own series, writes no results and exits with a status. A run's profile,
# where its trace windows wait, and Electron's `TMPDIR`, where Chromium spools each window's
# stream, are under `target/descent-spike/`, on disk. A run writes its results under
# `docs/measurements/descent-spike/`. Not part of `ci`.
# Run the descent spike, e.g. `just descent-spike --setting low` or `just descent-spike --smoke`.
[positional-arguments]
descent-spike *args:
    just build
    bash apps/hyperion/scripts/descentSpike.sh "$@"

# The several-views check (R07.T20): builds the client and runs it with `--views-check`, which
# opens VIEW on the kept PHASE TEST scene, drives it through its phases (a photorealistic primary
# alone and with two wireframe instruments, a resize of the primary alone, a wireframe primary with
# two wireframe instruments, with a photorealistic and a wireframe instrument, and alone) and
# writes a results file and its summary into `docs/measurements/several-views/`. Options:
# `--setting high|low` (VIEW's setting; the window 1920 × 1080 or 1280 × 720), `--hidden` (never
# shown, offscreen), `--smoke` (short windows, hidden, written under `target/views-check/`) and
# `--out <dir>`. A shown run asks once whether every view is the right way up. No server is needed.
# By hand, on a quiet machine; not part of `ci`.
# Run the several-views check, e.g. `just views-check` or `just views-check --setting low`.
[positional-arguments]
views-check *args: gen-surface
    pnpm --filter hyperion build
    bash apps/hyperion/scripts/viewsCheck.sh "$@"

# R07.T21's child window on a second monitor: builds the client and runs the smoke harness's
# child-window scene on the client's own graphics switches, the opener on the primary display and a
# same-origin child window, a view of the opener's engine, on the second. It records the child's
# pacing beside its display's period, both views' frame times and the release of the child's view
# on its `pagehide`, as `<date>-<machine>-child-window.md` in `docs/measurements/several-views/`.
# Options: `--seconds <n>` (the child's time, 60 by default), `--hidden` (an offscreen child on one
# display, the harness's own proof, written under `target/views-check/`) and `--out <dir>`. With
# one display it refuses (exit 2) before opening any window. Shown: by hand; not part of `ci`.
# Run the child-window check on two displays, e.g. `just child-window-check --seconds 60`.
[positional-arguments]
child-window-check *args: gen-surface
    pnpm --filter hyperion build
    bash apps/hyperion/scripts/childWindowCheck.sh "$@"

# Build the client and run the headless smoke harness on SwiftShader, once per capability path
# (R01.T9, Design note 17): every catalogued shader offline, then the engine's checks on read-back
# frames. Not part of `ci` (R01.T9.e); every task touching `view/engine/`, `src/smoke/` or a
# catalogued shader runs it. Arguments go to `apps/hyperion/scripts/testRender.sh`
# (`--variant=`, `--fixture=broken-wgsl|external-fetch`, `--drop-adapter-switches`). The surface
# module is made first, so that the build bundles the current one rather than a stale one.
test-render *args: gen-surface
    pnpm --filter hyperion build
    just _locked bash apps/hyperion/scripts/testRender.sh {{ args }}

# Replay a descent-spike capture natively (R05.T15, Design note 22): `tools/gpu-replay`, outside
# the workspace, so that the workspace's builds never build wgpu (`ci` checks the tool in a target
# directory of its own, `gpu-replay-check`). It validates the capture's WGSL with naga, replays its
# frames offscreen on the default adapter and writes a results file into
# `docs/measurements/descent-spike/` (`--out` elsewhere). `--present` replays in a window with FIFO
# presentation instead: a visible window, so by hand only. `--setting high|low` where the capture
# records none.
# Replay a capture, e.g. `just replay /path/to/capture --present`.
[positional-arguments]
replay *args:
    cargo run --release --manifest-path tools/gpu-replay/Cargo.toml -- replay "$@"

# `cargo <args>` on `tools/gpu-replay`, in `target/tools`: its own build directory, so that its
# builds run beside the workspace's without waiting on their lock, and wgpu never lands in theirs.
[positional-arguments]
_gpu-replay-cargo command *args:
    cargo "$1" --locked --manifest-path "{{ justfile_directory() }}/tools/gpu-replay/Cargo.toml" \
        --target-dir "{{ justfile_directory() }}/target/tools" "${@:2}"

# Clippy over every target of the tool, then its tests less the one that needs a GPU adapter
# (`tests/replay.rs`, ignored): the capture reader, naga's validation of the fixture's WGSL and
# the results' schema, on the CPU in well under a second, so they run outside the heavy-test lock.
# A cold build costs about a minute for each of Clippy and the tests (measured 2026-10-03 under
# shared load), a few seconds after an edit. `ci` runs it beside the workspace's builds.
# Lint and test tools/gpu-replay, less its GPU test.
gpu-replay-check:
    just _gpu-replay-cargo clippy --all-targets -- -D warnings
    just _gpu-replay-cargo test

# The tool's test that replays the fixture on the default GPU adapter, offscreen, with no window,
# under the heavy-test lock. `ci-slow` runs it; it fails where no adapter is found.
# Run tools/gpu-replay's GPU test (needs an adapter).
test-gpu-replay:
    just _gpu-replay-cargo test --no-run
    just _locked just _gpu-replay-cargo test --test replay -- --ignored

# Typecheck Rust and TypeScript.
check: gen-surface
    cargo check --workspace --all-targets
    pnpm typecheck

# Lint Rust (clippy) and TypeScript (oxlint, type-aware).
lint: gen-surface _clippy _oxlint

# Clippy over every target of the workspace. It compiles each one as `cargo check` does, so a
# compile error fails it too: `ci` runs it in place of `check`'s `cargo check`.
_clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# The platforms that `cross-clippy` checks, less the one it runs on: Linux, macOS on Apple silicon
# and Windows, each on the architecture its developers use. `rust-toolchain.toml` lists them too,
# so that rustup installs their standard libraries.
cross_targets := "x86_64-unknown-linux-gnu aarch64-apple-darwin x86_64-pc-windows-msvc"

# `_clippy` and `gpu-replay-check`'s Clippy for the other platforms (macOS and Windows, from
# Linux): code gated to one platform (`cfg(unix)`, `target_os = "linux"`) can leave an import or a
# helper unused on another, which only that platform's Clippy sees. Clippy never links, so the
# dependencies that compile C for their target (alloca, under the dev-dependency criterion, and
# gpu-replay's wayland-backend) get a stand-in compiler and archiver that write empty files, and no
# SDK is needed: only the rustup targets. The host keeps its own compiler for the build scripts. It
# builds in `target/cross` and `target/tools-cross`, beside the other builds, so `ci` runs it with
# the checks beside them: about a second when nothing changed, a minute or more from cold.
# Clippy for the other platforms (macOS and Windows from Linux), with no SDK.
cross-clippy:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ justfile_directory() }}"
    host="$(rustc -vV | sed -n 's/^host: //p')"
    targets=()
    for target in {{ cross_targets }}; do
        if [[ "$target" != "$host" ]]; then targets+=("$target"); fi
    done
    installed="$(rustup target list --installed)"
    for target in "${targets[@]}"; do
        grep -qx "$target" <<<"$installed" || {
            echo "error: the rustup target $target is missing: run \`rustup target add ${targets[*]}\`" >&2
            exit 1
        }
    done
    mkdir -p "$root/target/cross"
    stub="$root/target/cross/stub-cc"
    # cc-rs probes the compiler with `-E` (answered as clang) and `-?` (refused, so not cl), then
    # compiles to `-o`/`-Fo` and archives with `ar <mode> <archive>` or `-out:<archive>`. Written
    # beside and renamed into place, so that a build running the old one never reads half of it.
    cat >"$stub.new" <<'EOF'
    #!/usr/bin/env bash
    # cross-clippy's stand-in C compiler and archiver: an empty file at the output path.
    set -euo pipefail
    out=""
    if [[ $# -ge 2 && "$1" =~ ^[A-Za-z]+$ ]]; then out="$2"; fi
    prev=""
    for arg in "$@"; do
        case "$arg" in
            -E) echo '#pragma message "clang"'; exit 0 ;;
            '-?' | '/?') exit 1 ;;
            --version) echo stub-cc; exit 0 ;;
        esac
        if [[ "$prev" == -o ]]; then out="$arg"; fi
        case "$arg" in
            -out:*) out="${arg#-out:}" ;;
            -o?*) out="${arg#-o}" ;;
            -Fo?* | /Fo?*) out="${arg#?Fo}" ;;
        esac
        prev="$arg"
    done
    if [[ -z "$out" ]]; then echo "stub-cc: no output path in: $*" >&2; exit 1; fi
    : >"$out"
    EOF
    chmod +x "$stub.new"
    mv -f "$stub.new" "$stub"
    flags=()
    for target in "${targets[@]}"; do
        export "CC_${target//-/_}=$stub" "AR_${target//-/_}=$stub"
        flags+=(--target "$target")
    done
    cargo clippy --workspace --all-targets "${flags[@]}" --target-dir "$root/target/cross" -- -D warnings
    cargo clippy --locked --manifest-path "$root/tools/gpu-replay/Cargo.toml" --all-targets "${flags[@]}" \
        --target-dir "$root/target/tools-cross" -- -D warnings

# oxlint, type-aware; it reads the surface module's `.d.ts`, so `gen-surface` must have run.
_oxlint:
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

# Run a command under the heavy-test lock, waiting for its turn (a crashed holder releases it). The
# lock is flock(2) on the lock file, held by this shell's descriptor 9 and the copies its children
# inherit, and so released when they all end, whatever ends them. `flock` is util-linux's, or on
# stock macOS `tools/portable/flock`, which takes the same lock, so the two exclude each other.
[positional-arguments]
_locked +cmd:
    #!/usr/bin/env bash
    set -euo pipefail
    exec 9>"{{ heavy_lock }}"
    if ! flock -n 9; then
        echo "waiting for another heavy test run to finish (lock {{ heavy_lock }})..." >&2
        waited=$SECONDS
        flock 9
        echo "lock taken after $((SECONDS - waited)) s of waiting, running: $*" >&2
    fi
    held=$SECONDS
    status=0
    # Run in its own memory-capped cgroup where systemd allows it, so that a runaway heavy run is
    # stopped (by the cap, or by systemd-oomd watching the user manager) instead of freezing the
    # machine or taking the login session with it, as happened on 2026-10-03. `HEAVY_MEMORY_MAX`
    # overrides the cap, and `HEAVY_SLICE` names a user slice to run it in (such as one that bounds
    # all of a session's agent work together). With no systemd (macOS) it runs uncapped.
    if command -v systemd-run >/dev/null && systemd-run --user --scope --quiet true 2>/dev/null; then
        slice=()
        if [ -n "${HEAVY_SLICE:-}" ]; then slice=(--slice="$HEAVY_SLICE"); fi
        systemd-run --user --scope --quiet ${slice[@]+"${slice[@]}"} -p MemoryMax="${HEAVY_MEMORY_MAX:-22G}" -p MemorySwapMax=2G "$@" || status=$?
    else
        "$@" || status=$?
    fi
    echo "heavy-test lock released after $((SECONDS - held)) s" >&2
    exit "$status"

# The native suites run under cargo-nextest, which schedules every test of every binary on one pool
# of cores, where `cargo test` ran its 108 binaries one after another, each finishing on its longest
# test with most cores idle: 419 s against 221 s in the lock for the same tests, doctests and vitest
# included (2026-10-03, both under heavy shared load). Nextest runs each test in its own process, so
# a fixture in a `OnceLock` is built once per test rather than once per binary; the suite is written
# for that already (`test-slow` and the wasip1 suites run under nextest too). It runs no doctests,
# so `cargo test --doc` follows; doctests cannot be built without being run, so they build inside
# the lock, as they did under `cargo test`.
# Run all tests (built first, then run under the heavy-test lock).
test: gen-surface _test-build
    just _locked just _test-run

# Build the native test binaries, outside the heavy-test lock. Not after `gen-surface` itself: `ci`
# builds while `tsc` and oxlint read the module that `gen-surface` would delete and rewrite.
_test-build:
    cargo nextest run --workspace --no-run

# Run the native suites, the doctests and vitest; the caller holds the heavy-test lock.
_test-run:
    cargo nextest run --workspace
    cargo test --workspace --doc
    pnpm test

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

# The client's slow tests: the descent spike's terrain clearance on all six of its ruling's runs
# (seeds 0, 1 and 7, ridges off and on, decision-r05-descent-clearance.md), some 600 real bakes a
# run. `just ci` runs the roughest of them alone.
test-slow-client: gen-surface
    just _locked bash -c 'cd apps/hyperion && HYPERION_SLOW_TESTS=1 pnpm exec vitest run src/renderer/src/view/spike/clearance.wasm.test.ts'

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
        # Its absolute path, symbolic links resolved; cargo would create it.
        mkdir -p "$CARGO_TARGET_DIR"
        dirs+=" --dir=$(CDPATH='' cd -- "$CARGO_TARGET_DIR" && pwd -P)"
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

# Built first, then run in one hold of the heavy-test lock. The doctests wait for `test-wasm-slow`.
# The fast WebAssembly checks, run by `ci`: relaxed SIMD refused, wasip1, Clippy and the browser.
test-wasm-fast: _relaxed-simd-refused _browser-clippy _wasm-fast-build
    just _locked just _wasm-fast-run

# The builds of `test-wasm-fast`, outside the heavy-test lock, and the browser target's test lists.
_wasm-fast-build: (_wasm-preflight "wasip1" "browser")
    just _wasip1 {{ wasip1_nextest }} --no-run
    just _browser prepare

# The fast WebAssembly suites, built already; the caller holds the heavy-test lock.
_wasm-fast-run:
    just _wasip1 {{ wasip1_nextest }}
    just _browser run

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
    # Generated beside the module and swapped in only when it differs, so that an unchanged module
    # keeps its files' times and the caches keyed on them (tsc's, Vite's) stay valid.
    fresh="$out.new"
    rm -rf "$fresh"
    wasm-bindgen --target web --out-dir "$fresh" \
        "$target/wasm32-unknown-unknown/release/hyperion_surface.wasm"
    if diff -rq "$fresh" "$out" >/dev/null 2>&1; then
        rm -rf "$fresh"
    else
        rm -rf "$out"
        mv "$fresh" "$out"
    fi

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
# has no timeout of its own. The runner's `timeout` is coreutils', or `tools/portable/timeout`.
browser_timeout := "600"

# Run the browser target's suites (`--lib --tests`, at the slow-test profile, since debug wasm is
# slow) on the V8 that Electron ships, through `tools/electron-node/node` (R04 Design note 13).
# First it proves, through the shim, that `node` is Electron; then it builds, compares each crate's
# test list with the native one less its `native_only` tests, since a plain `#[test]` compiles there
# and is silently dropped (Design note 12); then it runs the tests under the heavy-test lock.
# The browser target's suites, on Electron's V8: base, the surface crate, the testkit.
test-wasm-browser: (_browser "prepare")
    just _locked just _browser run

# The browser target's suites in two steps: `prepare` proves the shim, builds both targets and
# compares the test lists, outside the lock; `run` runs the tests, built already, and its caller
# holds the heavy-test lock. Both lists come from one `cargo test` over the three crates at once,
# with each test attributed to its crate by the binary it is listed from: a `cargo test -p` of one
# crate resolves features for that crate alone, and so built every crate a second time for its list
# (47 s of a cold `ci`, 2026-10-03), and again after every edit.
_browser mode: (_wasm-preflight "browser")
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
    case "{{ mode }}" in
        run)
            cargo test "${browser[@]}" "${packages[@]}"
            exit 0
            ;;
        prepare) ;;
        *)
            echo "error: _browser: unknown mode {{ mode }}" >&2
            exit 2
            ;;
    esac
    cargo test "${browser[@]}" "${packages[@]}" --no-run
    cargo test --lib --tests "${packages[@]}" --no-run
    # `crate test` lines, sorted, of a `cargo test <args> -- --list` over the three crates: each
    # binary's `Running` line names its file, and the build's artifact messages map the file to its
    # crate. Every other line goes to standard error, so that a failed build or runner shows there
    # and stops the recipe with its own message, not as a difference between the lists.
    lists() {
        cargo test "$@" "${packages[@]}" --message-format=json-render-diagnostics -- --list 2>&1 \
            | python3 -c '
    import json, os, re, sys
    crate_of, crate = {}, None
    for line in sys.stdin:
        line = line.rstrip("\n")
        if line.startswith("{"):
            message = json.loads(line)
            if message.get("reason") == "compiler-artifact" and message.get("executable"):
                crate_of[os.path.basename(message["executable"])] = os.path.basename(
                    os.path.dirname(message["manifest_path"]))
            continue
        running = re.match(r"\s*Running .*\((.*)\)$", line)
        if running:
            crate = crate_of[os.path.basename(running.group(1))]
        elif line.endswith(": test"):
            print(crate, line[: -len(": test")])
        elif line and not re.fullmatch(r"\d+ tests?, \d+ benchmarks?", line):
            print(line, file=sys.stderr)
    ' | sort
    }
    native_lists="$(lists --lib --tests)"
    browser_lists="$(lists "${browser[@]}")"
    # The names of one crate's tests in those lines; `grep` finding none is not an error.
    tests() { { grep -E "^$1 " || true; } | cut -d ' ' -f 2-; }
    for crate in {{ browser_crates }}; do
        native="$(tests "$crate" <<<"$native_lists" | { grep -v 'native_only::' || true; })"
        on_browser="$(tests "$crate" <<<"$browser_lists")"
        if [[ "$native" != "$on_browser" ]]; then
            echo "error: $crate's tests on wasm32-unknown-unknown differ from its native ones less native_only::" >&2
            echo "(a test file or module without the \`wasm_bindgen_test as test\` import, or a test compiled out outside a native_only module):" >&2
            diff <(echo "$native") <(echo "$on_browser") >&2 || true
            exit 1
        fi
        echo "$crate: $(grep -c . <<<"$native") tests on the browser target, as natively less native_only"
    done

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
    # Over the workspace's library tests rather than `-p hyperion-protocol`, so that features resolve
    # as for the workspace's test build and the build is that one, not a second build of the protocol
    # and its dependencies with features of their own (41 s of a cold `ci`, 2026-10-03). Only the
    # protocol's tests are named `export_bindings*`.
    TS_RS_EXPORT_DIR="$fresh" cargo test --quiet --workspace --lib export_bindings
    diff -r "$fresh" packages/protocol/src/generated \
        || { echo "protocol bindings are stale: run 'just gen-protocol'" >&2; exit 1; }

# Build release artifacts.
build: gen-surface
    cargo build --workspace --release
    pnpm build

# `ci` runs what `fmt-check check lint test fit-check gen-protocol-check test-wasm-fast` ran, and
# `cross-clippy`, in three phases. First, beside the builds, the checks that need no cargo build
# directory of the worktree (formatting, `tsc`, oxlint, and the relaxed-SIMD refusal,
# `gpu-replay-check` and `cross-clippy`, which have target directories of their own), their output
# held until they finish. Second, the cargo steps one after another, since they share the build
# directory's lock: Clippy, which compiles every target as `cargo check` would, so `check`'s
# `cargo check` is not repeated; the bindings' check, before any test can rewrite the bindings it
# compares; the fitted tables' check; then every test build. Third, one hold of the heavy-test lock
# for all the suites, rather than three turns in the queue behind other worktrees.
# The gate before a commit: everything but the slow tests, with the fast suites on WebAssembly.
ci: gen-surface (_wasm-preflight "wasip1" "browser")
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{ justfile_directory() }}"
    side_log="$(mktemp)"
    trap 'rm -f "$side_log"' EXIT
    just fmt-check _typecheck-ts _oxlint _relaxed-simd-refused gpu-replay-check cross-clippy >"$side_log" 2>&1 &
    side=$!
    status=0
    just _clippy _browser-clippy gen-protocol-check fit-check _test-build _wasm-fast-build || status=$?
    side_status=0
    wait "$side" || side_status=$?
    cat "$side_log"
    if [[ "$status" -ne 0 || "$side_status" -ne 0 ]]; then
        echo "error: ci failed before the tests (builds and Rust checks: exit $status; formatting, TypeScript, the relaxed-SIMD refusal, gpu-replay and cross-clippy: exit $side_status)" >&2
        exit 1
    fi
    just _locked just _test-run _wasm-fast-run

# TypeScript's type check; the client's needs the surface module's `.d.ts`, from `gen-surface`.
_typecheck-ts:
    pnpm typecheck

# `ci` plus the slow statistical tests, natively and on WebAssembly: the full gate.
ci-slow: ci test-slow test-wasm-slow test-gpu-replay test-slow-client
