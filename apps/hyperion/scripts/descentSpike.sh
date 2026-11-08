#!/usr/bin/env bash
# The descent spike's runner (R05.T13.c, Design notes 18 and 20), called by `just descent-spike`
# after a build. It starts a local hyperion-server with its pool capped at two workers, runs the
# built client with `--descent-spike` and the spike's options, and stops everything it started.
#
#   descentSpike.sh [--companion-load <threads>] [--cold-cache] [--hidden] [--software]
#                   [--universe <NAME|ID>] [--body <BODY_ID>] [--survey <SOURCE> <RESOLUTION_M>]
#                   [--distance <AU>] [spike options...]
#
# Spike options (the client's, `src/main/cli.ts`): --setting high|low, --seed <u64>, --smoke,
# --out <dir>, --workers <n>, --vertex-path baked-offsets|face-differences, --normals double|mesh,
# --ridged on|off, --dawn-safety on|off, --capture <dir>, --trace-profile on|off (V8's CPU profiler
# and `gpu` in the trace, off by default: a profiled run is a diagnostic, never judged; R05.T14.e),
# --world test-planet|terrain-test|served and --style photorealistic|wireframe (R10.T14.c,
# R10.T15.b), --sizes on|off (R10.T14.b: a generated world's cache sizes measured, off by
# default; refused on the test planet), and --horizon on|off (R10.T15.b: the lit patches' horizon
# map, on by default; off is the A/B whose run beside an ordinary --sizes on run gives the map's
# share of the height workers' bake time; refused on the test planet and with --style wireframe),
# and --geometry <field>=<value>[,...] (plan R14, Design note 17: how a generated world's terrain
# draws its geometry, over the setting's; clusterPath=false is the way back, as before plan R14).
#
# --world test-planet, the default, flies R05's test planet. --world terrain-test flies TERRAIN
# TEST's moon over a generated world's terrain: its field is served in the page by TERRAIN TEST's
# fixture through VIEW's terrain session, not by the server, and its main view is VIEW's own frame
# drawer in --style's style (photorealistic by default). The server this script starts still runs
# on either world, as Design note 20's stand-in for the CPU a server takes. The terrain world
# measures its descent through the session before it flies (about 1,600 bakes, some 40 s on three
# workers), so its run is given 13 minutes more: the field's 2-minute hold and one site's 10 minutes
# of bakes at their limits, and a minute's margin. The client refuses --style on the test planet,
# and --ridged on, --vertex-path and --normals on a generated world.
#
# --world served flies a generated body this script's server serves (plan R10, R10.T15.b). The
# script builds the server and waits for it, then places the ship with `just place-ship` (its
# `--json` placement): at --body (place-ship's default without it, since it surveys: the first
# planet with a solid surface of the nearest system with one, found at the server's version) in
# --universe (place-ship's dev universe without it), at --distance au if given (half the body's
# Hill radius at pericentre without it), surveying every cell of the body's coarse field
# (--survey, orbital at 50 m by default), and hands the client the placement's universe and
# body as its own --universe and --body. Its run is given 21 minutes more: the field's 10-minute
# hold, since the server computes the body's coarse field before its first answer, one site's 10
# minutes of bakes, and a minute. Behind the serve gate (`GENERATED_SURFACES_FROM`, generator
# version 25) the server answers every generated body's field not modelled, and the spike says so
# and ends failed.
#
# A full run writes the results of its segments finished so far as each segment ends, as
# <name>-partial.json and .md beside where its results go, so that a run stopped part-way keeps
# them; its results, written under the same name as it ends, take their place (R14's Mac
# measurements). Their trace is missing, read only as the run ends, so their frame figures are
# requestAnimationFrame's, and they hold no memory series and no geometry or sizes block.
#
# --companion-load <threads> runs that many busy threads beside the run, standing in for the
# server's arrival work (Design note 20). --cold-cache starts with an empty Chromium GPU shader
# cache; otherwise the cache kept from the last run (in target/descent-spike/gpu-cache) is copied
# into the run's profile and kept again after it, so that cold and warm caches are run apart.
# --hidden never shows the window (offscreen rendering), as --smoke never does. Such a run on Linux
# with no display (DISPLAY and WAYLAND_DISPLAY unset or empty), a pod's or a CI machine's, takes
# headless Ozone and its screen size, --ozone-platform=headless and
# --ozone-override-screen-size=1920,1080, the views check's software mode's (`viewsCheck.sh`), so
# that Electron starts with no X server; with a GPU it draws there, and without one --software gives
# it SwiftShader. A run with a display is unchanged.
#
# --software runs the spike hidden on SwiftShader, Chromium's CPU Vulkan, with no GPU and no display,
# as a pod has (the check-only software mode, plan R01, R01.T16; main's ruling of 2026-11-01 for the
# spike, on R10.T15.b.2's runs): Linux only (exit 2 elsewhere), implying --hidden. It is
# `viewsCheck.sh --software`'s mode: Electron runs in test-render's environment (DISPLAY,
# WAYLAND_DISPLAY and XDG_SESSION_TYPE unset, the session's D-Bus out of reach) with test-render's
# adapter and headless switches, --hyperion-graphics-software, --ozone-override-screen-size=1920,1080
# and --disable-gpu-watchdog, never --ozone-platform=x11; the client then forces no Vulkan switch,
# draws both styles on the fallback adapter and records the launch mode `software`. Its frame times
# are SwiftShader's and its figures never a GPU's: the results go under target/descent-spike/results/
# (the client refuses --out with the switch), and its shader caches are kept in
# target/descent-spike/gpu-cache-software, apart from a GPU run's. The client's watchdog and this
# script's limit are five times as long. A GPU launch, without --software, is unchanged.
#
# Each run gets a fresh `--user-data-dir` under target/descent-spike/, on disk and never under
# TMPDIR, removed afterwards: the trace's window files wait there until the run ends, about 1 GB
# for a timed descent and 2 GB for a profiled one, which a RAM-backed /tmp cannot hold (decision-r05-trace-windows.md). Electron's
# TMPDIR is a fresh directory there too, removed afterwards, since Chromium spools each window's
# protobuf stream to a temporary file before the client reads it over CDP (R05.T14.i). The server,
# the companion load and Electron run in process groups of their own under `timeout`, each group
# killed when the script ends for any reason. The exit status is the client's: 0 pass, 1 fail, 3
# the watchdog. `setsid` and `timeout` are util-linux's and coreutils', or on macOS
# `tools/portable`'s stand-ins, which the script puts on PATH where they are missing. It runs on bash 3.2 (macOS's): an array that may be empty is expanded
# as `${a[@]+"${a[@]}"}`.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$here/../.." && pwd)"
# macOS has neither setsid(1) nor timeout(1). The justfile puts `tools/portable`'s stand-ins on
# PATH, after the system's own; a call from outside it (the Mac batch's, `macProfile.mjs`) takes
# them here, the same way, so that each launch still starts a process group of its own.
for tool in setsid timeout; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        PATH="$PATH:$repo/tools/portable"
        break
    fi
done
# The client's Electron; a test of this script gives a stand-in that records its arguments.
electron="${HYPERION_SPIKE_ELECTRON:-$here/node_modules/.bin/electron}"
# Where the runs' profiles and kept shader caches go; a test of this script gives its own.
runs="${HYPERION_SPIKE_RUNS:-$repo/target/descent-spike}"
cache_keep="$runs/gpu-cache"
port="${HYPERION_SPIKE_PORT:-7879}"

companion=0
cold=0
hidden=0
software=0
universe=""
body=""
survey_source=orbital
survey_resolution=50
distance=""
spike_args=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --companion-load) companion="$2"; shift 2 ;;
        --companion-load=*) companion="${1#--companion-load=}"; shift ;;
        --cold-cache) cold=1; shift ;;
        --hidden) hidden=1; shift ;;
        --software) software=1; hidden=1; shift ;;
        --universe) universe="$2"; shift 2 ;;
        --body) body="$2"; shift 2 ;;
        --survey) survey_source="$2"; survey_resolution="$3"; shift 3 ;;
        --distance) distance="$2"; shift 2 ;;
        *) spike_args+=("$1"); shift ;;
    esac
done
# The software mode's switches are the smoke harness's, which only Linux runs headless.
if [[ $software -eq 1 && "$(uname -s)" != Linux ]]; then
    echo "descentSpike.sh: --software runs on Linux only" >&2
    exit 2
fi
if [[ $software -eq 1 ]]; then
    # SwiftShader's shader caches are kept apart, so that they never warm a GPU run's.
    cache_keep="$runs/gpu-cache-software"
fi
if ! [[ "$companion" =~ ^[0-9]+$ ]]; then
    echo "descentSpike.sh: --companion-load takes a thread count, not $companion" >&2
    exit 2
fi
smoke=0
terrain=0
served=0
previous=""
for arg in ${spike_args[@]+"${spike_args[@]}"}; do
    if [[ "$arg" == --smoke ]]; then
        smoke=1
    fi
    if [[ "$arg" == --world=terrain-test || ( "$previous" == --world && "$arg" == terrain-test ) ]]; then
        terrain=1
    fi
    if [[ "$arg" == --world=served || ( "$previous" == --world && "$arg" == served ) ]]; then
        served=1
    fi
    previous="$arg"
done
if [[ $served -eq 0 && ( -n "$universe" || -n "$body" || -n "$distance" ) ]]; then
    echo "descentSpike.sh: --universe, --body and --distance are options of --world served" >&2
    exit 2
fi

profile=""
spool=""
groups=()
cleanup() {
    for group in ${groups[@]+"${groups[@]}"}; do
        kill -KILL -- "-$group" 2>/dev/null || true
    done
    if [[ -n "$profile" ]]; then
        rm -rf -- "$profile"
    fi
    if [[ -n "$spool" ]]; then
        rm -rf -- "$spool"
    fi
}
trap cleanup EXIT INT TERM

# The client's watchdog ends a run first (status 3): 3 minutes for a smoke and 45 for a descent,
# each 13 more on the terrain world and 21 more on a served one (`spikeWatchdogMs`), so that this
# limit stays above it.
limit=$(( (smoke == 1 ? 300 : 3600) + (terrain == 1 ? 780 : 0) + (served == 1 ? 1260 : 0) ))
# The software mode's, five times as long, as the client's watchdog is (`SOFTWARE_WAIT_FACTOR`).
software_factor=5
if [[ $software -eq 1 ]]; then
    limit=$(( limit * software_factor ))
    echo "descent spike: software mode, on SwiftShader; the run's limit $limit s"
fi
if [[ $served -eq 1 ]]; then
    # Built first, so that the server is up within seconds of starting and the ship can be placed.
    cargo build --release --quiet --manifest-path "$repo/Cargo.toml" -p hyperion-server
fi
setsid timeout --kill-after=10 "$(( limit + 60 ))" \
    cargo run --release --quiet --manifest-path "$repo/Cargo.toml" -p hyperion-server -- \
    --num-workers 2 --port "$port" >/dev/null 2>&1 &
groups+=("$!")

if [[ $served -eq 1 ]]; then
    place=(--port "$port" --json --survey "$survey_source" "$survey_resolution")
    if [[ -n "$universe" ]]; then
        place+=(--universe "$universe")
    fi
    if [[ -n "$body" ]]; then
        place+=(--look-at "$body")
    fi
    if [[ -n "$distance" ]]; then
        place+=(--distance "$distance")
    fi
    # place-ship fails at once until the server listens, a few seconds after it starts.
    placement=""
    for (( try = 0; try < 60; try++ )); do
        if placement="$(cd "$here" && node scripts/placeShip.mjs "${place[@]}" 2>/dev/null)"; then
            break
        fi
        placement=""
        sleep 1
    done
    if [[ -z "$placement" ]]; then
        echo "descentSpike.sh: place-ship could not place the ship on the server at port $port:" >&2
        (cd "$here" && node scripts/placeShip.mjs "${place[@]}") >&2 || true
        exit 2
    fi
    ids="$(node -e 'const p = JSON.parse(process.argv[1]); process.stdout.write(`${p.universe.id} ${p.body}`);' "$placement")"
    echo "descent spike: served world body ${ids#* } in universe ${ids% *}, $survey_source survey at $survey_resolution m"
    spike_args+=(--universe "${ids% *}" --body "${ids#* }")
fi

for (( i = 0; i < companion; i++ )); do
    setsid timeout --kill-after=10 "$(( limit + 60 ))" bash -c 'while :; do :; done' &
    groups+=("$!")
done

mkdir -p "$cache_keep"
profile="$(mktemp -d "$runs/profile.XXXXXX")"
spool="$(mktemp -d "$runs/tmp.XXXXXX")"
if [[ $cold -eq 0 ]]; then
    cp -a "$cache_keep/." "$profile/"
else
    rm -rf -- "${cache_keep:?}"/*
fi

env_run=("TMPDIR=$spool")
if [[ $hidden -eq 1 ]]; then
    env_run+=(HYPERION_SPIKE_HIDDEN=1)
fi
# The device's memory before the client starts, which the results subtract (Design note 18): read
# here, since the GPU process may start before the main process could read it.
if command -v nvidia-smi >/dev/null 2>&1 && nvidia-smi -q -x >"$profile/nvidia-baseline.xml" 2>/dev/null; then
    env_run+=("HYPERION_SPIKE_NVIDIA_BASELINE=$profile/nvidia-baseline.xml")
fi
# testRender.sh's adapter and headless switches, word for word (descentSpikeScript.test.ts holds
# the two lists equal), for --software alone.
adapter_switches=(--enable-unsafe-webgpu --use-webgpu-adapter=swiftshader)
headless_switches=(--ozone-platform=headless --use-angle=swiftshader --enable-features=Vulkan
    --use-vulkan=swiftshader)
env_unset=()
switches=()
if [[ $software -eq 1 ]]; then
    # testRender.sh's environment, no display and the session's D-Bus out of reach, and no
    # XDG_SESSION_TYPE either; `env` takes its -u options before the assignments.
    env_unset=(-u DISPLAY -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE)
    env_run+=(DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent)
    switches=("${adapter_switches[@]}" "${headless_switches[@]}" --hyperion-graphics-software
        --ozone-override-screen-size=1920,1080 --disable-gpu-watchdog)
elif [[ ( $hidden -eq 1 || $smoke -eq 1 ) && -z "${DISPLAY:-}" && -z "${WAYLAND_DISPLAY:-}" \
    && "$(uname -s)" == Linux ]]; then
    # No display to draw on and none needed: headless Ozone, which would otherwise hold the window
    # at the client's 1024 x 640 minimum, at the software mode's screen size.
    switches=(--ozone-platform=headless --ozone-override-screen-size=1920,1080)
elif [[ "${XDG_SESSION_TYPE:-}" == wayland ]]; then
    # As the `client` recipe does: on Wayland the client would relaunch itself through XWayland,
    # and this script would take the first process's exit for the run's.
    switches=(--ozone-platform=x11)
fi
status=0
start=$(date +%s)
env ${env_unset[@]+"${env_unset[@]}"} ${env_run[@]+"${env_run[@]}"} \
    setsid timeout --kill-after=10 "$limit" "$electron" \
    ${switches[@]+"${switches[@]}"} "$here/out/main/index.js" --descent-spike --port "$port" \
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
