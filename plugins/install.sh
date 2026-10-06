#!/bin/bash
# Link the quadrille shell plugins into ~/.config/omarchy/plugins and enable them.
#
#   plugins/install.sh              the bar, OSD, notifications, menu, system gauges, the popup panels
#                                   (audio, power, bluetooth, network, display, weather, clock, agents:
#                                   each only where the stock widget is in the bar; tailscale is linked
#                                   and left off), the wallpaper and the overlays
#   plugins/install.sh bar          just the bar
#   plugins/install.sh lab          also link the specimen sheet (not enabled)
#
# Restarts the shell at the end: the notification clone is a daemon, and a
# daemon is only swapped cleanly at startup.
#
# Back to the stock shell: plugins/stock.sh
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
dest=$HOME/.config/omarchy/plugins
mkdir -p "$dest"

want=${1:-all}
case $want in
  bar) ids=(quadrille.bar) ;;
  lab) ids=(quadrille.bar quadrille.osd quadrille.notifications quadrille.menu quadrille.sysmon quadrille.audio quadrille.power quadrille.background quadrille.reminders quadrille.emojis quadrille.clipboard quadrille.image-picker quadrille.bluetooth quadrille.network quadrille.monitor quadrille.weather quadrille.clock quadrille.agents quadrille.tailscale quadrille.lab) ;;
  *)   ids=(quadrille.bar quadrille.osd quadrille.notifications quadrille.menu quadrille.sysmon quadrille.audio quadrille.power quadrille.background quadrille.reminders quadrille.emojis quadrille.clipboard quadrille.image-picker quadrille.bluetooth quadrille.network quadrille.monitor quadrille.weather quadrille.clock quadrille.agents quadrille.tailscale) ;;
esac

for id in "${ids[@]}"; do
  ln -sfn "$here/$id" "$dest/$id"
done

omarchy-shell shell rescanPlugins >/dev/null
sleep 3

# A popup clone takes the place of the stock widget in the bar, so it is enabled
# only where the stock widget is: enabling one that is not there would add a
# widget to the bar. Tailscale is linked and never enabled for that reason
# (`omarchy plugin enable quadrille.tailscale` adds it).
in_bar() {
  local cfg=$HOME/.config/omarchy/shell.json
  [[ -f $cfg ]] || return 0
  jq -e --arg id "$1" '[.bar.layout[]?[]? | (if type == "object" then .id else . end)] | index($id) != null' "$cfg" >/dev/null 2>&1
}

for id in "${ids[@]}"; do
  case $id in
    quadrille.lab|quadrille.tailscale) continue ;;
    quadrille.bluetooth|quadrille.network|quadrille.monitor|quadrille.weather|quadrille.clock|quadrille.agents)
      in_bar "omarchy.${id#quadrille.}" || in_bar "$id" || continue ;;
  esac
  if [[ $id == quadrille.sysmon ]]; then
    omarchy plugin enable "$id" --section right --before omarchy.bluetooth ||
      omarchy plugin enable "$id" --section right
  else
    omarchy plugin enable "$id"
  fi
done

omarchy-restart-shell
