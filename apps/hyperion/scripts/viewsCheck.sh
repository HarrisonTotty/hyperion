#!/usr/bin/env bash
# The several-views check's runner (plan R07, T20), called by `just views-check` after a build. It
# runs the built client with `--views-check`, which opens `VIEW` on the kept `PHASE TEST` scene and
# drives it through its phases (a photorealistic primary alone and with two wireframe instruments,
# a resize of the primary alone, a wireframe primary with two wireframe instruments, with a
# photorealistic and a wireframe instrument, and alone), then writes the results file and its
# summary. No server is needed.
#
#   viewsCheck.sh [--hidden] [--software] [--setting high|low] [--smoke] [--out <dir>]
#                 [--terrain [--handover [--universe <ID> --body <ID>]] [--weathered | --whole]
#                  | --atmosphere
#                  | --inspection --universe <ID> --body <ID>]
#
# --setting is VIEW's quality setting and the window's size: high 1920 x 1080, low 1280 x 720.
# --hidden never shows the window (offscreen rendering): no presentation times and no question,
# so such a run proves the harness, not T20's criterion. --smoke runs short windows, hidden, and
# writes under target/views-check/; a full run writes under docs/measurements/several-views/
# (--out elsewhere). A shown run asks once whether every view is the right way up.
#
# --terrain runs the terrain run instead (plan R10, R10.T16.d; decision-r10-t16-contracts §6), on
# the kept `TERRAIN TEST` scene: the photorealistic primary alone through the hand-over, the
# descent and the sunset, then with instruments (high: one photorealistic and one wireframe; low:
# one wireframe), then a device loss the harness forces mid-stream. It writes under
# target/views-check/terrain/ for a smoke and docs/measurements/terrain-check/ otherwise, and exits
# 1 when one of its checks fails. A setting lasts the whole launch, so --terrain with no --setting
# makes two launches, high then low, each with its own profile, watchdog and results file; the
# script's exit is the first non-zero one.
#
# --terrain --handover runs the terrain run's check-only hand-over variant in its place (plan R10,
# R10.T10.g; decision-r10-t16-leans §1): three looks at TERRAIN TEST's moon, its survey at 30° and
# 90° of phase and near the terminator, the camera closing on the moon through its hand-over and
# backing out through the hand-back; the frames either side of each switch are read back and each
# surveyed cell's brightness compared. Both settings by default, as --terrain; it writes
# <date>-<machine>-<setting>-handover.json and .md beside the terrain run's, and exits 1 unless
# every judged cell is within 1/3 stop. --handover without --terrain is refused by the client.
#
# --terrain --handover --universe <ID> --body <ID> makes the hand-over check's looks on that body as a
# running server serves it, in TERRAIN TEST's place (plan R10, R10.T15.b): start the server and place
# and survey the body first (`just server`, then `just place-ship --look-at <body> --survey orbital 50
# --json`, whose placement names the universe's ID), and give the client the server's --port if it
# is not the default. Each launch is given 20 minutes more, above the client's 15 for reading the
# served world and its looks' fields. The files are named <date>-<machine>-<setting>-handover-served.
#
# --terrain --weathered, with or without --handover, serves the run TEST MOON weathered's field in
# TERRAIN TEST's place (plan R10, R10.T1.c; decision-r10-t1c-client-scope §3): the same moon on a
# 1 Gyr surface, its young craters ringed by haloes of fresh regolith. The files are named
# <date>-<machine>-<setting>-terrain-weathered and -handover-weathered, and the run block names
# the fixture. --weathered without --terrain is refused by the client; the script passes it on.
#
# --terrain --whole serves the terrain run TEST MOON's whole body in TERRAIN TEST's place (plan R10,
# R10.T14.c, the owner's request): the same field with every cell surveyed. The script first runs
# the native tool, hyperion-surface's whole_terrain_test example (cargo, the testing feature), which
# writes it to target/views-check/terrain/whole.golden, about 4.5 MB and never committed, then gives
# the client `--whole <that file>`, which its main process reads. The files are named
# <date>-<machine>-<setting>-terrain-whole. The client refuses it beside --weathered or --handover.
#
# --atmosphere runs the atmosphere run instead (plan R08, R08.T10.b), on the kept `ATMOSPHERE TEST`
# scene: the primary photorealistic, its atmosphere media awaited, the moonlit night captured from
# the seat on TEST EARTH's night side once AUTO has settled, then a scripted free-camera flight, a
# climb towards TEST MOON first, to a few radii of each air fixture, TEST EARTH O3 SLAB, TEST MARS,
# TEST SATURN, the four under cloud or haze (TEST VENUS, TEST VENUS 5.80 MPa, TEST TITAN and
# TEST SATURN NH3 ICE DECK), TEST JUPITER at four ranges from 9.5e8 m to 1.5e8 m, and TEST EARTH,
# each captured (a smoke flies the climb and TEST EARTH O3 SLAB alone).
# Both settings by default, as --terrain; it writes
# <date>-<machine>-<setting>-atmosphere.json and .md
# under target/views-check/atmosphere/ for a smoke and docs/measurements/atmosphere-flight/
# otherwise, and exits 1 when a capture shows an ATMOSPHERE NOT AVAILABLE, ATMOSPHERE DATA INVALID
# or graphics fault line or was not drawn photorealistic, when the engine raised a fault or the GPU
# process exited over the run (plan R08, R08.T10.g), or when no capture was taken. --atmosphere
# with --terrain is refused by the client.
#
# --inspection --universe <ID> --body <ID> runs the inspection run instead (the owner's request,
# 2026-11-01): on a running server, the universe opened on GALAXY, the body's system on SYSTEM, the
# body selected, FOCUS BODY and VIEW BODY pressed, and VIEW's photorealistic body inspection of it,
# on AUTO and its ground surveyed, captured once AUTO has settled, as
# between-inspection-<system>-<index>.png, then with MAXIMIZE VIEW on as
# between-inspection-<system>-<index>-maximized.png, then with GRID OFF as
# between-inspection-<system>-<index>-grid-off.png, and, over surveyed ground, closed in to 5% of
# its radius above it as between-inspection-<system>-<index>-close.png, under
# target/views-check/inspection/. One setting, high unless given, and one body a launch; it exits 1
# when a step does not take effect, VIEW BODY is held back, or a fault line stands on a capture.
#
# --software runs any of these checks hidden on SwiftShader, Chromium's CPU Vulkan, with no GPU and
# no display, as a pod or a CI machine has (plan R01, R01.T16): Linux only (exit 2 elsewhere), and
# implying --hidden. It is the check-only software mode, a test-tool exception to R01 Design note 8
# (the owner's ruling, 2026-10-29): the client draws both styles on the fallback adapter, its
# GRAPHICS panel reads SOFTWARE and the record's launch mode `software`. Electron runs in
# test-render's environment (DISPLAY and WAYLAND_DISPLAY unset, the session's D-Bus out of reach),
# with XDG_SESSION_TYPE unset too, and with test-render's adapter and headless switches,
# --hyperion-graphics-software, --ozone-override-screen-size=1920,1080 and --disable-gpu-watchdog,
# never --ozone-platform=x11. The research lane's prototype found both needed (AMD EPYC 7571,
# 4 vCPU, 2026-10-29): headless Ozone's screen held the window at the client's 1024 x 640 minimum
# whatever the setting, and Chromium's GPU watchdog ended SwiftShader's frames of 18.7 and 29.8 s
# with exit_code=512, where with it off the longest frame measured took 22.7 s. Every wait of the
# run, the client's watchdog and each launch's limit here are five times as long. Its results go
# under the smoke's target/views-check/ directories, a full run's included (never
# docs/measurements/), its frame times are SwiftShader's, and a GPU-process exit or a primary not
# drawn in the style its phase asks for fails the several-views check too. The client refuses the
# switch without --views-check, with --out and in a packaged build.
#
# Each launch gets a fresh `--user-data-dir`, removed afterwards; Electron runs in a process group
# of its own under `timeout`, killed when the launch ends or the script ends for any reason. The
# exit status is the client's: 0 the run is recorded (and, for the terrain and atmosphere runs,
# passes), 1 it could not finish or a terrain check or an atmosphere check failed, 3 the watchdog.
# A GPU-process crash loop ends a launch with 1, never relaunching into safe mode (R08.T10.g).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$here/../.." && pwd)"
# The client's Electron; a test of this script gives a stand-in that records its arguments.
electron="${HYPERION_VIEWS_CHECK_ELECTRON:-$here/node_modules/.bin/electron}"

hidden=0
software=0
smoke=0
terrain=0
served=0
atmosphere=0
inspection=0
whole=0
setting=""
check_args=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --hidden) hidden=1; shift ;;
        --software) software=1; hidden=1; shift ;;
        --smoke) smoke=1; check_args+=("$1"); shift ;;
        --terrain) terrain=1; check_args+=("$1"); shift ;;
        --handover) check_args+=("$1"); shift ;;
        --atmosphere) atmosphere=1; check_args+=("$1"); shift ;;
        --inspection) inspection=1; check_args+=("$1"); shift ;;
        --whole) whole=1; shift ;;
        --universe) served=1; check_args+=("$1"); shift ;;
        --setting)
            if [[ $# -lt 2 ]]; then
                echo "views check: --setting needs high or low" >&2
                exit 2
            fi
            setting="$2"; shift 2 ;;
        --setting=*) setting="${1#--setting=}"; shift ;;
        *) check_args+=("$1"); shift ;;
    esac
done

# The software mode's switches are the smoke harness's, which only Linux runs headless.
if [[ $software -eq 1 && "$(uname -s)" != Linux ]]; then
    echo "views check: --software runs on Linux only" >&2
    exit 2
fi

# The whole body's field, written by the native tool before any launch reads it; a test of this
# script gives a stand-in for cargo.
if [[ $whole -eq 1 ]]; then
    whole_field="$repo/target/views-check/terrain/whole.golden"
    echo "views check: writing the whole body's field, $whole_field"
    if ! (cd "$repo" && "${HYPERION_VIEWS_CHECK_CARGO:-cargo}" run -q -p hyperion-surface --release \
        --features testing --example whole_terrain_test -- "$whole_field"); then
        echo "views check: the whole body's field was not written" >&2
        exit 2
    fi
    check_args+=(--whole "$whole_field")
fi

# The settings to launch: the one given, or for the terrain or the atmosphere run with none, high
# then low (a setting holds for a launch); T20's run with none takes the client's default, high.
settings=()
if [[ -n "$setting" ]]; then
    settings=("$setting")
elif [[ $terrain -eq 1 || $atmosphere -eq 1 ]]; then
    settings=(high low)
else
    settings=("")
fi

profile=""
group=""
cleanup() {
    if [[ -n "$group" ]]; then
        kill -KILL -- "-$group" 2>/dev/null || true
        group=""
    fi
    if [[ -n "$profile" ]]; then
        rm -rf -- "$profile"
        profile=""
    fi
}
# An interrupt or a termination ends the script, after the launch it stops: bash's `wait` returns
# on a trapped signal, so a trap that only cleaned up would go on to the next launch.
trap cleanup EXIT
trap 'cleanup; exit 130' INT
trap 'cleanup; exit 143' TERM

# testRender.sh's adapter and headless switches, word for word (viewsCheckScript.test.ts holds the
# two lists equal), for --software alone.
adapter_switches=(--enable-unsafe-webgpu --use-webgpu-adapter=swiftshader)
headless_switches=(--ozone-platform=headless --use-angle=swiftshader --enable-features=Vulkan
    --use-vulkan=swiftshader)

env_run=()
switches=()
if [[ $software -eq 1 ]]; then
    # testRender.sh's environment, no display and the session's D-Bus out of reach, and no
    # XDG_SESSION_TYPE either.
    env_run+=(-u DISPLAY -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE
        DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent)
    switches=("${adapter_switches[@]}" "${headless_switches[@]}" --hyperion-graphics-software
        --ozone-override-screen-size=1920,1080 --disable-gpu-watchdog)
elif [[ "${XDG_SESSION_TYPE:-}" == wayland ]]; then
    # As the `client` recipe does: on Wayland the client would relaunch itself through XWayland, and
    # this script would take the first process's exit for the run's.
    switches=(--ozone-platform=x11)
fi
if [[ $hidden -eq 1 ]]; then
    env_run+=(HYPERION_VIEWS_CHECK_HIDDEN=1)
fi
# Each launch's limit: T20's smoke 5 minutes and its full run 25; the terrain run's 10 and 40, above
# the client's own watchdogs (8 and 30 minutes); the atmosphere run's 10 and 20, above its (8 and
# 15); the inspection run's 10 and 17, above its (8 and 15). Given two, the client refuses the
# launch at once, so the first run's limit here stands.
if [[ $terrain -eq 1 ]]; then
    limit=$(( (smoke == 1 ? 600 : 2400) + (served == 1 ? 1200 : 0) ))
elif [[ $atmosphere -eq 1 ]]; then
    limit=$(( smoke == 1 ? 600 : 1200 ))
elif [[ $inspection -eq 1 ]]; then
    limit=$(( smoke == 1 ? 600 : 1020 ))
else
    limit=$(( smoke == 1 ? 300 : 1500 ))
fi
# The software mode's, five times as long, as the client's watchdog and waits are (main's ruling,
# 2026-10-29: fixed, not a variable).
software_factor=5
if [[ $software -eq 1 ]]; then
    limit=$(( limit * software_factor ))
    echo "views check: software mode, on SwiftShader; each launch's limit $limit s"
fi
cd "$repo"
result=0
for each in "${settings[@]}"; do
    setting_args=()
    label="${each:-the default}"
    if [[ -n "$each" ]]; then
        setting_args=(--setting "$each")
    fi
    profile="$(mktemp -d "${TMPDIR:-/tmp}/hyperion-views-check.XXXXXX")"
    status=0
    start=$(date +%s)
    echo "views check: launching, setting $label"
    env "${env_run[@]}" setsid timeout --kill-after=10 "$limit" "$electron" "${switches[@]}" \
        "$here/out/main/index.js" --views-check "${setting_args[@]}" "${check_args[@]}" \
        "--user-data-dir=$profile" &
    group=$!
    wait "$group" || status=$?
    kill -KILL -- "-$group" 2>/dev/null || true
    group=""
    rm -rf -- "$profile"
    profile=""
    echo "views check: setting $label, exit $status after $(( $(date +%s) - start )) s"
    if [[ $result -eq 0 && $status -ne 0 ]]; then
        result=$status
    fi
done
echo "views check: exit $result"
exit "$result"
