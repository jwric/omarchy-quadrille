#!/bin/bash
# textbench.sh : how long the ways of drawing pixel text take, and whether they draw the same pixels, in one locked hold:
#   flock -o -w 900 /tmp/quadrille-live.lock plugins/tools/surfaces/textbench.sh > $SURF_WORK/bench.out 2>&1
# Variants (see textbench/): 0 PixelText (a Rectangle for each run), 1 the runs merged down the rows, 2 one Shape for a line.
# `BENCH` lines: create= is the time to make the items, then= the time until the next event after that (layout, sync).
for fd in 3 4 5 6 7 8 9; do eval "exec $fd>&-"; done
cleanup() {
  kill $(jobs -p) 2>/dev/null
  for p in $(pgrep -f "[q]uickshell -p .*$S/benchroot"); do kill $p 2>/dev/null; done
  for p in $(pgrep -f "[i]notifywait.*$S/benchroot"); do kill $p 2>/dev/null; done
}
trap cleanup EXIT INT TERM
HERE=$(cd "$(dirname "$0")" && pwd)
S=${SURF_WORK:-${TMPDIR:-/tmp}/quadrille-surfaces}
N=$HERE/../../../layershell/tools/nested.sh
export QUADRILLE_NESTED_DIR=$S/nested
B=$S/benchroot
rm -rf $B; $HERE/../testroot.sh $B >/dev/null
for f in shell.qml PixelTextMerged.qml PixelTextShape.qml Runs.js; do ln -sfn $HERE/textbench/$f $B/$f; done
$N up || exit 1
$N run dbus-run-session -- $HERE/textbench-inner.sh
$N down
