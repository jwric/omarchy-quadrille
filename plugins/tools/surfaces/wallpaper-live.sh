#!/bin/bash
# A short, disposable preview over empty live workspaces, after nested checks.
# Does not install a plugin, restart the shell or alter its configuration.
#   flock -o -w 900 /tmp/quadrille-live.lock plugins/tools/surfaces/wallpaper-live.sh OUT
set -euo pipefail
for fd in 3 4 5 6 7 8 9; do eval "exec $fd>&-"; done
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(readlink -f "$HERE/../../..")
OUT=${1:-/tmp/quadrille-wallpaper-live}
mkdir -p "$OUT"
OUT=$(readlink -f "$OUT")
WORK=$(mktemp -d)
restore() {
  if [[ -f $WORK/restore.sh ]]; then bash "$WORK/restore.sh" >/dev/null 2>&1 || true; fi
  while read -r child; do kill "$child" 2>/dev/null || true; done < <(pgrep -f "[q]uickshell -p $WORK/root" || true)
  if [[ -n ${shell_pid:-} ]]; then kill "$shell_pid" 2>/dev/null || true; wait "$shell_pid" 2>/dev/null || true; fi
  rm -rf "$WORK"
}
trap restore EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
hyprctl monitors -j > "$WORK/monitors.json"
hyprctl clients -j | python3 -c 'import json,sys; assert all(w.get("workspace",{}).get("id") not in (8,9) for w in json.load(sys.stdin)), "workspaces 8/9 are occupied"'
python3 - "$WORK" <<'PY'
import json,re,sys
from pathlib import Path
root=Path(sys.argv[1]); monitors=json.loads((root/'monitors.json').read_text())
assert 1<=len(monitors)<=2, 'preview expects one or two outputs'
focus=next((m['name'] for m in monitors if m.get('focused')),monitors[0]['name'])
show=[]; restore=[]; captures=[]
for index,m in enumerate(monitors):
 name=m['name']; assert re.fullmatch(r'[A-Za-z0-9_-]+',name)
 workspace=m['activeWorkspace']['id']; assert workspace>0
 def command(field,value):
  return "hyprctl dispatch 'hl.dsp.focus({ "+field+' = '+json.dumps(str(value))+" })' >/dev/null"
 show += [command('monitor',name), command('workspace',8+index)]
 restore += [command('monitor',name),command('workspace',workspace)]
 captures.append({'name':name,'width':m['width'],'height':m['height'],'phys':max(1,round(2*m['scale']))})
restore.append(command('monitor',focus))
(root/'show.sh').write_text('\n'.join(show)+'\n');(root/'restore.sh').write_text('\n'.join(restore)+'\n')
(root/'captures.json').write_text(json.dumps(captures))
PY
mkdir -p "$WORK/root"
for name in Commons Ui services; do ln -s "/usr/share/omarchy/shell/$name" "$WORK/root/$name"; done
cp -rL "$REPO/plugins/quadrille.background" "$WORK/background"
python3 - "$WORK/background/Background.qml" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]);s=p.read_text()
s=s.replace('target: "background"','target: "quadrille-wallpaper-preview"')
s=s.replace('namespace: "omarchy-background"','namespace: "quadrille-wallpaper-preview"')
s=s.replace('color: "transparent"','color: "transparent"\n      mask: Region {}',1)
p.write_text(s)
PY
cat > "$WORK/root/shell.qml" <<'QML'
import QtQuick
import Quickshell
ShellRoot { Loader { source: Quickshell.env("H_BACKGROUND") } }
QML
cat > "$WORK/bus.conf" <<'XML'
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><auth>EXTERNAL</auth>
<policy context="default"><allow send_destination="*"/><allow eavesdrop="true"/><allow own="*"/></policy></busconfig>
XML
bash "$WORK/show.sh"
dbus-run-session --config-file "$WORK/bus.conf" -- env H_BACKGROUND="file:$WORK/background/Background.qml" \
  QT_QPA_PLATFORM=wayland QT_NO_XDG_DESKTOP_PORTAL=1 QT_ACCESSIBILITY=0 NO_AT_BRIDGE=1 QT_QPA_PLATFORMTHEME=generic \
  quickshell -p "$WORK/root" > "$OUT/preview.log" 2>&1 &
shell_pid=$!
sleep 3
# Recheck immediately before capture: never retain a shot with any window text.
hyprctl clients -j | python3 -c 'import json,sys; assert all(w.get("workspace",{}).get("id") not in (8,9) for w in json.load(sys.stdin)), "a window appeared on preview workspace"'
python3 - "$WORK/captures.json" "$WORK" "$OUT" <<'PY'
import json,subprocess,sys
from pathlib import Path
from PIL import Image
for m in json.loads(Path(sys.argv[1]).read_text()):
 raw=Path(sys.argv[2])/f'{m["name"]}.png'
 subprocess.run(['grim','-o',m['name'],str(raw)],check=True)
 image=Image.open(raw)
 # Only wallpaper: discard the whole live bar, including workspace/app/status text.
 top=20*m['phys']
 image.crop((0,top,image.width,image.height)).save(Path(sys.argv[3])/f'live-{m["name"]}.png')
PY
if ! hyprctl clients -j | python3 -c 'import json,sys; assert all(w.get("workspace",{}).get("id") not in (8,9) for w in json.load(sys.stdin))'; then
  rm -f "$OUT"/live-*.png
  exit 1
fi
echo "wallpaper preview captured; workspaces restored on exit"
