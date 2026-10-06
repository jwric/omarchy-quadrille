#!/bin/bash
# Install the quadrille themes and fonts for the current user.
#
#   tools/install.sh            link the themes, install the fonts
#
# The themes are symlinked, so regenerating them (tools/gen_themes.py) is all it
# takes to see a change. Applying one is `omarchy theme set quadrille-terminal`.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
# quadrille's checkout: $QUADRILLE, or one beside this repository (it holds the fonts).
quadrille=${QUADRILLE:-}
for dir in "$root/../quadrille" "$root/../graticule"; do
  [[ -n $quadrille ]] && break
  [[ -d $dir/crates/quadrille/fonts ]] && quadrille=$dir
done
[[ -d $quadrille/crates/quadrille/fonts ]] || {
  echo "install.sh: quadrille's checkout not found; clone it beside this repository or set QUADRILLE=/path" >&2
  exit 1
}

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

# foot, with Departure Mono's size chosen per monitor (see tools/quadrille-foot). A
# user-level foot.desktop shadows the package's, so the terminal binding picks it up
# without any config edit; delete the two links to undo.
if command -v foot >/dev/null; then
  mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications"
  ln -sfn "$root/tools/quadrille-foot" "$HOME/.local/bin/quadrille-foot"
  ln -sfn "$root/terminal/foot.desktop" "$HOME/.local/share/applications/foot.desktop"
  echo "terminal: foot -> quadrille-foot"
fi
