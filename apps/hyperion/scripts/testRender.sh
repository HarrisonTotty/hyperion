#!/usr/bin/env bash
# The headless smoke harness's runner (R01.T9, Design note 17), called by `just test-render` under
# the heavy-test lock after a build. It runs the built harness once per variant on SwiftShader with
# no display, and stops at the first run that does not exit 0, with that run's code: 1 a property
# failed or a request went out, 2 a setup error (no adapter), 3 the watchdog.
#
#   testRender.sh [--variant=NAME]... [--fixture=broken-wgsl|external-fetch] [--drop-adapter-switches]
#
# --drop-adapter-switches removes only `--enable-unsafe-webgpu` and `--use-webgpu-adapter`, keeping
# the headless Ozone, ANGLE and Vulkan switches, so that Electron starts and finds no adapter.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
electron="$here/node_modules/.bin/electron"

adapter_switches=(--enable-unsafe-webgpu --use-webgpu-adapter=swiftshader)
headless_switches=(--ozone-platform=headless --use-angle=swiftshader --enable-features=Vulkan --use-vulkan=swiftshader)

variants=()
fixture=none
for arg in "$@"; do
    case "$arg" in
        --variant=*) variants+=("${arg#--variant=}") ;;
        --fixture=*) fixture="${arg#--fixture=}" ;;
        --drop-adapter-switches) adapter_switches=() ;;
        *) echo "testRender.sh: unknown argument $arg" >&2; exit 2 ;;
    esac
done
if [[ ${#variants[@]} -eq 0 ]]; then
    variants=(default no-subgroups)
fi

for variant in "${variants[@]}"; do
    echo "== smoke run: variant $variant, fixture $fixture"
    start=$(date +%s%3N)
    status=0
    env -u DISPLAY -u WAYLAND_DISPLAY "$electron" "${adapter_switches[@]}" "${headless_switches[@]}" \
        "$here/out/main/smoke.js" --smoke-variant="$variant" --smoke-fixture="$fixture" || status=$?
    echo "== exit $status after $(( $(date +%s%3N) - start )) ms"
    if [[ $status -ne 0 ]]; then
        exit "$status"
    fi
done
