#!/bin/bash
# trayn-inner.sh : inside nested + private bus. All three outputs get the bar; fake tray items; drawer; screenshots.
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
R=$S/rep
export HOME=$R/home OMARCHY_PATH=$R/omarchy QUADRILLE_PIXEL_TRAY=${PIXEL:-1} QUADRILLE_DEBUG_SURFACES=1
q() { timeout 5 quickshell -p $R/omarchy/shell "$@"; }
env QT_QPA_PLATFORM=wayland quickshell -p $R/omarchy/shell > $S/trayn.log 2>&1 &
qpid=$!
for i in $(seq 1 20); do sleep 0.5; [ "$(q ipc call shell ping 2>&1)" = ok ] && break; done
echo "shell up after $i"
sleep 2
hyprctl monitors -j | python3 -c "
import json,sys
for m in json.load(sys.stdin): print(m['name'],m['x'],m['y'],m['width'],m['height'],m['scale'])"
python3 $HERE/../fake_sni.py 9 4 > $S/trayn-fake.log 2>&1 &
fpid=$!
mkdir -p $S/trayn
shot() { for o in QA QB; do read mx my mw mh ms <<< "$(hyprctl monitors -j | python3 -c "
import json,sys
for m in json.load(sys.stdin):
    if m['name']=='$o': print(m['x'],m['y'],m['width'],m['height'],m['scale'])")"; lw=$(python3 -c "print(int($mw/$ms))"); grim -t ppm -g "$((mx + lw/2)),$my $((lw/2))x360" $S/trayn/$1-$o.ppm; done; }
for i in 1 2 3 4 5 6 7 8; do
  sleep 1
  out=$(timeout 3 quickshell -p $R/omarchy/shell ipc call shell ping 2>&1); rc=$?
  echo "t=$i rc=$rc $out"
  case $i in
    2) shot bar;;
    3) q ipc call quadrille.tray drawer; sleep 0.4; shot drawer; q ipc call quadrille.tray close;;
    5) q ipc call quadrille.tray menu 0; sleep 0.4; shot menu; q ipc call quadrille.tray close;;
    7) q ipc call quadrille.tray manage; sleep 0.4; shot manage; q ipc call quadrille.tray close;;
  esac
  if [ $rc != 0 ]; then echo FROZEN; top -H -b -n1 -p $qpid | head -15; timeout 25 gdb -p $qpid -batch -ex "thread apply all bt 14" > $S/gdb.txt 2>&1; break; fi
done
kill $fpid 2>/dev/null; sleep 1
out=$(timeout 3 quickshell -p $R/omarchy/shell ipc call shell ping 2>&1); echo "after rc=$? $out"
kill $qpid; sleep 0.5; kill -9 $qpid 2>/dev/null
echo finished
