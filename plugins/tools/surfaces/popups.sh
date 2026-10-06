#!/bin/bash
# popups.sh SCREEN "CASES" : the size-flicker measurement for the popup clones, one locked hold:
#   flock -o -w 900 /tmp/quadrille-live.lock plugins/tools/surfaces/popups.sh QA "audio power switch/audio/weather" > $SURF_WORK/flick-A.out
# SCREEN is QA (1.666667) or QB (1). Needs `build.sh` first. Same output files as flick.sh: read them with
# analyse.py TAG and frames.py TAG CASE (TAG is the screen name).
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
echo "=== ${1:-QA} legacy=0"
LEGACY=0 $N run dbus-run-session -- $HERE/popups-inner.sh "${1:-QA}" "${2:-audio power}"
$N down
