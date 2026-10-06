#!/bin/bash
# overlays-build.sh: build.sh's scratch shell, plus the overlay clones (menu, OSD, notifications,
# emojis, reminders, image picker; not the clipboard, whose startup kills the live shell's watchers
# by name) and the stock plugins they replace, so they can be opened in a nested compositor.
#   SURF_WORK=DIR plugins/tools/surfaces/overlays-build.sh
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
S=${SURF_WORK:-${TMPDIR:-/tmp}/quadrille-surfaces}
R=$S/rep
"$HERE/build.sh" >/dev/null
for p in menu osd notifications emojis reminders image-picker; do
  ln -sfn /usr/share/omarchy/shell/plugins/$p $R/omarchy/shell/plugins/$p
done
for n in menu osd notifications emojis reminders image-picker; do
  ln -sfn "$HERE/../../quadrille.$n" $R/home/.config/omarchy/plugins/quadrille.$n
done
python3 - "$R/home/.config/omarchy/shell.json" <<'PY'
import json, sys
p = sys.argv[1]
c = json.load(open(p))
c["plugins"] = [{"id": "quadrille." + n} for n in ("menu", "osd", "notifications", "emojis", "reminders", "image-picker")]
c["disabledPlugins"] = ["omarchy." + n for n in ("menu", "osd", "notifications", "emojis", "reminders", "image-picker")]
json.dump(c, open(p, "w"), indent=2)
PY
echo "overlays ready in $R"
