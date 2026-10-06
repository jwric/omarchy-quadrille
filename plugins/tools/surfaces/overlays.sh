#!/bin/bash
# overlays.sh SCREEN "CASES": the size-flicker measurement for the overlay clones, one locked hold:
#   flock -o -w 900 /tmp/quadrille-live.lock plugins/tools/surfaces/overlays.sh QA > $SURF_WORK/overlays-QA.out
# SCREEN is QA (1.666667) or QB (1). Needs overlays-build.sh first. Read the result with
#   plugins/tools/surfaces/overlays-report.py SCREEN
for fd in 3 4 5 6 7 8 9; do eval "exec $fd>&-"; done
cleanup() {
  kill $(jobs -p) 2>/dev/null
  for p in $(pgrep -f "[i]notifywait.*$S/rep"); do kill $p 2>/dev/null; done
  for p in $(pgrep -f "[q]uickshell -p .*$S/"); do kill $p 2>/dev/null; done
}
trap cleanup EXIT INT TERM
HERE=$(cd "$(dirname "$0")" && pwd)
S=${SURF_WORK:-${TMPDIR:-/tmp}/quadrille-surfaces}
N=$HERE/../../../layershell/tools/nested.sh
export QUADRILLE_NESTED_DIR=$S/nested
rm -f $S/rep/home/.local/state/omarchy/toggles/bar-off
$N up || exit 1
echo "=== ${1:-QA}"
$N run dbus-run-session -- $HERE/overlays-inner.sh "${1:-QA}" "${2:-}"
$N down
