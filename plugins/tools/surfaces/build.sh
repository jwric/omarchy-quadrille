#!/bin/bash
# build.sh : a copy of the live shell to try the bar in, without touching the live one.
# Own OMARCHY_PATH (the real shell's Commons, Ui and services linked, the stock bar,
# panels and agents plugins, no service plugins), own HOME (the real config, theme and
# icons linked; its own shell.json, with the layout of the real one, and its own state),
# the quadrille plugins from this tree. Run it on a private session bus
# (dbus-run-session) so its tray, notifications and so on are its own.
#   SURF_WORK=DIR plugins/tools/surfaces/build.sh
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
S=${SURF_WORK:-${TMPDIR:-/tmp}/quadrille-surfaces}
PLUGINS=$HERE/../..
R=$S/rep
rm -rf $R; mkdir -p $R/omarchy/shell/plugins $R/home/.config/omarchy/plugins $R/home/.local/state/omarchy/toggles $R/home/.cache
REAL=/usr/share/omarchy
for e in $REAL/*; do n=$(basename $e); [ $n = shell ] || ln -s $e $R/omarchy/$n; done
cp $REAL/shell/shell.qml $R/omarchy/shell/shell.qml
for d in Commons Ui services; do ln -s $REAL/shell/$d $R/omarchy/shell/$d; done
for p in bar panels agents; do ln -s $REAL/shell/plugins/$p $R/omarchy/shell/plugins/$p; done
for e in ~/.config/*; do n=$(basename $e); [ $n = omarchy ] || ln -s $e $R/home/.config/$n; done
for e in ~/.config/omarchy/*; do n=$(basename $e); case $n in shell.json|plugins) ;; *) ln -s $e $R/home/.config/omarchy/$n;; esac; done
for n in audio bluetooth clock monitor power sysmon weather bar agents network; do ln -s $PLUGINS/quadrille.$n $R/home/.config/omarchy/plugins/quadrille.$n; done
mkdir -p $R/home/.local; for e in ~/.local/share; do ln -s $e $R/home/.local/share; done
ln -s ~/.local/state/omarchy/current $R/home/.local/state/omarchy/current
touch $R/home/.local/state/omarchy/toggles/bar-off
python3 - <<PY
import json
c=json.load(open("$HOME/.config/omarchy/shell.json"))
c["plugins"]=[]; c.pop("disabledPlugins",None); c.pop("cloneSourceRestores",None)
c["bar"]["layout"]["right"]=[{"id":"omarchy.tray"},{"id":"quadrille.agents"},{"id":"quadrille.sysmon"},{"id":"quadrille.bluetooth"},{"id":"quadrille.network"},{"id":"quadrille.audio"},{"id":"quadrille.monitor"},{"id":"quadrille.power"}]
json.dump(c,open("$R/home/.config/omarchy/shell.json","w"),indent=2)
PY
echo ready $R
