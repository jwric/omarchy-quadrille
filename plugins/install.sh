#!/bin/bash
# Link the quadrille shell plugins into ~/.config/omarchy/plugins and enable them.
#
#   plugins/install.sh              the bar, OSD, notifications, menu, system gauges, the audio and
#                                   power popups, the wallpaper
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
  lab) ids=(quadrille.bar quadrille.osd quadrille.notifications quadrille.menu quadrille.sysmon quadrille.audio quadrille.power quadrille.background quadrille.reminders quadrille.emojis quadrille.clipboard quadrille.image-picker quadrille.lab) ;;
  *)   ids=(quadrille.bar quadrille.osd quadrille.notifications quadrille.menu quadrille.sysmon quadrille.audio quadrille.power quadrille.background quadrille.reminders quadrille.emojis quadrille.clipboard quadrille.image-picker) ;;
esac

for id in "${ids[@]}"; do
  ln -sfn "$here/$id" "$dest/$id"
done

omarchy-shell shell rescanPlugins >/dev/null
sleep 3

for id in "${ids[@]}"; do
  [[ $id == quadrille.lab ]] && continue
  if [[ $id == quadrille.sysmon ]]; then
    omarchy plugin enable "$id" --section right --before omarchy.bluetooth
  else
    omarchy plugin enable "$id"
  fi
done

omarchy-restart-shell
