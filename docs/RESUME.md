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

## State at the pause (what each slice reported)

All four slices stopped cleanly; the shell answered `omarchy-shell shell ping`
and the quadrille bar was active. Each wrote a "Status at pause" section with
next steps (`plugins/NOTES.md`: agents B, O, P; `layershell/NOTES.md`).

- **P (popups, theme tokens).** Done: the theme fix for overlapping stock popups
  (the cause was spacing not scaled with the 22 px type, not only the font pins):
  `[spacing] scale = 2.0`, `scale-with-font = false`, heading 22, display-large 44,
  icon-large 33. Stock audio, bluetooth, network, monitor, power, weather, clock and
  agents popups were checked on both monitors; the final regenerated file was only
  re-checked on the bar, and tailscale was not checked. The popup kit (QPopup, QRow,
  QSlider, ...) and the `quadrille.audio` and `quadrille.power` clones are enabled and
  pixel-crisp. **Not started:** bluetooth, network, monitor, tailscale, agents,
  weather and clock clones (brief in `plugins/tools/popup-brief.md`).
- **B (bar, kit).** Round-3 files committed. **The pixel tray is not finished:**
  `pixelTray: false` in `plugins/quadrille.bar/Bar.qml` keeps the stock tray. With
  fake tray items registered (`plugins/tools/fake_sni.py`) the shell stopped
  answering IPC until they exited: investigate before enabling it. Pitfalls in NOTES.
- **O (menu, OSD, toasts, icons, wallpaper).** Done: menu empty-state overlap,
  readable selected sub-text, icon boxes, placeholder icons for missing app icons,
  pixel Confirm; OSD touchpad/touch/download icons and two-line messages; toasts
  placed under the bar's real height; kit pieces (Pictograms, AppIcon, Confirm,
  Scrim). `quadrille.background` draws the wallpaper at each output's pixel grid.
  **Not done:** reminders, emojis, clipboard, image picker clones; no lock or polkit
  clone exists (do not enable one without a safe test). `AppIcon` wraps B's shader
  `PixelIcon` and is unverified on the real GPU.
- **H (iced panels).** `audio`, `network`, `bluetooth`, `power` panels are built and
  registered beside `sysmon` (release build clean, 98 unit tests pass; nested tests
  pass for audio and network with stubbed commands). Bluetooth has 2 failing keyboard
  checks (probably the test's row counting); the power and look test sections have
  not run; idle numbers and the registry tests remain. **Not installed:** the
  user's `~/.local/bin/quadrille-bar` is still the earlier build, and the running host
  is that one.
- Housekeeping done by the lead: `plugins/install.sh` and `stock.sh` now include the
  audio, power and background clones, so `stock.sh` is a full way back again.
- Side effects to know about: test notifications were created and deleted from the
  notification history, but rotation had already dropped two older entries; one agent
  photographed the user's workspace while checking and deleted the file.

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
