#!/bin/bash
for fd in 3 4 5 6 7 8 9; do eval "exec $fd>&-"; done   # never hand the live lock to a child
cleanup() {
  kill $(jobs -p) 2>/dev/null
  # the scratch shell's plugin watcher is orphaned when it dies: take it down by its path
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
$N run dbus-run-session -- $HERE/tray-inner.sh
$N down
