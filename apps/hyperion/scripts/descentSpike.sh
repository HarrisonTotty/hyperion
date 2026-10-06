#!/usr/bin/env bash
# The descent spike's runner (R05.T13.c, Design notes 18 and 20), called by `just descent-spike`
# after a build. It starts a local hyperion-server with its pool capped at two workers, runs the
# built client with `--descent-spike` and the spike's options, and stops everything it started.
#
#   descentSpike.sh [--companion-load <threads>] [--cold-cache] [--hidden] [spike options...]
#
# Spike options (the client's, `src/main/cli.ts`): --setting high|low, --seed <u64>, --smoke,
# --out <dir>, --workers <n>, --vertex-path baked-offsets|face-differences, --normals double|mesh,
# --ridged on|off, --dawn-safety on|off, --capture <dir>, --trace-profile on|off (V8's CPU profiler
# in the trace, off by default: a profiled run is a diagnostic, never judged; R05.T14.e).
#
# --companion-load <threads> runs that many busy threads beside the run, standing in for the
# server's arrival work (Design note 20). --cold-cache starts with an empty Chromium GPU shader
# cache; otherwise the cache kept from the last run (in target/descent-spike/gpu-cache) is copied
# into the run's profile and kept again after it, so that cold and warm caches are run apart.
# --hidden never shows the window (offscreen rendering), as --smoke never does.
#
# Each run gets a fresh `--user-data-dir` under target/descent-spike/, on disk and never under
# TMPDIR, removed afterwards: the trace's window files wait there until the run ends, about 1.5 GB
# for a descent, which a RAM-backed /tmp cannot hold (decision-r05-trace-windows.md). The server,
# the companion load and Electron run in process groups of their own under `timeout`, each group
# killed when the script ends for any reason. The exit status is the client's: 0 pass, 1 fail, 3
# the watchdog. `setsid` and `timeout` are util-linux's and coreutils', or on macOS the justfile's
# `tools/portable` stand-ins. It runs on bash 3.2 (macOS's): an array that may be empty is expanded
# as `${a[@]+"${a[@]}"}`.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$here/../.." && pwd)"
electron="$here/node_modules/.bin/electron"
runs="$repo/target/descent-spike"
cache_keep="$runs/gpu-cache"
port="${HYPERION_SPIKE_PORT:-7879}"

companion=0
cold=0
hidden=0
spike_args=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --companion-load) companion="$2"; shift 2 ;;
        --companion-load=*) companion="${1#--companion-load=}"; shift ;;
        --cold-cache) cold=1; shift ;;
        --hidden) hidden=1; shift ;;
        *) spike_args+=("$1"); shift ;;
    esac
done
if ! [[ "$companion" =~ ^[0-9]+$ ]]; then
    echo "descentSpike.sh: --companion-load takes a thread count, not $companion" >&2
    exit 2
fi
smoke=0
for arg in ${spike_args[@]+"${spike_args[@]}"}; do
    if [[ "$arg" == --smoke ]]; then
        smoke=1
    fi
done

profile=""
groups=()
cleanup() {
    for group in ${groups[@]+"${groups[@]}"}; do
        kill -KILL -- "-$group" 2>/dev/null || true
    done
    if [[ -n "$profile" ]]; then
        rm -rf -- "$profile"
    fi
}
trap cleanup EXIT INT TERM

limit=$(( smoke == 1 ? 300 : 3600 ))
setsid timeout --kill-after=10 "$(( limit + 60 ))" \
    cargo run --release --quiet --manifest-path "$repo/Cargo.toml" -p hyperion-server -- \
    --num-workers 2 --port "$port" >/dev/null 2>&1 &
groups+=("$!")

for (( i = 0; i < companion; i++ )); do
    setsid timeout --kill-after=10 "$(( limit + 60 ))" bash -c 'while :; do :; done' &
    groups+=("$!")
done

mkdir -p "$cache_keep"
profile="$(mktemp -d "$runs/profile.XXXXXX")"
if [[ $cold -eq 0 ]]; then
    cp -a "$cache_keep/." "$profile/"
else
    rm -rf -- "${cache_keep:?}"/*
fi

env_run=()
if [[ $hidden -eq 1 ]]; then
    env_run+=(HYPERION_SPIKE_HIDDEN=1)
fi
# The device's memory before the client starts, which the results subtract (Design note 18): read
# here, since the GPU process may start before the main process could read it.
if command -v nvidia-smi >/dev/null 2>&1 && nvidia-smi -q -x >"$profile/nvidia-baseline.xml" 2>/dev/null; then
    env_run+=("HYPERION_SPIKE_NVIDIA_BASELINE=$profile/nvidia-baseline.xml")
fi
# As the `client` recipe does: on Wayland the client would relaunch itself through XWayland, and
# this script would take the first process's exit for the run's.
x11=()
if [[ "${XDG_SESSION_TYPE:-}" == wayland ]]; then
    x11=(--ozone-platform=x11)
fi
status=0
start=$(date +%s)
env ${env_run[@]+"${env_run[@]}"} setsid timeout --kill-after=10 "$limit" "$electron" \
    ${x11[@]+"${x11[@]}"} "$here/out/main/index.js" --descent-spike --port "$port" \
    ${spike_args[@]+"${spike_args[@]}"} \
    "--user-data-dir=$profile" &
client=$!
groups+=("$client")
wait "$client" || status=$?

# The shader caches Chromium and Dawn keep in the profile, for the next warm run.
for dir in "$profile"/*Cache; do
    if [[ -d "$dir" ]]; then
        rm -rf -- "${cache_keep:?}/$(basename "$dir")"
        cp -a "$dir" "$cache_keep/"
    fi
done
echo "descent spike: exit $status after $(( $(date +%s) - start )) s"
exit "$status"
