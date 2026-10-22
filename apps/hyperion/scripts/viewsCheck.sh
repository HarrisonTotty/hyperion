#!/usr/bin/env bash
# The several-views check's runner (plan R07, T20), called by `just views-check` after a build. It
# runs the built client with `--views-check`, which opens `VIEW` on the kept `PHASE TEST` scene and
# drives it through its phases (a photorealistic primary alone and with two wireframe instruments,
# a resize of the primary alone, a wireframe primary with two wireframe instruments, with a
# photorealistic and a wireframe instrument, and alone), then writes the results file and its
# summary. No server is needed.
#
#   viewsCheck.sh [--hidden] [--setting high|low] [--smoke] [--out <dir>]
#                 [--terrain [--handover] [--weathered]]
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
# --terrain --weathered, with or without --handover, serves the run TEST MOON weathered's field in
# TERRAIN TEST's place (plan R10, R10.T1.c; decision-r10-t1c-client-scope §3): the same moon on a
# 1 Gyr surface, its young craters ringed by haloes of fresh regolith. The files are named
# <date>-<machine>-<setting>-terrain-weathered and -handover-weathered, and the run block names
# the fixture. --weathered without --terrain is refused by the client; the script passes it on.
#
# Each launch gets a fresh `--user-data-dir`, removed afterwards; Electron runs in a process group
# of its own under `timeout`, killed when the launch ends or the script ends for any reason. The
# exit status is the client's: 0 the run is recorded (and, for the terrain run, passes), 1 it could
# not finish or a terrain check failed, 3 the watchdog.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$here/../.." && pwd)"
# The client's Electron; a test of this script gives a stand-in that records its arguments.
electron="${HYPERION_VIEWS_CHECK_ELECTRON:-$here/node_modules/.bin/electron}"

hidden=0
smoke=0
terrain=0
setting=""
check_args=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --hidden) hidden=1; shift ;;
        --smoke) smoke=1; check_args+=("$1"); shift ;;
        --terrain) terrain=1; check_args+=("$1"); shift ;;
        --handover) check_args+=("$1"); shift ;;
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

# The settings to launch: the one given, or for the terrain run with none, high then low (a
# setting holds for a launch); T20's run with none takes the client's default, high.
settings=()
if [[ -n "$setting" ]]; then
    settings=("$setting")
elif [[ $terrain -eq 1 ]]; then
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

env_run=()
if [[ $hidden -eq 1 ]]; then
    env_run+=(HYPERION_VIEWS_CHECK_HIDDEN=1)
fi
# As the `client` recipe does: on Wayland the client would relaunch itself through XWayland, and
# this script would take the first process's exit for the run's.
x11=()
if [[ "${XDG_SESSION_TYPE:-}" == wayland ]]; then
    x11=(--ozone-platform=x11)
fi
# Each launch's limit: T20's smoke 5 minutes and its full run 25; the terrain run's 10 and 40, above
# the client's own watchdogs (8 and 30 minutes).
if [[ $terrain -eq 1 ]]; then
    limit=$(( smoke == 1 ? 600 : 2400 ))
else
    limit=$(( smoke == 1 ? 300 : 1500 ))
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
    env "${env_run[@]}" setsid timeout --kill-after=10 "$limit" "$electron" "${x11[@]}" \
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
