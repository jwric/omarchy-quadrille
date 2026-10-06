#!/bin/bash
# Render a plugin's component offscreen, at a chosen output scale, without touching
# the live session: no Wayland, no layer-shell, no D-Bus names.
#
#   plugins/tools/offscreen/run.sh PLUGIN_DIR ENTRY.qml SCALE 'STEPS_JSON' OUTDIR
#
# STEPS_JSON is a list of {"wait":ms} {"call":"fn","args":[..]} {"set":{..}}
# {"eval":"js"} {"grab":"name"} (see shell.qml). Environment: H_W / H_H the window
# size (default: the laptop's 1536x960 logical at 1.666667, else 3440x1440),
# H_TIMEOUT seconds (25), H_EXTRA a test .qml copied beside the plugin's files,
# H_REAL_ICONS=1 keeps the kit's real PixelIcon (a GPU shader; offscreen it draws
# nothing: the Qt platform renders in software).
#
# A throwaway copy of the plugin is made in which `PanelWindow` becomes a plain
# window (`Stage`) whose content can be grabbed, and the kit's PixelIcon is
# replaced by a plain nearest-neighbour picture. Everything else is the real
# code. QT_SCALE_FACTOR gives the fractional device pixel ratio, grabs come out
# at device resolution. Needs: quickshell, python3 not at all.
set -u
here=$(cd "$(dirname "$0")" && pwd)
plug=$(readlink -f "$1"); entry=$2; scale=${3:-1.666667}; steps=${4:-[]}; out=${5:-$PWD/offscreen-out}
case $scale in 1|1.0) W=3440; H=1440;; *) W=1536; H=960;; esac
W=${H_W:-$W}; H=${H_H:-$H}
shell=${OMARCHY_PATH:-/usr/share/omarchy}/shell
mkdir -p "$out"
# The config root holds the driver and the host's own modules, and nothing that
# changes while it runs (a change in the root reloads Quickshell).
root=$(mktemp -d); b=$(mktemp -d)
trap 'rm -rf "$root" "$b"' EXIT
cp "$here/shell.qml" "$root/"
for d in Commons Ui services; do ln -s "$shell/$d" "$root/$d"; done
mkdir "$b/plugin"
for f in "$plug"/*; do
  n=$(basename "$f")
  if [[ $n == *.qml ]] && grep -q "PanelWindow\|WlrLayershell" "$f"; then
    sed -E \
      -e "s/\bPanelWindow \{/Stage { implicitWidth: $W; implicitHeight: $H;/" \
      -e '/^\s*(anchors \{ top|WlrLayershell\.|exclusionMode|mask:|screen:)/d' \
      -e '/^\s*margins \{/d' \
      -e 's/"pkill"/"true"/; s/"wl-paste"/"true"/; s#clipboard-history.json#clipboard-history.offscreen.json#' \
      "$f" > "$b/plugin/$n"
  elif [ -d "$f" ] && [ ! -L "$f" ] || [ -L "$f" ] && [ -d "$(readlink -f "$f")" ]; then
    cp -rL "$f" "$b/plugin/$n"
  else
    cp -L "$f" "$b/plugin/$n"
  fi
done
# components/ and friends: the same swap in nested files
find "$b/plugin" -mindepth 2 -name '*.qml' | while read -r f; do
  if grep -q "PanelWindow\|WlrLayershell" "$f"; then
    sed -E -i -e "s/\bPanelWindow \{/Stage { implicitWidth: $W; implicitHeight: $H;/" \
      -e '/^\s*(anchors \{ top|WlrLayershell\.|exclusionMode|mask:|screen:)/d' -e '/^\s*margins \{/d' "$f"
  fi
done
# the plugin's Q link resolves to the kit; the kit in the copy has PixelIcon stubbed
if [ -d "$b/plugin/Q" ] && [ -z "${H_REAL_ICONS:-}" ]; then
cat > "$b/plugin/Q/PixelIcon.qml" <<'QML'
import QtQuick
import "."
Item {
  id: root
  readonly property var g: Px.of(root)
  property string source: ""
  property int cells: 11
  property int levels: 4
  property bool silhouette: false
  property color ink: Role.ink
  property bool dimmed: false
  readonly property bool ready: img.status === Image.Ready
  readonly property bool shaderFailed: false
  implicitWidth: cells * g.unit; implicitHeight: cells * g.unit; width: implicitWidth; height: implicitHeight
  Image { id: img; anchors.fill: parent; source: root.source; sourceSize.width: Math.round(root.width * root.g.dpr); sourceSize.height: Math.round(root.height * root.g.dpr); fillMode: Image.PreserveAspectFit; smooth: false; opacity: root.dimmed ? 0.4 : 1 }
}
QML
fi
if [ -d "$b/plugin/Q" ] && [ -z "${H_REAL_ICONS:-}" ]; then
cat > "$b/plugin/Q/PixelEmoji.qml" <<'QML'
import QtQuick
import "."
Item {
  id: root
  readonly property var g: Px.of(root)
  property string text: ""
  property int cells: 12
  property int levels: 6
  readonly property bool ready: true
  implicitWidth: cells * g.unit; implicitHeight: cells * g.unit; width: implicitWidth; height: implicitHeight
  Text { anchors.centerIn: parent; text: root.text; font.family: "Noto Color Emoji"; font.pixelSize: Math.round(root.height * 0.8) }
}
QML
fi
cat > "$b/plugin/Stage.qml" <<'QML'
import QtQuick
import Quickshell
FloatingWindow {
  id: stageWin
  default property alias stageData: stage.data
  property alias stage: stage
  Item { id: stage; anchors.fill: parent }
}
QML
if [ -n "${H_EXTRA:-}" ]; then
  sed -E -e "s/\bPanelWindow \{/Stage { implicitWidth: $W; implicitHeight: $H;/" "$H_EXTRA" > "$b/plugin/$(basename "$H_EXTRA")"
fi
cd "$root"
env -u WAYLAND_DISPLAY -u HYPRLAND_INSTANCE_SIGNATURE QT_QPA_PLATFORM=offscreen QT_SCALE_FACTOR=$scale \
  OMARCHY_PATH=${OMARCHY_PATH:-/usr/share/omarchy} H_SRC="file:$b/plugin/$entry" H_STEPS="$steps" H_OUT="$out" \
  timeout -k 2 "${H_TIMEOUT:-25}" quickshell -p . 2>&1 | stdbuf -oL grep --line-buffered -v "INFO\|window masks"
