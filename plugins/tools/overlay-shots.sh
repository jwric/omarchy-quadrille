#!/bin/bash
# Screenshot overlays (menu, OSD, toasts, pickers) of the running shell, over the
# wallpaper only: popup-shots.sh's way, for anything a command can open.
#
#   plugins/tools/overlay-shots.sh PREFIX 'MONITOR|LABEL|OPEN|CLOSE' ...
#
# Both outputs are parked on empty workspaces; for every spec the output is
# focused, OPEN runs (a shell command: `omarchy-menu summon apps`, `omarchy-osd
# -i volume-high -p 60 -d 4000`, `notify-send ...`), the output is grabbed as
# PREFIX-LABEL-MONITOR.png after WAIT seconds (default 1.6), CLOSE runs; at the
# end workspaces and focus are put back. A grab is thrown away unless every output
# showed an empty workspace just before and just after it.
#
# Nothing is typed or clicked. Run it under the shared lock and keep it short:
#   flock -w 900 /tmp/quadrille-live.lock plugins/tools/overlay-shots.sh ...
# Anything it opens has to be closed by CLOSE; notifications it sends are the
# caller's to delete from ~/.local/state/omarchy/notifications/history.
set -u
PFX=${1:?usage: overlay-shots.sh PREFIX 'MONITOR|LABEL|OPEN|CLOSE' ...}; shift
EMPTY_A=${EMPTY_A:-9}; EMPTY_B=${EMPTY_B:-8}
mons=$(hyprctl monitors -j)
names=$(python3 -c "import json,sys; print(' '.join(m['name'] for m in json.load(sys.stdin)))" <<<"$mons")
focused=$(python3 -c "import json,sys; print([m['name'] for m in json.load(sys.stdin) if m['focused']][0])" <<<"$mons")
win=$(hyprctl activewindow -j | python3 -c "import json,sys; print(json.load(sys.stdin).get('address',''))")
declare -A ws
for n in $names; do ws[$n]=$(python3 -c "import json,sys; print([m for m in json.load(sys.stdin) if m['name']=='$n'][0]['activeWorkspace']['id'])" <<<"$mons"); done
d() { hyprctl dispatch "$1" >/dev/null; sleep 0.25; }
empty() {
  python3 - <<'PY'
import json, subprocess, sys
mons = json.loads(subprocess.check_output(["hyprctl", "monitors", "-j"]))
wss = {w["id"]: w for w in json.loads(subprocess.check_output(["hyprctl", "workspaces", "-j"]))}
for m in mons:
    w = wss.get(m["activeWorkspace"]["id"])
    if w is None or w["windows"] != 0 or m["specialWorkspace"]["id"] != 0:
        sys.exit(1)
PY
}
grab() { if empty; then grim -o "$1" "$2"; fi; if ! empty; then rm -f "$2"; echo "DISCARDED $2 (a workspace was not empty)"; fi; }
restore() {
  for n in $names; do d "hl.dsp.focus({ monitor = \"$n\" })"; d "hl.dsp.focus({ workspace = \"${ws[$n]}\" })"; done
  d "hl.dsp.focus({ monitor = \"$focused\" })"
  [ -n "$win" ] && d "hl.dsp.focus({ window = \"address:$win\" })"
}
trap restore EXIT
i=0
for n in $names; do
  d "hl.dsp.focus({ monitor = \"$n\" })"
  d "hl.dsp.focus({ workspace = \"$([ $i = 0 ] && echo $EMPTY_A || echo $EMPTY_B)\" })"
  i=$((i + 1))
done
sleep 0.4
for spec in "$@"; do
  IFS='|' read -r mon label open close <<<"$spec"
  d "hl.dsp.focus({ monitor = \"$mon\" })"
  sleep 0.3
  bash -c "$open" >/dev/null 2>&1
  sleep "${WAIT:-1.6}"
  grab "$mon" "$PFX-$label-$mon.png"
  [ -n "$close" ] && bash -c "$close" >/dev/null 2>&1
  sleep 0.4
done
