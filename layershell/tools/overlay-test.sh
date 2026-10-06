#!/bin/bash
# Native reticle placement and cost, with input confined to nested Hyprland.
set -euo pipefail
HERE=$(cd "$(dirname "$0")/.." && pwd)
OUT=${2:-${1:-/tmp/quadrille-overlay-test}}
if [ "${1:-}" != --locked ]; then
  mkdir -p "$OUT"
  exec flock -o -w 900 /tmp/quadrille-live.lock timeout --kill-after=2 55 "$0" --locked "$OUT"
fi
mkdir -p "$OUT"
export QUADRILLE_NESTED_DIR="$OUT/nested"
export QUADRILLE_BAR_SOCKET=quadrille-overlay-test.sock
touch "$OUT/reticle.capture"
host_pid=""
driver_pid=""
kill_tree() {
  local child
  for child in $(pgrep -P "$1" || true); do kill_tree "$child"; done
  kill "$1" 2>/dev/null || true
}
cleanup() {
  if [ -n "$driver_pid" ]; then kill_tree "$driver_pid"; wait "$driver_pid" 2>/dev/null || true; fi
  if [ -n "$host_pid" ]; then kill_tree "$host_pid"; wait "$host_pid" 2>/dev/null || true; fi
  "$HERE/tools/nested.sh" down >/dev/null 2>&1 || true
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
"$HERE/tools/nested.sh" up > "$OUT/nested.log" 2>&1
"$HERE/tools/nested.sh" run env VPTR_EXTENT=3440x1440 "$HERE/target/release/vptr" QB move 420 300 sleep 100
"$HERE/tools/nested.sh" run env QUADRILLE_COMMANDS="$HERE/tools/stubs" QUADRILLE_STUB_STATE="$OUT/stubs" QUADRILLE_RETICLE_DUMP="$OUT/reticle" RUST_LOG=quadrille_bar=debug,iced_layer=info \
  "$HERE/target/release/quadrille-bar" --no-bar > "$OUT/host.log" 2>&1 &
host_pid=$!
python3 "$HERE/tools/overlay-test.py" "$HERE" "$OUT" "$host_pid" &
driver_pid=$!
wait "$driver_pid"
driver_pid=""
