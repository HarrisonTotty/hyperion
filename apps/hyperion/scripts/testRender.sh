#!/usr/bin/env bash
# The headless smoke harness's runner (R01.T9, Design note 17), called by `just test-render` under
# the heavy-test lock after a build. It runs the built harness once per variant on SwiftShader with
# no display, and stops at the first run that does not exit 0, with that run's code: 1 a property
# failed or a request went out, 2 a setup error (no adapter), 3 the watchdog.
#
#   testRender.sh [--variant=NAME]... [--fixture=broken-wgsl|external-fetch] [--drop-adapter-switches]
#                 [--captures=DIR]
#
# --captures=DIR also renders the capture frames (R05.T12.c's atmosphere comparison, R05.T11.c's
# terrain, R05.T13.b's spike, R07.T9's occultation) and saves them in DIR as PNGs.
#
# --drop-adapter-switches removes only `--enable-unsafe-webgpu` and `--use-webgpu-adapter`, keeping
# the headless Ozone, ANGLE and Vulkan switches, so that Electron starts and finds no adapter.
#
# Each run gets a fresh `--user-data-dir`, removed afterwards, so that a run killed part-way leaves
# no profile, lock or cache for the next; Electron runs in a process group of its own under a
# `timeout`, and that whole group is killed when the script ends for any reason. `setsid` and
# `timeout` are util-linux's and coreutils', or on macOS the justfile's `tools/portable` stand-ins.
# It runs on bash 3.2 (macOS's): an array that may be empty is expanded as `${a[@]+"${a[@]}"}`.
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

profile=""
group=""
cleanup() {
    if [[ -n "$group" ]]; then
        kill -KILL -- "-$group" 2>/dev/null || true
    fi
    if [[ -n "$profile" ]]; then
        rm -rf -- "$profile"
    fi
}
trap cleanup EXIT INT TERM

for variant in "${variants[@]}"; do
    echo "== smoke run: variant $variant, fixture $fixture"
    profile="$(mktemp -d "${TMPDIR:-/tmp}/hyperion-smoke.XXXXXX")"
    start=$(now_ms)
    status=0
    env -u DISPLAY -u WAYLAND_DISPLAY setsid timeout --kill-after=10 300 "$electron" \
        ${adapter_switches[@]+"${adapter_switches[@]}"} "${headless_switches[@]}" \
        "--user-data-dir=$profile" "$here/out/main/smoke.js" --smoke-variant="$variant" \
        --smoke-fixture="$fixture" ${captures[@]+"${captures[@]}"} &
    group=$!
    wait "$group" || status=$?
    kill -KILL -- "-$group" 2>/dev/null || true
    group=""
    rm -rf -- "$profile"
    profile=""
    echo "== exit $status after $(( $(now_ms) - start )) ms"
    if [[ $status -ne 0 ]]; then
        exit "$status"
    fi
done
