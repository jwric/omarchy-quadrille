#!/bin/bash
# Build quadrille-bar and quadrille-screensaver and install them for this
# user: ~/.local/bin, no sudo, no system path, with the screensaver's launcher
# linked beside them. It edits nothing else: the lines to add to the Hyprland
# config are printed, for you to put where you want them.
#
#   tools/install.sh              build and install
#   tools/install.sh --uninstall  remove the binaries
set -eu

HERE=$(cd "$(dirname "$0")/.." && pwd)
PREFIX=${QUADRILLE_PREFIX:-$HOME/.local}
BIN=$PREFIX/bin/quadrille-bar
SAVER=$PREFIX/bin/quadrille-screensaver
LAUNCH=$PREFIX/bin/quadrille-screensaver-launch

if [ "${1:-}" = "--uninstall" ]; then
  # A running host keeps working until it quits; ask it to.
  "$BIN" ctl quit >/dev/null 2>&1 || true
  rm -f "$BIN" "$SAVER" "$LAUNCH"
  echo "removed $BIN, $SAVER and $LAUNCH"
  exit 0
fi

command -v cargo >/dev/null || { echo "install.sh: cargo is not installed" >&2; exit 1; }

echo "building quadrille-bar and quadrille-screensaver (the service profile: release, fat LTO, stripped; a minute or two)..."
cargo build --profile service --manifest-path "$HERE/Cargo.toml" -p quadrille-bar -p quadrille-screensaver

install -Dm755 "$HERE/target/service/quadrille-bar" "$BIN"
install -Dm755 "$HERE/target/service/quadrille-screensaver" "$SAVER"
ln -sfn "$HERE/../tools/quadrille-screensaver-launch" "$LAUNCH"

echo
echo "installed $BIN ($(du -h "$BIN" | cut -f1))"
echo "installed $SAVER ($(du -h "$SAVER" | cut -f1)) and $LAUNCH"

case ":$PATH:" in
  *":$PREFIX/bin:"*) ;;
  *) echo "note: $PREFIX/bin is not in PATH here; the lines below use the name, so it has to be in the session's PATH" ;;
esac

cat <<LINES

Nothing has been changed in your Hyprland or Omarchy configuration. To run the
panels beside the QML bar, add these two lines:

  # ~/.config/hypr/autostart.lua
  o.launch_on_start("quadrille-bar --no-bar")

  # ~/.config/hypr/bindings.lua   (pick the keys you like; SUPER + CTRL + M is free)
  o.bind("SUPER + CTRL + M", "System monitor", "quadrille-bar ctl toggle sysmon")

With --no-bar the host has no surface at all, no exclusive zone and no timer
until a panel is shown, and it opens panels on the focused output. Any other
menu action or script can run the same command:

  quadrille-bar ctl toggle sysmon      show it, or hide it if it is shown
  quadrille-bar ctl summon sysmon '{"output":"eDP-2"}'
  quadrille-bar ctl hide
  quadrille-bar ctl list

To start it now, without logging out:

  uwsm-app -- quadrille-bar --no-bar &

\`quadrille-bar ctl ...\` exits with 1 and says so when no host is running.

The screensaver runs as it is (any key, click or movement ends it):

  quadrille-screensaver-launch force

Omarchy's idle service starts the stock one. To have it start this one,
enable the idle clone (opt-in: it is also what locks the screen):

  plugins/install.sh idle         # back: plugins/stock.sh

and, for the menu's Screensaver entry, in ~/.config/omarchy/extensions/omarchy-menu.jsonc:

  "system.screensaver": {"icon":"󱄄","label":"Screensaver","action":"quadrille-screensaver-launch force"},
LINES
