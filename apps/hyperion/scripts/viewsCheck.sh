#!/usr/bin/env bash
# The several-views check's runner (plan R07, T20), called by `just views-check` after a build. It
# runs the built client with `--views-check`, which opens `VIEW` on the kept `PHASE TEST` scene and
# drives it through its phases (a photorealistic primary alone and with two wireframe instruments,
# a resize of the primary alone, a wireframe primary with two wireframe instruments, with a
# photorealistic and a wireframe instrument, and alone), then writes the results file and its
# summary. No server is needed.
#
#   viewsCheck.sh [--hidden] [--setting high|low] [--smoke] [--out <dir>]
#
# --setting is VIEW's quality setting and the window's size: high 1920 x 1080, low 1280 x 720.
# --hidden never shows the window (offscreen rendering): no presentation times and no question,
# so such a run proves the harness, not T20's criterion. --smoke runs short windows, hidden, and
# writes under target/views-check/; a full run writes under docs/measurements/several-views/
# (--out elsewhere). A shown run asks once whether every view is the right way up.
#
# The run gets a fresh `--user-data-dir`, removed afterwards; Electron runs in a process group of
# its own under `timeout`, killed when the script ends for any reason. The exit status is the
# client's: 0 the run is recorded, 1 it could not finish, 3 the watchdog.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$here/../.." && pwd)"
electron="$here/node_modules/.bin/electron"

hidden=0
smoke=0
check_args=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --hidden) hidden=1; shift ;;
        --smoke) smoke=1; check_args+=("$1"); shift ;;
        *) check_args+=("$1"); shift ;;
    esac
done

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

profile="$(mktemp -d "${TMPDIR:-/tmp}/hyperion-views-check.XXXXXX")"
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
limit=$(( smoke == 1 ? 300 : 1500 ))
status=0
start=$(date +%s)
cd "$repo"
env "${env_run[@]}" setsid timeout --kill-after=10 "$limit" "$electron" "${x11[@]}" \
    "$here/out/main/index.js" --views-check "${check_args[@]}" "--user-data-dir=$profile" &
group=$!
wait "$group" || status=$?
echo "views check: exit $status after $(( $(date +%s) - start )) s"
exit "$status"
