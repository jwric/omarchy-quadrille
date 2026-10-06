#!/bin/bash
for fd in 3 4 5 6 7 8 9; do eval "exec $fd>&-"; done
cleanup() {
  kill $(jobs -p) 2>/dev/null
  for p in $(pgrep -f "[q]uickshell -p .*$S/benchroot"); do kill $p 2>/dev/null; done
  for p in $(pgrep -f "[i]notifywait.*$S/benchroot"); do kill $p 2>/dev/null; done
}
trap cleanup EXIT INT TERM
S=${SURF_WORK:-${TMPDIR:-/tmp}/quadrille-surfaces}
B=$S/benchroot
export QUADRILLE_DEBUG_SURFACES=1 BENCH_SCREEN=QA
q() { timeout 8 quickshell -p $B "$@"; }
env QT_QPA_PLATFORM=wayland quickshell -p $B > $S/bench.log 2>&1 &
qpid=$!
for i in $(seq 1 30); do sleep 0.5; [ -n "$(q ipc show 2>&1 | grep -c bench | grep -v '^0')" ] && break; done
echo "up after $i"; sleep 2
read mx my mw mh ms <<< "$(hyprctl monitors -j | python3 -c "
import json,sys
for m in json.load(sys.stdin):
    if m['name']=='QA': print(m['x'],m['y'],m['width'],m['height'],m['scale'])")"
echo "QA $mx $my $mw $mh $ms"
mkdir -p $S/bench; rm -f $S/bench/*
TEXT="Hello Wifi 123 Wednesday 16 km/h"
for v in 0 1 2; do
  q ipc call bench run $v 60 "$TEXT" >/dev/null; sleep 1.2
  grim -t ppm -g "$mx,$my 760x560" $S/bench/v$v.ppm
done
q ipc call bench clear >/dev/null
for rep in 1 2 3; do for v in 0 1 2; do
  q ipc call bench run $v 300 "$TEXT" >/dev/null; sleep 2.0
done; done
q ipc call bench clear >/dev/null
sed 's/\x1b\[[0-9;]*m//g' $S/bench.log | grep "BENCH\|Error\|error\|Binding"
python3 - <<PY
from PIL import Image, ImageChops
a=Image.open("$S/bench/v0.ppm").convert("RGB")
for v in (1,2):
    b=Image.open("$S/bench/v%d.ppm"%v).convert("RGB")
    d=ImageChops.difference(a,b).convert("L").point(lambda x:255 if x>0 else 0)
    print("pixels v0 vs v%d: %d differ, bbox %s"%(v, sum(1 for p in d.getdata() if p), d.getbbox()))
PY
kill $qpid; sleep 0.5; kill -9 $qpid 2>/dev/null
echo finished
