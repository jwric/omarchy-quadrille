# Brief: restyle one Omarchy popup panel in the quadrille language

You are one of several agents finishing "omarchy-quadrille": making the Omarchy 4 desktop
shell (Quickshell / QML) look like quadrille, a pixel-perfect retro UI toolkit. Repo:
/home/jwric/dev/omarchy-quadrille (branch main; remote is private; DO NOT PUSH).

Your job: ONE bar popup panel (named in your task message) is cloned from the host's own
plugin and its view is redrawn with the popup kit, keeping every line of the stock
logic. Two panels are already done and are your reference: **plugins/quadrille.power**
(small) and **plugins/quadrille.audio** (sections, sliders, lists, scroll, keyboard cursor).
Read them first, side by side with the stock files in /usr/share/omarchy/shell/plugins/.

## Read first
1. /home/jwric/dev/graticule/DESIGN.md: quadrille's design language. A pixel is the unit; Departure
   Mono at native size; colours are roles; zones not cards; hairlines; inverse block for
   emphasis; corner brackets for focus/selection; groups `┌── NAME ──┐`; 7x7 icons;
   nothing tweens or fades; square corners; no shadows/blur/antialiasing; DROP, DON'T CUT
   (a label that does not fit is left out, it is never cut to a different label).
2. /home/jwric/dev/omarchy-quadrille/plugins/NOTES.md: what a plugin can and cannot do,
   the per-screen pixel grid, baked bitmap text, and the section "Popup panels (agent P)"
   at the end: the kit table and how a clone plugs in.
3. The kit, plugins/quadrille.bar/Q/*.qml: Px (the grid), Role (colours), PixelText,
   PixelParagraph, BigText, Sprite, Sprites (bar icons), PanelSprites, Group, Brackets, Tab,
   Hairline, Lamp, BarGauge, and the popup pieces QPopup QHero QRow QSlider QMeter QSwitch
   QButton QReading QScroll QField QTip QEmpty. Read their headers: each says what it is.

## How the grid works (so you do not fight it)
- A virtual pixel ("vpx") is a whole number of DEVICE pixels, resolved per window:
  HDMI-A-1 (3440x1440, scale 1): 1 vpx = 2 logical = 2 device px.
  eDP-2 (2560x1600, scale 1.666667): 1 vpx = 1.8 logical = 3 device px.
- Every length is `g.px(n)` where `g` is `Px.of(item)` (or `panel.g` inside a QPopup).
  Never write a bare number of logical pixels for a position, size, margin or gap.
- Text is `PixelText` (6 x 12 vpx cell, baseline row 10, capitals rows 2-9), never a Qt
  `Text`. Widths are in cells: `panel.cardWidth(44)`; fit a label with `columns:
  g.columns(width)`; wrap with `PixelParagraph`/`QRow.maxLines`. `BigText` is the same at
  2x or 3x for hero numbers.
- Icons are 7x7 `Sprite`s (rows of "#", "1".."9" lit by `level`, "!" accent, "."): see
  Sprites.qml / PanelSprites.qml for the alphabet and examples. Draw what you need. Keep them
  in an `Icons.qml` (a plain QtObject, NOT a singleton) inside your plugin dir and
  instantiate it in Panel.qml, so you never touch a shared singleton (which needs a shell
  restart and which other agents edit). Judge sprites before using them: copy
  /home/jwric/dev/omarchy-quadrille/plugins/tools/preview_sprites.py, point `SRC` at your
  file (any file of `readonly property var name: [ "..." ]` blocks), render a sheet to a
  PNG and Read it. Icons that are brand marks or glyphs (a Claude asterisk, a lock, a
  battery) are redrawn as 7x7 monochrome sprites; where the stock panel drew Nerd Font
  glyphs, map glyph -> sprite in one function (see quadrille.audio `spriteForGlyph`) and
  leave Model.js untouched.
- Colours are `Role.*` only (ink muted faint edge ground raised hover accent onAccent
  live caution alarm highlight line void_). No hex, no `Qt.darker`, no opacity for tone.
- NOTHING animates: no Behavior, NumberAnimation, ColorAnimation, SequentialAnimation,
  RotationAnimator, no opacity fades, no pill/rounded shapes (no `radius`), no
  `antialiasing: true`, no blur, no Text. A busy state is a muted word (SCANNING) and, if you
  want motion, a Timer that steps a lamp or a counter; nothing tweens.
- Selected / current / in use = inverse block (accent behind onAccent; QRow.current,
  QButton.active). Keyboard cursor = corner brackets (`Brackets`; colour Role.ink on an
  inverse block). Pointer hover = `raised` face. Section = `Group`. Reading = QReading.
  Slider/gauge = stepped cells (QSlider/QMeter). Switch = QSwitch. Tabs = QButton active.
- Group needs an explicit `height: implicitHeight` and `width: parent.width`.
- A list that can be longer than the screen scrolls inside `QScroll` (see audio);
  QPopup.fittedContentHeight(h, capPx) caps the card. A list that can be empty has an
  `QEmpty` row (so the card does not collapse and nothing jumps). Long names wrap to two
  lines (QRow.maxLines 2) and only then end in an ellipsis. Never overlap, clip or
  silently truncate: check the longest realistic string in every field.
- Panel width: choose it in cells (e.g. 40 to 56), wide enough that the widest realistic
  row fits at both scales; rows are 14 vpx (a line and a vpx of air each side), sections
  are 6 vpx apart, rows 2 vpx apart, hero is two lines (24 vpx).

## What stays exactly as in the stock panel (the host contract)
- Everything above the `KeyboardPanel { ... }` block in the stock file: properties,
  services, Processes, Timers, IPC handlers, keyboard cursor model, persisted settings,
  `ipcTarget` / `moduleName` (the stock id stays inside the code; the host routes it to the
  clone through `omarchy.clonedFrom`), the bar face item (`BarIconButton` etc: the bar hides
  it under a skin). Remove only what animates (fade Behaviors, SequentialAnimation) and
  keep the logic those drove.
- Third-party clones can be handed a null `shell` or missing services by the host: the
  stock code mostly guards `bar?.shell?...`; keep every guard and add one wherever you
  touch `bar.shell`.
- `KeyboardPanel` becomes `QPopup` (same properties; use `panel.cardWidth(cells)` for the
  width and `panel.fittedContentHeight(content.implicitHeight, cap)` for the height).
  `PanelKeyCatcher`, `Panel`, `BarIconButton` still come from `qs.Ui`.
- The clone dir: copy the whole stock plugin dir (`cp -aL`), add the symlink
  `ln -s ../quadrille.bar/Q Q`, give manifest.json the id `quadrille.<x>`, name
  "Quadrille <x>", `"omarchy": { "clonedFrom": "omarchy.<x>" }`, the same kinds and
  entryPoints; copy the stock barWidget block. See quadrille.audio/manifest.json.

## Live-session rules (three other agents share the user's real desktop)
- LOCK: wrap EVERY live sequence (shell rescan/restart, enabling a plugin, summoning a
  popup, workspace switching, screenshots) in
  `flock -w 900 /tmp/quadrille-live.lock bash -c '...'`. Hold it < 60 s. Never hold it while
  thinking. If the lock is busy the command just waits (use run_in_background for long
  waits). Another agent may restart the shell any time; if your surface vanishes, retry once.
- NEVER inject input into the live session (no wtype/ydotool/virtual pointer, no
  hyprctl dispatch that clicks or types). Workspace and focus dispatches are fine.
  Open popups only through the shell: `omarchy-shell shell summon omarchy.<x> '{}'` and
  `... hide ...` (these resolve to your enabled clone). So you can SEE: hover/keyboard
  cursor/pressed states cannot be screenshotted live; check them by reading the code and,
  if worth it, by temporarily forcing the property (`hasCursor: true`) for one screenshot.
- Never edit anything under /usr/share. Never touch ~/.config/hypr or any Hyprland/uwsm/env
  file. Do not change monitor scales, fonts, or the active theme. Do not change
  ~/.config/omarchy/shell.toml (the display panel's "text size" slider writes it).
- Keep the shell alive: `omarchy-shell shell ping` must say ok when you finish. If your
  plugin breaks the shell: `omarchy plugin disable quadrille.<x>` then
  `omarchy-restart-shell`.
- SCREENSHOTS in the repo must show NOTHING private: only over an EMPTY workspace with the
  wallpaper behind. Use plugins/tools/popup-shots.sh (it parks both outputs on empty
  workspaces, summons through the shell IPC, restores, and discards a grab if a workspace
  was not empty). READ every PNG you keep and confirm no window text is legible. Do not
  describe the contents of the user's windows anywhere. Other agents' test surfaces
  (red/orange brackets, toasts) may appear in your grabs: crop them out or retake.
  Keep scratch screenshots out of the repo (use your scratchpad); only commit PNGs under
  plugins/screenshots/ if the task message says to, tightly cropped.

## Deploy / test loop
```sh
cd /home/jwric/dev/omarchy-quadrille
ln -sfn $PWD/plugins/quadrille.X ~/.config/omarchy/plugins/quadrille.X      # once
flock -w 900 /tmp/quadrille-live.lock bash -c '
  omarchy-shell shell rescanPlugins
  until omarchy-shell shell ping >/dev/null 2>&1; do sleep 1; done; sleep 2
  omarchy plugin enable quadrille.X'          # once; swaps the stock id for the clone in the bar layout
# each edit: rescan (reloads non-keepLoaded plugins, kit components), wait for ping, then:
flock -w 900 /tmp/quadrille-live.lock bash -c '
  omarchy-shell shell rescanPlugins; until omarchy-shell shell ping >/dev/null 2>&1; do sleep 1; done; sleep 2
  WAIT=1.8 plugins/tools/popup-shots.sh /path/scratch/x HDMI-A-1:omarchy.X eDP-2:omarchy.X'
python3 plugins/tools/popup-crop.py /path/scratch/x-base-HDMI-A-1.png /path/scratch/x-X-HDMI-A-1.png /path/scratch/x-crop.png 34 10
python3 plugins/tools/popup-crop.py /path/scratch/x-base-eDP-2.png /path/scratch/x-X-eDP-2.png /path/scratch/y-crop.png 50 10   # eDP bar strip is 48 px
```
The shell can answer "not responding" for a few seconds after a rescan: wait for ping.
Edits to *singletons* (Px, Role, Sprites, PanelSprites) need `omarchy-restart-shell`; edits
to your Panel.qml / Icons.qml / non-singleton kit files only a rescan. QML errors appear in
`quickshell log -i "$(quickshell list --all | sed -n 's/^Instance \(.*\):$/\1/p' | head -1)" -t 60`
(a plugin that fails to load shows a warning there and the bar slot goes empty:
disable the clone, fix, retry).
- Crispness measurement: the card's box printed by popup-crop.py, handed to
  `python3 layershell/tools/crisp.py SHOT.png x0,y0,x1,y1 PS` with PS = 2 on HDMI-A-1 and 3 on
  eDP-2: expect <= ~12 colours and every transition residue 0. (See the power/audio runs.)
- To see states the live system is not in (empty list, long names, error text, a connecting
  state): temporarily inject fake data in your clone (override a list or a property), rescan,
  screenshot, then REVERT. Never commit fake data. Do not change the user's real
  state (do not mute, switch devices, toggle radios, connect, change scale or text size).

## Shared files (edit rules)
- Do not edit plugins/quadrille.bar/Bar.qml, install.sh, stock.sh, NOTES.md (the lead does:
  Bar.qml already maps all nine clone ids to the stock skins). Report what the notes
  should say in your final message.
- Kit files in Q/ are shared with agents working concurrently. Prefer components local to
  your plugin dir (e.g. quadrille.network/NetRow.qml). If a piece is clearly reusable, add it
  as a NEW file in Q/ and append ONE line to Q/qmldir with `printf '...\n' >> qmldir`
  (re-read the file right before; never rewrite it). Do not change an existing kit
  component's behaviour; a backwards-compatible property is acceptable.
- Git: commit only your own paths with `git add <paths>` (never `git add -A`, never
  `git commit -a`); if the index is locked wait and retry. Plain commit message, NEVER any
  Co-Authored-By trailer or Claude/AI attribution (the user's hard rule). Do not push.

## Done means
- No overlap, clipping, truncation or half-cell positions at any state you can produce, on both
  monitors (HDMI-A-1 scale 1, eDP-2 scale 1.666667): screenshots read and judged, crisp.py run.
- Every interaction of the stock panel still reachable (click, wheel, keyboard cursor, Esc,
  Tab to the next panel, right click) because the logic is untouched; new view code calls the same
  functions the stock view called.
- Final message to the lead (under 250 words): what was done, the evidence (what you measured,
  on which output), what is not exact and why, host limits found, and a few sentences for the
  NOTES.md section. Report where your plugin dir is and the commit id.
