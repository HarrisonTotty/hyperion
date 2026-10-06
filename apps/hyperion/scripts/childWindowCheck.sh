#!/usr/bin/env bash
# R07.T21's child window on a second monitor, called by `just child-window-check` after a build. It
# runs the smoke harness's child-window scene (`--smoke-child`) on the client's own graphics
# switches: an opener whose full-window canvas stands for the main view, on the primary display,
# and a same-origin child window on the second display whose canvas is a view of the opener's
# engine. The child is resized halfway and closed at the end, its view dropped on its `pagehide`,
# and the opener draws on for 2 s. The run writes its record (the displays and every check with
# its figures) as `<date>-<machine>-child-window.md`.
#
#   childWindowCheck.sh [--seconds <n>] [--hidden] [--out <dir>]
#
# --seconds is how long the child draws before it closes (60 by default). With one display the
# harness refuses, exit 2, before it opens any window. --hidden puts an offscreen child on the same
# display to prove the harness where there is one display; it adds `--disable-vulkan-surface`,
# since creating a hidden offscreen window restarts the GPU process under the Vulkan surface
# (R01.T12 and T13), and writes under target/views-check/. A shown run writes under
# docs/measurements/several-views/ (--out elsewhere).
#
# The run gets a fresh `--user-data-dir`, removed afterwards; Electron runs in a process group of
# its own under `timeout`, killed when the script ends for any reason. The exit status is the
# harness's: 0 every check passed, 1 one failed, 2 a setup error (one display), 3 the watchdog.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$here/../.." && pwd)"
electron="$here/node_modules/.bin/electron"

seconds=60
hidden=0
out=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --seconds) seconds="$2"; shift 2 ;;
        --seconds=*) seconds="${1#--seconds=}"; shift ;;
        --hidden) hidden=1; shift ;;
        --out) out="$2"; shift 2 ;;
        --out=*) out="${1#--out=}"; shift ;;
        *) echo "childWindowCheck.sh: unknown argument $1" >&2; exit 2 ;;
    esac
done
if ! [[ "$seconds" =~ ^[0-9]+$ ]] || [[ "$seconds" -lt 4 ]]; then
    echo "childWindowCheck.sh: --seconds takes a whole number of seconds, 4 or more, not $seconds" >&2
    exit 2
fi
if [[ -z "$out" ]]; then
    out=$([[ $hidden -eq 1 ]] && echo "$repo/target/views-check" || echo "$repo/docs/measurements/several-views")
fi

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

profile="$(mktemp -d "${TMPDIR:-/tmp}/hyperion-child-window.XXXXXX")"
switches=()
child_args=()
if [[ "${XDG_SESSION_TYPE:-}" == wayland ]]; then
    switches+=(--ozone-platform=x11)
fi
if [[ $hidden -eq 1 ]]; then
    switches+=(--disable-vulkan-surface)
    child_args+=(--smoke-child-hidden=1)
fi
status=0
start=$(date +%s)
setsid timeout --kill-after=10 "$(( seconds + 240 ))" "$electron" "${switches[@]}" \
    "--user-data-dir=$profile" "$here/out/main/smoke.js" \
    "--smoke-child=$seconds" "--smoke-out=$out" "${child_args[@]}" &
group=$!
wait "$group" || status=$?
echo "child window check: exit $status after $(( $(date +%s) - start )) s"
exit "$status"
