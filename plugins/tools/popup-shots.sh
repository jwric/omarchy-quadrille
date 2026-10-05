#!/bin/bash
# Screenshot popups of the running shell, over the wallpaper only.
#
#   plugins/tools/popup-shots.sh PREFIX MONITOR:PLUGIN [MONITOR:PLUGIN ...]
#
# Puts both outputs on empty workspaces, grabs a baseline of each, then for every
# MONITOR:PLUGIN focuses that output, summons the plugin through the shell's own
# IPC (`shell summon`), grabs the output as PREFIX-<plugin>-<monitor>.png, hides
# it again, and at the end puts the workspaces and the focus back. PREFIX-base-
# <monitor>.png is the wallpaper alone: plugins/tools/popup-crop.py cuts a popup
# out by what differs from it.
#
# Nothing is typed or clicked: only workspace and focus dispatches. A grab is
# thrown away unless both outputs showed an empty workspace, and no special one,
# just before and just after it, so a screenshot never holds a window.
#
# It moves the user's workspaces for a few seconds, so on a shared desktop run it
# under a lock (`flock -w 900 /tmp/quadrille-live.lock plugins/tools/popup-shots.sh ...`)
# and keep it short. WAIT is the seconds a popup gets to draw (default 1.6).
#
# Hyprland 0.56 (the Lua config) takes dispatchers as expressions:
#   hyprctl dispatch 'hl.dsp.focus({ workspace = "8" })'
set -u
PFX=${1:?usage: popup-shots.sh PREFIX MONITOR:PLUGIN ...}; shift
EMPTY_A=${EMPTY_A:-9}   # workspace to park the first output on
EMPTY_B=${EMPTY_B:-8}   # and the second

mons=$(hyprctl monitors -j)
field() { python3 -c "import json,sys; m=[m for m in json.load(sys.stdin) if m['name']=='$1'][0]; print($2)" <<<"$mons"; }
names=$(python3 -c "import json,sys; print(' '.join(m['name'] for m in json.load(sys.stdin)))" <<<"$mons")
focused=$(python3 -c "import json,sys; print([m['name'] for m in json.load(sys.stdin) if m['focused']][0])" <<<"$mons")
win=$(hyprctl activewindow -j | python3 -c "import json,sys; print(json.load(sys.stdin).get('address',''))")
declare -A ws
for n in $names; do ws[$n]=$(field "$n" "m['activeWorkspace']['id']"); done

d() { hyprctl dispatch "$1" >/dev/null; sleep 0.25; }
empty() {
  python3 - <<'PY'
import json, subprocess, sys
mons = json.loads(subprocess.check_output(["hyprctl", "monitors", "-j"]))
wss = {w["id"]: w for w in json.loads(subprocess.check_output(["hyprctl", "workspaces", "-j"]))}
ok = True
for m in mons:
    w = wss.get(m["activeWorkspace"]["id"])
    if w is None or w["windows"] != 0 or m["specialWorkspace"]["id"] != 0:
        ok = False
        print("not empty:", m["name"], "workspace", m["activeWorkspace"]["id"], file=sys.stderr)
sys.exit(0 if ok else 1)
PY
}
grab() {  # grab OUTPUT FILE
  if empty; then grim -o "$1" "$2"; fi
  if ! empty; then rm -f "$2"; echo "DISCARDED $2 (a workspace was not empty)"; fi
}

i=0
for n in $names; do
  d "hl.dsp.focus({ monitor = \"$n\" })"
  d "hl.dsp.focus({ workspace = \"$([ $i = 0 ] && echo $EMPTY_A || echo $EMPTY_B)\" })"
  i=$((i + 1))
done
sleep 0.4
for n in $names; do grab "$n" "$PFX-base-$n.png"; done

for spec in "$@"; do
  mon=${spec%%:*}; plugin=${spec#*:}; name=${plugin#*.}
  d "hl.dsp.focus({ monitor = \"$mon\" })"
  sleep 0.3
  omarchy-shell shell summon "$plugin" '{}' >/dev/null
  sleep "${WAIT:-1.6}"
  grab "$mon" "$PFX-$name-$mon.png"
  omarchy-shell shell hide "$plugin" >/dev/null
  sleep 0.4
done

for n in $names; do
  d "hl.dsp.focus({ monitor = \"$n\" })"
  d "hl.dsp.focus({ workspace = \"${ws[$n]}\" })"
done
d "hl.dsp.focus({ monitor = \"$focused\" })"
[ -n "$win" ] && d "hl.dsp.focus({ window = \"address:$win\" })"
hyprctl monitors -j | python3 -c "import json,sys; print([(m['name'],m['activeWorkspace']['id'],m['focused']) for m in json.load(sys.stdin)])"
