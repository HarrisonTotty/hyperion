#!/usr/bin/env bash
# The headless smoke harness's runner (R01.T9, Design note 17), called by `just test-render` under
# the heavy-test lock after a build. It runs the built harness once per variant on SwiftShader with
# no display, every variant at once, and exits with the code of the first run, in variant order,
# that does not exit 0: 1 a property failed or a request went out, 2 a setup error (no adapter), 3
# the watchdog.
#
#   testRender.sh [--variant=NAME]... [--fixture=broken-wgsl|external-fetch] [--drop-adapter-switches]
#                 [--captures=DIR]
#
# --captures=DIR also renders the capture frames (R05.T12.c's atmosphere comparison, R05.T11.c's
# terrain, R05.T13.b's spike, R07.T9's occultation, R07.T10.c's eclipse scene) and saves them in
# DIR as PNGs, each named for its variant.
#
# --drop-adapter-switches removes only `--enable-unsafe-webgpu` and `--use-webgpu-adapter`, keeping
# the headless Ozone, ANGLE and Vulkan switches, so that Electron starts and finds no adapter.
#
# A variant keeps 1.4 to 2.3 cores busy, so the variants run side by side rather than one after
# another (perf-ts.md, Q2; 141 s for both against about 230 s, measured 2026-10-10). Each run's
# output goes to a log of its own, and the logs are printed whole in variant order once every run
# has ended, each with the time from the start to its own end; the exit code is the one the runs
# gave one after another, except that a later run is no longer skipped after a failure.
#
# Each run gets a fresh `--user-data-dir`, removed afterwards, so that a run killed part-way leaves
# no profile, lock or cache for the next, and none is shared between runs; it does not reach the
# session's D-Bus. Electron runs in a process group of its own under a `timeout`, and every group
# is killed when the script ends for any reason. `setsid` and `timeout` are util-linux's and
# coreutils', or on macOS the justfile's `tools/portable` stand-ins. It runs on bash 3.2 (macOS's):
# an array that may be empty is expanded as `${a[@]+"${a[@]}"}`, and there is no `wait -n`.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
electron="$here/node_modules/.bin/electron"

adapter_switches=(--enable-unsafe-webgpu --use-webgpu-adapter=swiftshader)
headless_switches=(--ozone-platform=headless --use-angle=swiftshader --enable-features=Vulkan --use-vulkan=swiftshader)

variants=()
fixture=none
captures=()
for arg in "$@"; do
    case "$arg" in
        --variant=*) variants+=("${arg#--variant=}") ;;
        --fixture=*) fixture="${arg#--fixture=}" ;;
        --drop-adapter-switches) adapter_switches=() ;;
        --captures=*) captures=("--smoke-captures=${arg#--captures=}") ;;
        *) echo "testRender.sh: unknown argument $arg" >&2; exit 2 ;;
    esac
done
if [[ ${#variants[@]} -eq 0 ]]; then
    variants=(default no-subgroups)
fi

# Milliseconds since the epoch: GNU date's `%3N`, or Perl's clock where date has no `%N` (macOS).
now_ms() {
    local now
    now="$(date +%s%3N 2>/dev/null || true)"
    if [[ -z "$now" || "$now" == *[!0-9]* ]]; then
        now="$(perl -MTime::HiRes=time -e 'printf "%d\n", time * 1000')"
    fi
    echo "$now"
}

# Per run, by its index in `variants`: its profile and its process group until it has ended, then
# its exit code and the time it ended. An ended run's group and profile are gone and are cleared,
# so that the cleanup never kills a group that has since taken the same ID.
profiles=()
groups=()
statuses=()
ended=()
logs=""
printed=0
# A run cut short (an interrupt, or an outer timeout) still shows what its runs had written, on
# stderr, before their logs go.
cleanup() {
    local each i
    for each in ${groups[@]+"${groups[@]}"}; do
        if [[ -n "$each" ]]; then
            kill -KILL -- "-$each" 2>/dev/null || true
        fi
    done
    for each in ${profiles[@]+"${profiles[@]}"}; do
        if [[ -n "$each" ]]; then
            rm -rf -- "$each"
        fi
    done
    if [[ -n "$logs" ]]; then
        if [[ $printed -eq 0 ]]; then
            for i in "${!variants[@]}"; do
                if [[ -f "$logs/$i.log" ]]; then
                    echo "== smoke run cut short: variant ${variants[i]}, fixture $fixture" >&2
                    cat "$logs/$i.log" >&2 || true
                fi
            done
        fi
        rm -rf -- "$logs"
    fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

logs="$(mktemp -d "${TMPDIR:-/tmp}/hyperion-smoke-logs.XXXXXX")"
start=$(now_ms)
for i in "${!variants[@]}"; do
    profiles[i]="$(mktemp -d "${TMPDIR:-/tmp}/hyperion-smoke.XXXXXX")"
    env -u DISPLAY -u WAYLAND_DISPLAY DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent \
        setsid timeout --kill-after=10 300 "$electron" \
        ${adapter_switches[@]+"${adapter_switches[@]}"} "${headless_switches[@]}" \
        "--user-data-dir=${profiles[i]}" "$here/out/main/smoke.js" --smoke-variant="${variants[i]}" \
        --smoke-fixture="$fixture" ${captures[@]+"${captures[@]}"} >"$logs/$i.log" 2>&1 &
    groups[i]=$!
done

# Each run's end, seen within a fifth of a second: the shell reaps a run as it ends, after which
# `kill -0` finds no process, and `wait` then gives the code it kept.
running=${#variants[@]}
while [[ $running -gt 0 ]]; do
    for i in "${!variants[@]}"; do
        if [[ -z "${ended[i]:-}" ]] && ! kill -0 "${groups[i]}" 2>/dev/null; then
            status=0
            wait "${groups[i]}" || status=$?
            statuses[i]=$status
            ended[i]=$(now_ms)
            kill -KILL -- "-${groups[i]}" 2>/dev/null || true
            groups[i]=""
            rm -rf -- "${profiles[i]}"
            profiles[i]=""
            running=$((running - 1))
        fi
    done
    if [[ $running -gt 0 ]]; then
        sleep 0.2
    fi
done

first_failure=0
for i in "${!variants[@]}"; do
    echo "== smoke run: variant ${variants[i]}, fixture $fixture"
    cat "$logs/$i.log"
    echo "== exit ${statuses[i]} after $((ended[i] - start)) ms"
    if [[ $first_failure -eq 0 && ${statuses[i]} -ne 0 ]]; then
        first_failure=${statuses[i]}
    fi
done
printed=1
exit "$first_failure"
