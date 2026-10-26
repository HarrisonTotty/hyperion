#!/usr/bin/env bash
# The headless smoke harness's runner (R01.T9, Design note 17), called by `just test-render` under
# the heavy-test lock after a build. It runs the built harness on SwiftShader with no display, once
# per part of the page and variant: the parts one after the other, the variants of a part at once.
# It exits with the code of the first run, in part and then variant order, that does not exit 0: 1
# a property failed or a request went out, 2 a setup error (no adapter), 3 the watchdog.
#
#   testRender.sh [--variant=NAME]... [--part=N]... [--fixture=broken-wgsl|external-fetch]
#                 [--drop-adapter-switches] [--captures=DIR]
#
# The page's groups are in eleven parts (`src/renderer/src/smoke/page.ts`), each a run of its own
# under its own watchdog, the harness's hang detector (perf-smoke, 2026-10-13): parts 2 to 6 and 8
# to 11 hold the atmosphere's CPU-twin groups (2 R08.T6.b but its four absorbers, 3 R08.T6.g, 4
# R08.T8.b's far limb, 5 R08.T6.f's Saturn-class figure and R08.T6.i, 6 R08.T6.b's four absorbers,
# 8 R08.T6.f's Earth, 9 R08.T8.a's march band, 10 R08.T7 and R08.T8.e's slot layers on its binary,
# 11 R08.T9.b's two and R08.T8.c's far limb's footprint), part 4 also R05.T12.e's marches and
# part 6 R05.T12.c's frames, each first, part 7 R10's terrain groups, and part 1 every other group.
# SwiftShader's GPU process and the twins together took some 940 CPU-seconds for the two variants
# (measured 2026-10-13), so on a machine busy with other work, with about 4 cores left to the
# smoke, a run of the whole page outlasted the 180 s watchdog however its work overlapped. Three
# parts left the third 140 s once R08.T7 had joined it (load 3 to 9), so there were four; four
# left three of them 124 to 155 s once R08.T6.f's slices had joined them (load about 7), so there
# are five, each about a fifth of the page; R08.T9.b's surface lighting took a sixth of its own,
# and R10's terrain groups a seventh once R10.T9.b's cascades took the sixth to 163 s at a load of
# 2 (2026-10-21). R08.T8.c's footprints, whose nodes the twins and SwiftShader march on every limb
# from above the top, took part 4 past its 180 s watchdog and parts 2, 3, 5 and 6 to 149 to 159 s
# (2026-10-22), so there are nine: R08.T6.i, R08.T6.f's Earth, R08.T8.a's march band and R08.T7
# moved to parts 5, 8 and 9, and R08.T8.c's footprint took part 8, every part about 130 s at most
# at those figures but part 2, R08.T6.b's alone at 149 s. R08.T8.e's slot layers took part 2 past
# the watchdog (175 s at 100bee27's gate), so R08.T6.b's four absorbers moved to part 6 and
# R08.T9.b's Saturn-class figure to part 2: 140 s and 122 to 124 s at a load of 7 to 12
# (2026-10-23); with R08.T8.e's slot-layer texel checks part 2 holds 141 s, part 6 133 to 137 and
# part 9 158 to 160 at a load of 7 to 14, and with R08.T7's binary's in part 1 instead part 9
# held 152 to 154 s and part 1 122 to 123 at a load of 8 to 11; no group of part 9 fitted another
# part whole, so there are ten, R08.T7 and the binary's slot layers in part 10 (part 9 88 s and part
# 10 98 s on the whole render at a load of median 14, each part's start 1 to 7 s); and part 8 stood
# at 151 to 153 s, so there are eleven, R08.T8.c's footprint in part 11 (51 s; part 8 93 s). The
# whole render took 1,281 s at a load of median 14, its longest parts 6 (154 s) and 2 (149 s); the
# two parts' starts and their twins set up again cost it about 30 s (2026-10-23).
# --part=N runs that part alone (every part by default).
#
# --captures=DIR also renders the capture frames (R05.T12.c's atmosphere comparison, R08.T8.b's
# ascent, R05.T11.c's terrain, R05.T13.b's spike, R07.T9's occultation, R07.T10.c's eclipse scene)
# and saves them in DIR as PNGs, each named for its variant; they are part 1's, and they scale part
# 1's watchdog (`src/smoke/result.ts`'s `smokeWatchdogMs`): it gains `CAPTURES_WATCHDOG_MS`, and
# part 1's outer limit below gains the same `captures_allowance_s`, so that it stays 120 s over the
# watchdog. Alone the captures overran part 1's 180 s (2026-10-21).
#
# --drop-adapter-switches removes only `--enable-unsafe-webgpu` and `--use-webgpu-adapter`, keeping
# the headless Ozone, ANGLE and Vulkan switches, so that Electron starts and finds no adapter.
#
# A variant keeps 1.4 to 2.3 cores busy, so a part's variants run side by side rather than one after
# another (perf-ts.md, Q2; 141 s for both against about 230 s, measured 2026-10-10). Each run's
# output goes to a log of its own, and the logs are printed whole in order once every run has ended,
# each with the time from its part's start to its own end; the exit code is the one the runs gave
# one after another, except that a later run is no longer skipped after a failure.
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
parts=()
fixture=none
captures=()
for arg in "$@"; do
    case "$arg" in
        --variant=*) variants+=("${arg#--variant=}") ;;
        --part=[1-9] | --part=1[01]) parts+=("${arg#--part=}") ;;
        --fixture=*) fixture="${arg#--fixture=}" ;;
        --drop-adapter-switches) adapter_switches=() ;;
        --captures=*) captures=("--smoke-captures=${arg#--captures=}") ;;
        *) echo "testRender.sh: unknown argument $arg" >&2; exit 2 ;;
    esac
done
if [[ ${#variants[@]} -eq 0 ]]; then
    variants=(default no-subgroups)
fi
if [[ ${#parts[@]} -eq 0 ]]; then
    parts=(1 2 3 4 5 6 7 8 9 10 11)
fi

# Each run's outer limit, s: 120 s over its watchdog (`src/smoke/result.ts`'s
# `SMOKE_WATCHDOG_BASE_MS`, 180 s), and over part 1's with the captures, which gains
# `CAPTURES_WATCHDOG_MS` (`result.test.ts` holds the allowance equal to it).
outer_limit_s=300
captures_allowance_s=1380

# Milliseconds since the epoch: GNU date's `%3N`, or Perl's clock where date has no `%N` (macOS).
now_ms() {
    local now
    now="$(date +%s%3N 2>/dev/null || true)"
    if [[ -z "$now" || "$now" == *[!0-9]* ]]; then
        now="$(perl -MTime::HiRes=time -e 'printf "%d\n", time * 1000')"
    fi
    echo "$now"
}

# Per run, by its index (part by part, each part's variants in order): its part, its variant, its
# profile and its process group until it has ended, then its exit code and how long it took from
# its part's start. An ended run's group and profile are gone and are cleared, so that the cleanup
# never kills a group that has since taken the same ID.
run_parts=()
run_variants=()
profiles=()
groups=()
statuses=()
took=()
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
            for i in ${run_parts[@]+"${!run_parts[@]}"}; do
                if [[ -f "$logs/$i.log" ]]; then
                    echo "== smoke run cut short: variant ${run_variants[i]}, part ${run_parts[i]}, fixture $fixture" >&2
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

# Electron's package fetches its binary the first time it is asked for it (`require("electron")`),
# which runs started at once would each try, and all but one fail (seen in a fresh worktree,
# 2026-10-11): it is asked for here first, alone. A binary already fetched is only looked up.
if ! (cd "$here" && node -e 'require("electron")' >&2); then
    echo "testRender.sh: Electron's binary is missing and could not be fetched" >&2
    exit 2
fi

logs="$(mktemp -d "${TMPDIR:-/tmp}/hyperion-smoke-logs.XXXXXX")"
for part in "${parts[@]}"; do
    start=$(now_ms)
    first=${#run_parts[@]}
    limit_s=$outer_limit_s
    if [[ ${#captures[@]} -gt 0 && "$part" == 1 ]]; then
        limit_s=$((outer_limit_s + captures_allowance_s))
    fi
    for variant in "${variants[@]}"; do
        i=${#run_parts[@]}
        run_parts[i]=$part
        run_variants[i]=$variant
        profiles[i]="$(mktemp -d "${TMPDIR:-/tmp}/hyperion-smoke.XXXXXX")"
        env -u DISPLAY -u WAYLAND_DISPLAY DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent \
            setsid timeout --kill-after=10 "$limit_s" "$electron" \
            ${adapter_switches[@]+"${adapter_switches[@]}"} "${headless_switches[@]}" \
            "--user-data-dir=${profiles[i]}" "$here/out/main/smoke.js" --smoke-variant="$variant" \
            --smoke-part="$part" --smoke-fixture="$fixture" ${captures[@]+"${captures[@]}"} \
            >"$logs/$i.log" 2>&1 &
        groups[i]=$!
    done

    # Each run's end, seen within a fifth of a second: the shell reaps a run as it ends, after which
    # `kill -0` finds no process, and `wait` then gives the code it kept.
    running=${#variants[@]}
    while [[ $running -gt 0 ]]; do
        for (( i = first; i < ${#run_parts[@]}; i++ )); do
            if [[ -z "${took[i]:-}" ]] && ! kill -0 "${groups[i]}" 2>/dev/null; then
                status=0
                wait "${groups[i]}" || status=$?
                statuses[i]=$status
                took[i]=$(( $(now_ms) - start ))
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
done

first_failure=0
for i in "${!run_parts[@]}"; do
    echo "== smoke run: variant ${run_variants[i]}, part ${run_parts[i]}, fixture $fixture"
    cat "$logs/$i.log"
    echo "== exit ${statuses[i]} after ${took[i]} ms"
    if [[ $first_failure -eq 0 && ${statuses[i]} -ne 0 ]]; then
        first_failure=${statuses[i]}
    fi
done
printed=1
exit "$first_failure"
