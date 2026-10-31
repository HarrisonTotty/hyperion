#!/usr/bin/env bash
# R03.T15's transfer check in the real renderer (plan R10, R10.T15.b): one generated body's coarse
# field, asked of a running hyperion-server through `RequestClient.requestBulk` over a real socket,
# its chunks and terminal response checked whole and in order (`src/renderer/src/smoke/transfer.ts`),
# its size recorded against 15 MiB (61 chunks). Run it after a build (`pnpm --filter hyperion build`):
#
#   transferCheck.sh [--port <port>] [--workers <n>] [--universe <NAME|ID>] [--body <BODY_ID>]
#                    [--survey <SOURCE> <RESOLUTION_M>]
#
# It builds and starts a release server on --port (7881 by default) with its pool at --workers
# (4), places the ship with place-ship (`--json`) at --body in --universe (place-ship's defaults: the
# dev universe, and the first planet of the nearest system with one) and surveys every cell of the
# body's coarse field (`--survey`, orbital at 50 m by default), then runs the smoke harness's
# transfer mode (`out/main/smoke.js --smoke-transfer`) on the place-ship's universe and body,
# headless on SwiftShader with no display, as `testRender.sh` runs the harness: Electron's one
# request beside the page's own files is that server's socket. Behind the serve gate
# (`GENERATED_SURFACES_FROM`, generator version 25) the server answers the field `not_modelled`,
# and the check fails, saying so.
#
# The server and Electron run in process groups of their own under `timeout`, each killed when the
# script ends for any reason; Electron gets a fresh `--user-data-dir`, removed afterwards. The exit
# status is the harness's: 0 pass, 1 a check failed, 2 a setup error (the server or place-ship
# failed first), 3 the watchdog. `setsid` and `timeout` are util-linux's and coreutils', or on
# macOS the justfile's `tools/portable` stand-ins. It runs on bash 3.2 (macOS's).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$here/../.." && pwd)"
electron="$here/node_modules/.bin/electron"
server="$repo/target/release/hyperion-server"

port=7881
workers=4
universe=""
body=""
source=orbital
resolution=50
while [[ $# -gt 0 ]]; do
    case "$1" in
        --port) port="$2"; shift 2 ;;
        --workers) workers="$2"; shift 2 ;;
        --universe) universe="$2"; shift 2 ;;
        --body) body="$2"; shift 2 ;;
        --survey) source="$2"; resolution="$3"; shift 3 ;;
        *) echo "transferCheck.sh: unknown argument $1" >&2; exit 2 ;;
    esac
done
if ! [[ "$port" =~ ^[0-9]+$ && "$workers" =~ ^[0-9]+$ ]]; then
    echo "transferCheck.sh: --port and --workers take whole numbers" >&2
    exit 2
fi

profile=""
groups=()
cleanup() {
    for group in ${groups[@]+"${groups[@]}"}; do
        kill -KILL -- "-$group" 2>/dev/null || true
    done
    # Reaped here, so that bash reports no killed job.
    wait 2>/dev/null || true
    if [[ -n "$profile" ]]; then
        rm -rf -- "$profile"
    fi
}
trap cleanup EXIT INT TERM

# The harness's watchdog is 14 minutes in its transfer mode (`TRANSFER_WATCHDOG_MS` over the base),
# so that the outer limits stay above it.
limit=1020
cargo build --release --quiet --manifest-path "$repo/Cargo.toml" -p hyperion-server
setsid timeout --kill-after=10 "$(( limit + 120 ))" \
    "$server" --num-workers "$workers" --port "$port" >/dev/null 2>&1 &
groups+=("$!")

place=(--port "$port" --json --survey "$source" "$resolution")
if [[ -n "$universe" ]]; then
    place+=(--universe "$universe")
fi
if [[ -n "$body" ]]; then
    place+=(--look-at "$body")
fi
# The server listens within a few seconds of starting; place-ship fails at once until it does.
placement=""
for (( try = 0; try < 60; try++ )); do
    if placement="$(cd "$here" && node scripts/placeShip.mjs "${place[@]}" 2>/dev/null)"; then
        break
    fi
    placement=""
    sleep 1
done
if [[ -z "$placement" ]]; then
    echo "transferCheck.sh: place-ship could not place the ship on the server at port $port:" >&2
    (cd "$here" && node scripts/placeShip.mjs "${place[@]}") >&2 || true
    exit 2
fi
ids="$(node -e 'const p = JSON.parse(process.argv[1]); process.stdout.write(`${p.universe.id} ${p.body}`);' "$placement")"
universe_id="${ids% *}"
body_id="${ids#* }"
echo "transfer check: universe $universe_id, body $body_id, $source survey at $resolution m"

profile="$(mktemp -d "${TMPDIR:-/tmp}/hyperion-transfer.XXXXXX")"
status=0
start=$(date +%s)
env -u DISPLAY -u WAYLAND_DISPLAY DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent \
    setsid timeout --kill-after=10 "$limit" "$electron" \
    --ozone-platform=headless --use-angle=swiftshader --enable-features=Vulkan \
    --use-vulkan=swiftshader "--user-data-dir=$profile" "$here/out/main/smoke.js" \
    "--smoke-transfer=ws://127.0.0.1:$port/ws" "--smoke-transfer-universe=$universe_id" \
    "--smoke-transfer-body=$body_id" &
groups+=("$!")
wait "$!" || status=$?
echo "transfer check: exit $status after $(( $(date +%s) - start )) s"
exit "$status"
