# Resuming the work

Written at a pause (2026-10-05, evening). Read this, then `plugins/NOTES.md` and
`layershell/NOTES.md` (each has a "Status at pause" section from the agent that
owned that slice), then `git log --oneline`.

## What exists (all committed; GitHub: jwric/omarchy-quadrille, private)

1. **Themes and font** (`themes/`, `fontconfig/`, `tools/`): five themes from
   quadrille's palettes, applied live ("Quadrille Terminal"), Departure Mono as the
   system monospace with antialiasing off.
2. **QML shell plugins** (`plugins/`): kit in `plugins/quadrille.bar/Q/`, bar,
   menu, OSD, notifications, sysmon gauges; per-screen pixel unit; baked bitmap
   text; `plugins/stock.sh` returns to the stock shell.
3. **Panel host** (`layershell/`): `quadrille-bar`, own sctk windowing shell under
   the iced fork; `--no-bar` service mode; `sysmon` panel; nested-Hyprland tests.
   Installed in `~/.local/bin` (started by hand for the session, not autostarted).

## The round that was in flight

The user asked: "fix the incomplete things: icons, bad layout, overlapping, pixel
perfectness, everything else". Four slices were running in parallel:

| slice | owns | goal |
|---|---|---|
| P | bar popup panels (audio, bluetooth, network, monitor, power, weather, clock, tailscale, agents) as restyled clones; `tools/gen_themes.py` font tokens | stop popups overlapping (the weather popup's temperature overlapped its label because the theme pins heading/display at 2x/3x body) and restyle them in the quadrille language |
| B | `plugins/quadrille.bar` and the kit `Q/` | tray / indicators / weather / keyboard / update icons as pixel sprites, app icons pixel-art, bar layout and overlap on both monitors, remaining pixel exactness |
| O | menu, OSD, notifications, overlays (clipboard, emojis, image picker, reminders, lock, polkit), app icons, wallpaper | audit every state, restyle the rest, exact wallpaper on both monitors; lock/polkit only if testable safely, else shipped disabled |
| H | `layershell/` | real iced panels: audio, network, bluetooth, power (state-changing commands tested against stubs only) |

Each wrote its own "Status at pause" section when told to pause. To resume, give a
fresh agent the section's "next steps", plus the rules below.

## Rules every agent worked under (keep them)

- **Live-session lock.** Several agents share the user's desktop. Wrap every
  live-session sequence (shell restart, opening a popup/menu, workspace switch,
  screenshot, nested-compositor run) in
  `flock -w 900 /tmp/quadrille-live.lock bash -c '...'`, short, never held while
  thinking; close what you opened, restore the user's workspace and focus.
- **No input injection** into the live session (no wtype / virtual pointer /
  keyboard / clicking dispatches). Input is tested only in the nested Hyprland
  (`layershell/tools/nested.sh`). A past agent typed into the user's windows once.
- **Nothing private in the repo.** Screenshots only over an empty workspace with
  wallpaper behind, tightly cropped, each PNG looked at. History was rewritten once
  before the first push to drop screenshots that showed private windows.
- Never edit `/usr/share`. Never edit `~/.config/hypr` or other Hyprland/uwsm
  config: the auto-mode classifier blocks it even with the user's say-so; the user
  pastes those lines themselves.
- Keep the shell alive (`omarchy-shell shell ping`), keep `plugins/stock.sh` a full
  way back, disable anything half-done rather than leave it broken. Never leave a
  lock-screen or polkit clone enabled unless it was verified safe.
- Commits: plain messages, **no Co-Authored-By or Claude/AI trailer** (the user's
  standing rule). Commit only your own paths. Push is the lead's, at the end.

## Waiting on the user

- Paste the Hyprland edits (autostart `quadrille-bar --no-bar`, binding
  `SUPER + CTRL + M` for `quadrille-bar ctl toggle sysmon`, and
  `hl.env("QML_DISABLE_DISTANCEFIELD", "1")` for crisp shell text). Without the
  autostart the host is gone after a logout.
- Laptop scale stays 1.666667 (1.5 is invalid there; the per-screen unit makes the
  grid exact at 3 physical px). Nothing to change.
- Terminal font size is not tuned (ghostty/alacritty at size 9; Departure Mono
  native cell is 7 x 14 at 11 px). Needs a decision, and the edit is the user's.
- Visibility of the GitHub repo (private now). Making it public needs the
  `quadrille` repo public first (the host depends on it by git), and a review.

## After the agents report (lead's checklist)

1. `git log --oneline` and `git status`: everything committed, no trailers
   (`git log --format=%B | grep -ci 'co-authored\|claude'` must be 0).
2. `omarchy-shell shell ping`; look at both monitors over an empty workspace.
3. Rebuild and reinstall the host if it changed: `layershell/tools/install.sh`
   (then restart the running host: `quadrille-bar ctl quit`, start it again).
4. Re-run the nested tests (`layershell/tools/nested-test.sh`) and the Rust tests
   (`cd layershell && cargo test --workspace`).
5. Scan for private strings before every push (the repo is checked for secrets and
   personal data; keep screenshots clean), then `git push`.
6. Update the README ("What is not exact") to match reality.

## Useful commands

```sh
omarchy-shell shell ping                  # the shell is alive
omarchy-restart-shell                     # restart it (under the flock)
omarchy-theme-set "Quadrille Terminal"    # re-apply the theme (check the bar after)
plugins/install.sh | plugins/stock.sh     # enable the quadrille plugins | back to stock
quadrille-bar ctl list | toggle sysmon    # the panel host
python3 tools/gen_themes.py               # regenerate the themes (QUADRILLE=/path if needed)
```

Back to how it was before any of this: `plugins/stock.sh`,
`omarchy theme set ristretto`, `omarchy font set "CaskaydiaMono Nerd Font"`, delete
`~/.config/fontconfig/conf.d/60-quadrille-pixel.conf`.
