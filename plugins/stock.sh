#!/bin/bash
# Put the stock Omarchy shell back: the stock bar, menu, OSD and notifications,
# and no quadrille widgets in the bar. The plugin links stay in place (disabled);
# `plugins/stock.sh --unlink` removes them.
set -euo pipefail

dest=$HOME/.config/omarchy/plugins

# A clone's disable puts its built-in source back.
for id in quadrille.sysmon quadrille.menu quadrille.osd quadrille.notifications quadrille.audio quadrille.power quadrille.background quadrille.reminders quadrille.emojis quadrille.clipboard quadrille.image-picker quadrille.lock quadrille.bluetooth quadrille.network quadrille.monitor quadrille.weather quadrille.clock quadrille.agents quadrille.tailscale quadrille.lab; do
  omarchy plugin disable "$id" >/dev/null 2>&1 || true
done
omarchy plugin enable omarchy.bar

if [[ ${1:-} == --unlink ]]; then
  for id in quadrille.bar quadrille.osd quadrille.notifications quadrille.menu quadrille.sysmon quadrille.audio quadrille.power quadrille.background quadrille.reminders quadrille.emojis quadrille.clipboard quadrille.image-picker quadrille.lock quadrille.bluetooth quadrille.network quadrille.monitor quadrille.weather quadrille.clock quadrille.agents quadrille.tailscale quadrille.lab; do
    [[ -L $dest/$id ]] && rm "$dest/$id"
  done
  omarchy-shell shell rescanPlugins >/dev/null
fi

omarchy-restart-shell
