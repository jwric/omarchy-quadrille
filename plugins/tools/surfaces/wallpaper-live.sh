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
hyprctl workspaces -j > "$WORK/workspaces.json"
python3 "$HERE/wallpaper_live.py" plan "$WORK"
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
python3 "$HERE/wallpaper_live.py" guard "$WORK"
dbus-run-session --config-file "$WORK/bus.conf" -- env H_BACKGROUND="file:$WORK/background/Background.qml" \
  QT_QPA_PLATFORM=wayland QT_NO_XDG_DESKTOP_PORTAL=1 QT_ACCESSIBILITY=0 NO_AT_BRIDGE=1 QT_QPA_PLATFORMTHEME=generic \
  quickshell -p "$WORK/root" > "$OUT/preview.log" 2>&1 &
shell_pid=$!
sleep 3
# Every actual output must be empty immediately before and after each grab.
python3 "$HERE/wallpaper_live.py" capture "$WORK" "$OUT"
echo "wallpaper preview captured; workspaces restored on exit"
