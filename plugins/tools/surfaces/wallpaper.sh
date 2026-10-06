#!/bin/bash
# The real background clone in a private nested session. No live input or
# user configuration is read or changed; QA/QB get synthetic EDID metadata.
#   flock -o -w 900 /tmp/quadrille-live.lock plugins/tools/surfaces/wallpaper.sh OUT
set -euo pipefail
for fd in 3 4 5 6 7 8 9; do eval "exec $fd>&-"; done
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(readlink -f "$HERE/../../..")
OUT=${1:-/tmp/quadrille-wallpaper}
mkdir -p "$OUT"
OUT=$(readlink -f "$OUT")
WORK=$(mktemp -d)
export QUADRILLE_NESTED_DIR=$WORK/nested
N=$REPO/layershell/tools/nested.sh
cleanup() {
  while read -r child; do kill "$child" 2>/dev/null || true; done < <(pgrep -f "[q]uickshell -p $WORK/root" || true)
  if [[ -n ${shell_pid:-} ]]; then kill "$shell_pid" 2>/dev/null || true; wait "$shell_pid" 2>/dev/null || true; fi
  "$N" down >/dev/null 2>&1 || true
  rm -rf "$WORK"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
mkdir -p "$WORK/root" "$WORK/home/.local/state/omarchy/current"
for name in Commons Ui services; do ln -s "/usr/share/omarchy/shell/$name" "$WORK/root/$name"; done
ln -s "$REPO/themes/quadrille-terminal" "$WORK/home/.local/state/omarchy/current/theme"
ln -s "$REPO/themes/quadrille-terminal/backgrounds/1-graticule.png" "$WORK/home/.local/state/omarchy/current/background"
cp -rL "$REPO/plugins/quadrille.background" "$WORK/background"
python3 - "$WORK/background" <<'PY'
from pathlib import Path
import sys
root=Path(sys.argv[1])
p=root/'Background.qml'; s=p.read_text()
s=s.replace('out.push(object)', '''out.push(Object.assign({}, object, {
        physicalWidth: object.name === "QA" ? 340 : 800,
        physicalHeight: object.name === "QA" ? 220 : 330,
        x: object.name === "QA" ? 0 : -952, y: object.name === "QA" ? 0 : -1440 }))''')
s=s.replace('object.scale > 0)', 'object.scale > 0 && (object.name === "QA" || object.name === "QB"))')
s=s.replace('model: Quickshell.screens', 'model: Quickshell.screens.filter(function(s) { return s.name === "QA" || s.name === "QB" })')
p.write_text(s)
p=root/'Graticule.qml'; s=p.read_text()
s=s.replace('onPaint: Drafting.paint(', 'onPaint: { console.log("WALLPAPER_FRAME", root.monitor.name, root.visible, Math.round(width * root.dpr), Math.round(height * root.dpr), root.physical.widthPx, root.physical.heightPx, root.physical.pixelsPerVpx); Drafting.paint(')
s=s.replace('root.software, root.dpr)', 'root.software, root.dpr) }')
p.write_text(s)
PY
cat > "$WORK/root/shell.qml" <<'QML'
import Quickshell
import QtQuick
ShellRoot { Loader { source: Quickshell.env("H_BACKGROUND") } }
QML
cat > "$WORK/bus.conf" <<'XML'
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><auth>EXTERNAL</auth>
<policy context="default"><allow send_destination="*"/><allow eavesdrop="true"/><allow own="*"/></policy></busconfig>
XML
if [[ ${WALLPAPER_HEADLESS:-0} == 1 ]]; then
  env -u HYPRLAND_INSTANCE_SIGNATURE -u WAYLAND_DISPLAY HOME="$WORK/home" H_BACKGROUND="file:$WORK/background/Background.qml" QT_QPA_PLATFORM=offscreen \
    QT_NO_XDG_DESKTOP_PORTAL=1 QT_ACCESSIBILITY=0 NO_AT_BRIDGE=1 timeout 3 quickshell -p "$WORK/root" > "$OUT/wrapper.log" 2>&1 || [[ $? == 124 ]]
  ! rg 'is not a type|Type .* unavailable|Cannot assign|SyntaxError' "$OUT/wrapper.log"
  exit
fi
"$N" up > "$OUT/nested.log" 2>&1
"$N" run dbus-run-session --config-file "$WORK/bus.conf" -- env HOME="$WORK/home" H_BACKGROUND="file:$WORK/background/Background.qml" QT_QPA_PLATFORM=wayland \
  QT_NO_XDG_DESKTOP_PORTAL=1 QT_ACCESSIBILITY=0 NO_AT_BRIDGE=1 QT_QPA_PLATFORMTHEME=generic \
  quickshell -p "$WORK/root" > "$OUT/gpu.log" 2>&1 &
shell_pid=$!
sleep 4
pointer_output=$("$N" ctl monitors -j | python3 -c 'import json,sys; print(next(m["name"] for m in json.load(sys.stdin) if m["name"] not in ("QA", "QB")))')
"$N" run "$REPO/layershell/target/release/vptr" "$pointer_output" move 10 10 sleep 100
"$N" run grim -o QA "$OUT/gpu-laptop.png"
"$N" run grim -o QB "$OUT/gpu-ultrawide.png"
while read -r child; do kill "$child" 2>/dev/null || true; done < <(pgrep -f "[q]uickshell -p $WORK/root" || true)
kill "$shell_pid" 2>/dev/null || true
wait "$shell_pid" 2>/dev/null || true
shell_pid=""
"$N" down > /dev/null
python3 "$REPO/layershell/tools/crisp.py" "$OUT/gpu-laptop.png" 0,0,2560,1600 3 laptop --check
python3 "$REPO/layershell/tools/crisp.py" "$OUT/gpu-ultrawide.png" 0,0,3440,1440 2 ultrawide --check
python3 - "$OUT/gpu.log" <<'PY'
import re,sys
seen={}
for line in open(sys.argv[1]):
 m=re.search(r'WALLPAPER_FRAME (QA|QB) true (\d+) (\d+) (\d+) (\d+) (\d+)',line)
 if m:
  name=m[1]; actual=tuple(map(int,m.groups()[1:]))
  expected=(2560,1600,2560,1600,3) if name=='QA' else (3440,1440,3440,1440,2)
  assert actual==expected,(name,actual,expected)
  seen[name]=actual
assert len(seen)==2,seen
print('wallpaper: every visible paint has final buffer geometry',seen)
PY
