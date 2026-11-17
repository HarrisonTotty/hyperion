#!/usr/bin/env bash
# The perf runner's launch (plan R12, R12.T4.b; Design notes 7, 13 and 14), called by
# `just perf-run` after it builds the client and the release server. It states the run's build in
# the environment, the commit (`git rev-parse HEAD`), whether the tracked tree differs from it, and
# hyperion-base's GENERATOR_VERSION, which a perf record must carry, with the server binary the
# runner starts and restarts, and runs the built client with `--perf-run <scene> --perf-reps <n>`.
#
#   perfRun.sh <scene> <reps> [--hidden] [--software] [client options...]
#
# Client options (`src/main/cli.ts`): --setting high|low, --perf-out <file> (by default
# target/perf-run/runs.v1.jsonl, whose records `just perf-record` adds to the store),
# --perf-control (the scene's control run, without the timing switch) and --force (run though a
# precondition fails, the record marked forced); --address and --port name a station scene's remote
# server, which its operator restarts.
#
# --hidden never shows the window (offscreen rendering; its frames paced by Chromium's timer, so the
# record's display is `virtual`, which the tables leave out). On Linux with no display (DISPLAY and
# WAYLAND_DISPLAY unset or empty) it takes headless Ozone at 1920 x 1080, as the descent spike's
# does. --software runs hidden on SwiftShader with no GPU and no display, a pod's (plan R01,
# R01.T16): Linux only (exit 2 elsewhere), in test-render's environment and switches, with the
# client's watchdog and this script's limit five times as long. Its figures are SwiftShader's,
# never a GPU's; its record goes under target/ alone.
#
# The device's NVIDIA memory is read before the client starts, as the descent spike's is, since the
# GPU process may start before the main process could read it. Each run gets a fresh
# `--user-data-dir` under target/perf-run/ (the server's data directories and the trace's windows go
# there), removed afterwards, and Electron's TMPDIR beside it. Electron runs in a process group of
# its own under `timeout`, killed when the script ends for any reason; the server, the runner's
# child, stops when its standard input closes. The exit status is the client's: 0 a record
# appended, 1 a failed run, 2 a refusal (a precondition without --force, or a usage error).
# It runs on bash 3.2 (macOS's): an array that may be empty is expanded as `${a[@]+"${a[@]}"}`.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$here/../.." && pwd)"
# macOS has neither setsid(1) nor timeout(1): `tools/portable`'s stand-ins, as descentSpike.sh.
for tool in setsid timeout; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        PATH="$PATH:$repo/tools/portable"
        break
    fi
done
# The client's Electron; a test of this script gives a stand-in that records its arguments.
electron="${HYPERION_PERF_ELECTRON:-$here/node_modules/.bin/electron}"
# Where the runs' profiles go; a test of this script gives its own.
runs="${HYPERION_PERF_RUNS:-$repo/target/perf-run}"
server="${HYPERION_PERF_SERVER_BINARY:-$repo/target/release/hyperion-server}"

if [[ $# -lt 2 ]]; then
    echo "usage: perfRun.sh <scene> <reps> [--hidden] [--software] [client options...]" >&2
    exit 2
fi
scene="$1"
reps="$2"
shift 2
if ! [[ "$reps" =~ ^[0-9]+$ ]]; then
    echo "perfRun.sh: the repetitions are a count, not $reps" >&2
    exit 2
fi
# One repetition's span, s (Design note 13): R05's whole descent, the approach, the arrival's
# allowance and the sweep; the scenes' own are `view/perf/scenes.ts`'.
case "$scene" in
    cockpit-descent | station-wireframe-descent) span=1230 ;;
    ringed-giant-approach) span=300 ;;
    arrival-memory) span=1200 ;;
    nuclear-disc-sky) span=120 ;;
    *)
        echo "perfRun.sh: $scene is not a scene of the catalogue" >&2
        exit 2
        ;;
esac

hidden=0
software=0
client_args=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --hidden) hidden=1; shift ;;
        --software) software=1; hidden=1; shift ;;
        *) client_args+=("$1"); shift ;;
    esac
done
if [[ $software -eq 1 && "$(uname -s)" != Linux ]]; then
    echo "perfRun.sh: --software runs on Linux only" >&2
    exit 2
fi

commit="$(git -C "$repo" rev-parse HEAD)"
dirty=0
if [[ -n "$(git -C "$repo" status --porcelain --untracked-files=no)" ]]; then
    dirty=1
fi
version="$(sed -n 's/.*GeneratorVersion::new(\([0-9][0-9]*\)).*/\1/p' \
    "$repo/crates/hyperion-base/src/version.rs" | head -n 1)"
if [[ -z "$version" ]]; then
    echo "perfRun.sh: hyperion-base's GENERATOR_VERSION could not be read" >&2
    exit 2
fi

profile=""
spool=""
group=""
cleanup() {
    if [[ -n "$group" ]]; then
        kill -KILL -- "-$group" 2>/dev/null || true
    fi
    if [[ -n "$profile" ]]; then
        rm -rf -- "$profile"
    fi
    if [[ -n "$spool" ]]; then
        rm -rf -- "$spool"
    fi
}
trap cleanup EXIT INT TERM

# The client's watchdog ends a hung run first: this limit is the warm-up's ten minutes, each
# repetition's span twice with five minutes' room, and ten minutes for the start and the record.
limit=$(( 600 + 600 + reps * (2 * span + 300) ))
if [[ $software -eq 1 ]]; then
    limit=$(( limit * 5 ))
    echo "perf run: software mode, on SwiftShader; the run's limit $limit s"
fi

mkdir -p "$runs"
profile="$(mktemp -d "$runs/profile.XXXXXX")"
spool="$(mktemp -d "$runs/tmp.XXXXXX")"
env_run=(
    "TMPDIR=$spool"
    "HYPERION_PERF_COMMIT=$commit"
    "HYPERION_PERF_DIRTY=$dirty"
    "HYPERION_PERF_GENERATOR_VERSION=$version"
    "HYPERION_PERF_SERVER=$server"
)
if [[ $hidden -eq 1 ]]; then
    env_run+=(HYPERION_PERF_HIDDEN=1)
fi
if command -v nvidia-smi >/dev/null 2>&1 && nvidia-smi -q -x >"$profile/nvidia-baseline.xml" 2>/dev/null; then
    env_run+=("HYPERION_PERF_NVIDIA_BASELINE=$profile/nvidia-baseline.xml")
fi
# testRender.sh's adapter and headless switches, word for word, for --software alone, as
# descentSpike.sh takes them.
adapter_switches=(--enable-unsafe-webgpu --use-webgpu-adapter=swiftshader)
headless_switches=(--ozone-platform=headless --use-angle=swiftshader --enable-features=Vulkan
    --use-vulkan=swiftshader)
env_unset=()
switches=()
if [[ $software -eq 1 ]]; then
    env_unset=(-u DISPLAY -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE)
    env_run+=(DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent)
    switches=("${adapter_switches[@]}" "${headless_switches[@]}" --hyperion-graphics-software
        --ozone-override-screen-size=1920,1080 --disable-gpu-watchdog)
elif [[ $hidden -eq 1 && -z "${DISPLAY:-}" && -z "${WAYLAND_DISPLAY:-}" \
    && "$(uname -s)" == Linux ]]; then
    switches=(--ozone-platform=headless --ozone-override-screen-size=1920,1080)
elif [[ "${XDG_SESSION_TYPE:-}" == wayland ]]; then
    # As the `client` recipe does: on Wayland the client would relaunch itself through XWayland,
    # and this script would take the first process's exit for the run's.
    switches=(--ozone-platform=x11)
fi
status=0
start=$(date +%s)
env ${env_unset[@]+"${env_unset[@]}"} "${env_run[@]}" \
    setsid timeout --kill-after=10 "$limit" "$electron" \
    ${switches[@]+"${switches[@]}"} "$here/out/main/index.js" \
    --perf-run "$scene" --perf-reps "$reps" ${client_args[@]+"${client_args[@]}"} \
    "--user-data-dir=$profile" &
group=$!
wait "$group" || status=$?
echo "perf run: exit $status after $(( $(date +%s) - start )) s"
exit "$status"
