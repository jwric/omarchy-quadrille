#!/bin/bash
# popups-inner.sh SCREEN [CASES] (a copy of flick-inner.sh whose switch case takes switch/A/B: popup A, then B 300 ms later): inside the nested compositor and a private bus. Opens each popup while a tight
# grim loop records the corner of the output; the shell logs SURF lines (QUADRILLE_DEBUG_SURFACES=1).
# LEGACY=1 in the environment makes Px resolve the unit from the window's own ratio, as before the fix.
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
screen=$1; cases=${2:-"power network switch tooltip"}
export HOME=$R/home OMARCHY_PATH=$R/omarchy QUADRILLE_PIXEL_TRAY=1 QUADRILLE_DEBUG_SURFACES=1 QUADRILLE_ONLY_SCREEN=$screen QUADRILLE_LEGACY_DPR=${LEGACY:-0}
tag=$screen; [ "${LEGACY:-0}" = 1 ] && tag=$screen-legacy
q() { timeout 5 quickshell -p $R/omarchy/shell "$@"; }
env QT_QPA_PLATFORM=wayland quickshell -p $R/omarchy/shell > $S/flick-$tag.log 2>&1 &
qpid=$!
for i in $(seq 1 20); do sleep 0.5; [ "$(q ipc call shell ping 2>&1)" = ok ] && break; done
echo "shell up after $i"
sleep 2
read mx my mw mh ms <<< "$(hyprctl monitors -j | python3 -c "
import json,sys
for m in json.load(sys.stdin):
    if m['name']=='$screen': print(m['x'],m['y'],m['width'],m['height'],m['scale'])")"
echo "monitor $screen: $mx $my $mw $mh $ms"
lw=$(python3 -c "print(int($mw/$ms))")
region="$((mx + lw/2)),$my $((lw/2))x560"
echo "region $region"
mkdir -p $S/frames/$tag; rm -f $S/frames/$tag/*
capture() { # name n  (the tooltip hangs off the left of the bar, the popups off the right)
  reg=$region; [ $1 = tooltip ] && reg="$mx,$my $((lw/2))x200"
  for i in $(seq 1 $2); do
    echo "$1-$i $(date +%s%3N)" >> $S/frames/$tag/ts
    grim -t ppm -g "$reg" $S/frames/$tag/$1-$i.ppm
  done
}
mark() { echo "MARK $(date +%s%3N) $1"; }
opener() { # name -> open command
  case $1 in
    tray) q ipc call quadrille.tray drawer;;
    traymenu) q ipc call quadrille.tray menu 0;;
    tooltip) q ipc call quadrille.debug tooltip 2 "Tooltip text";;
    *) q ipc call omarchy.$1 toggle;;
  esac
}
closer() {
  case $1 in
    tray|traymenu) q ipc call quadrille.tray close;;
    tooltip) ;;
    *) q ipc call omarchy.$1 close >/dev/null 2>&1 || q ipc call omarchy.$1 toggle;;
  esac
}
if echo " $cases " | grep -q " tray"; then
  python3 $HERE/../fake_sni.py 40 3 > $S/fake-$tag.log 2>&1 &
  fpid=$!; sleep 1.5
fi
for t in $cases; do
  case $t in switch|switch/*)
    a=power; b=clock
    if [ $t != switch ]; then IFS=/ read _ a b <<< "$t"; fi
    capture ${t//\//-} 18 & cap=$!
    sleep 0.12
    mark "open $a"; q ipc call omarchy.$a toggle >/dev/null 2>&1
    sleep 0.3
    mark "open $b (switch)"; q ipc call omarchy.$b toggle >/dev/null 2>&1
    wait $cap
    mark "close"; closer $a; closer $b;;
  *)
    capture $t 14 & cap=$!
    sleep 0.12
    mark "open $t"; out=$(opener $t 2>&1); echo "open $t -> [$out]"
    wait $cap
    mark "close $t"; closer $t;;
  esac
  sleep 0.7
done
[ -n "$fpid" ] && kill $fpid 2>/dev/null
kill $qpid; sleep 0.5; kill -9 $qpid 2>/dev/null
echo finished
