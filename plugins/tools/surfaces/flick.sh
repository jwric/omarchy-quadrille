#!/bin/bash
# flick.sh A|B : the size-flicker measurement, as one locked hold of under a minute:
#   flock -o -w 900 /tmp/quadrille-live.lock plugins/tools/surfaces/flick.sh A > $SURF_WORK/flick-A.out
# (`-o`: the lock's file descriptor is not handed to the scratch shell and its
# inotifywait, which would outlive the run and keep the lock.) Needs `build.sh` first.
# A = QA (1.666667) with the unit taken from the window as it used to be, then with
# the output's scale; B = QB (scale 1). Writes SURF logs and grim frames to $SURF_WORK;
# read them with analyse.py and frames.py.
#  nested Hyprland, then the replica shell: QA with the old unit source (tooltip and two popups),
# QA with the fix (every popup), QB with the fix (every popup)
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
case "${1:-A}" in
  A)
    echo "=== QA legacy=1"; LEGACY=1 $N run dbus-run-session -- $HERE/flick-inner.sh QA "tooltip switch"
    echo "=== QA legacy=0"; LEGACY=0 $N run dbus-run-session -- $HERE/flick-inner.sh QA "tooltip power clock tray traymenu switch";;
  B)
    echo "=== QB legacy=0"; LEGACY=0 $N run dbus-run-session -- $HERE/flick-inner.sh QB "tooltip power clock tray traymenu switch";;
esac
$N down
