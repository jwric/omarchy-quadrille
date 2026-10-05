#!/bin/bash
# Install the quadrille themes and fonts for the current user.
#
#   tools/install.sh            link the themes, install the fonts
#
# The themes are symlinked, so regenerating them (tools/gen_themes.py) is all it
# takes to see a change. Applying one is `omarchy theme set quadrille-terminal`.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
quadrille=${QUADRILLE:-$root/../graticule}

mkdir -p "$HOME/.config/omarchy/themes"
for theme in "$root"/themes/quadrille-*/; do
  ln -sfn "${theme%/}" "$HOME/.config/omarchy/themes/$(basename "$theme")"
done

fonts=$HOME/.local/share/fonts/departure-mono
mkdir -p "$fonts"
cp -u "$quadrille"/crates/quadrille/fonts/Departure*.{otf,ttf} "$fonts"/
fc-cache -f "$fonts"

echo "themes: $(ls -d "$root"/themes/quadrille-*/ | xargs -n1 basename | tr '\n' ' ')"
fc-list | grep -i "departure" | sed 's/^/font: /'

mkdir -p "$HOME/.config/fontconfig/conf.d"
cp "$root"/fontconfig/*.conf "$HOME/.config/fontconfig/conf.d/"
fc-cache -f
