# omarchy-quadrille

[quadrille](https://github.com/jwric/quadrille)'s pixel-perfect look on an
[Omarchy](https://github.com/basecamp/omarchy) desktop: its five palettes as
Omarchy themes, Departure Mono drawn without antialiasing in every toolkit, a
whole-pixel QML bar, menu, OSD and notifications for the Omarchy shell, and
panels drawn by quadrille itself as Wayland layer-shell surfaces.

Each monitor's wallpaper, APERTURE, is a calibrated technical portrait of that
display, drawn on its own pixel grid. Its click-through cursor draws live CAD
measurements that update as the pointer moves whenever the overlay is enabled.

![APERTURE on the laptop (eDP-2, 3 physical pixels per virtual pixel)](plugins/screenshots/testcard-terminal-laptop.png)

![The five themes, as the bar draws them](plugins/screenshots/themes.png)

![Menu, OSD and notifications at 3 physical pixels per virtual pixel](plugins/screenshots/laptop.png)

It is three layers, each useful without the next:

| | what | where |
|---|---|---|
| 1. Themes and font | five Omarchy themes generated from quadrille's palettes (terminal, paper, phosphor, amber, lcd), a dithered graticule wallpaper, square whole-pixel Hyprland borders and gaps, no animation; a fontconfig rule that turns antialiasing off for Departure Mono | `themes/`, `fontconfig/`, `tools/` |
| 2. Shell plugins | a QML kit (lamps, gauges, tabs, groups, 24 pixel icons, text from baked bitmaps), a replacement `quadrille.bar`, and restyled menu, OSD and notifications | `plugins/` |
| 3. Panel host | `quadrille-bar`, an iced app on a layer-shell windowing shell of its own (sctk + tiny-skia): system, audio, network, Bluetooth and power panels, plus a cursor graticule, following the Omarchy theme live | `layershell/` |

## Use

Needs an Omarchy 4 desktop (Hyprland, Quickshell) and a checkout of
[quadrille](https://github.com/jwric/quadrille) beside this one (or
`QUADRILLE=/path`): the fonts and the palettes come from it.

```sh
tools/install.sh                           # link the themes, install Departure Mono and its fontconfig rule
omarchy theme set "Quadrille Terminal"     # or Paper, Phosphor, Amber, Lcd
omarchy font set "Departure Mono"

plugins/install.sh                         # the bar, menu, OSD, notifications, CPU/MEM gauges
plugins/stock.sh                           # the stock Omarchy shell again

layershell/tools/install.sh                # builds quadrille-bar into ~/.local/bin and prints
                                           # the autostart and binding lines to add
```

`tools/install.sh` also sets up **foot**: a user-level `foot.desktop` runs
`tools/quadrille-foot`, which picks Departure Mono's pixel size from the focused
monitor's scale so every glyph edge is on the pixel grid (11 px at scale 1, 13.2 px at
1.666667, both exact multiples of the font's native 11 px; a plain point size such as 9
is off the grid on both). Your `foot.ini` is not touched. `QUADRILLE_FOOT_MULTIPLE=2` or
`3` makes the terminal's pixels the same size as the shell's, with chunkier text. To undo,
delete `~/.local/bin/quadrille-foot` and `~/.local/share/applications/foot.desktop`.
`tools/term-test.sh` renders candidates in a nested compositor at both scales.

**Measuring overlay key.** The cursor reticle and live dimension lines are off at login
(`quadrille-bar --no-bar --no-overlay`); `SUPER + CTRL + G` runs `tools/quadrille-overlay-toggle`
(linked to `~/.local/bin`) to turn them on and off. `quadrille-bar ctl overlay on|off|status` does the same by hand.

Nothing here edits your Hyprland config; the scripts print what to add. Crisper
shell text wants one line in it: `hl.env("QML_DISABLE_DISTANCEFIELD", "1")`
(Qt's distance-field text renderer smears a pixel font; this turns it off for
the shell).

Back to how it was: `omarchy theme set <your theme>`, `omarchy font set <your
font>`, `plugins/stock.sh`, and delete
`~/.config/fontconfig/conf.d/60-quadrille-pixel.conf`.

> **Plugins run unsandboxed inside `omarchy-shell`.** Read `plugins/` before
> enabling it. The clones execute what the Omarchy plugins they come from
> execute, and nothing else (see `plugins/NOTES.md`).

## Pixel scale

A virtual pixel is a whole number of physical pixels: `max(1, round(2 × monitor
scale))`. A monitor at scale 1 gets 2, at 1.666667 gets 3 (the logical size is
then 1.8 per virtual pixel; both the QML kit and the panel host work in device
pixels), at 2 gets 4. The themes' shell and Hyprland sizes assume 2 logical
pixels (`tools/gen_themes.py --unit`).

## Display calibration and cursor overlay

Centimetre-rounded EDID is snapped to a nominal panel family, then width and
height follow the native pixel aspect. This machine resolves to 344.6 × 215.4 mm
and 796.6 × 333.5 mm; these are inferred sizes. A mark rounds to the nearest vpx:
0.404 mm on the laptop, 0.463 mm on the Dell. The sheet's mark-error bound is
half a vpx, conservatively rounded upward; it is not a calibration guarantee.
Missing EDID uses an explicitly estimated density and `~` labels.

Each output draws its own static sheet, APERTURE: the monitor's number beside a
miniature of the configured desktop layout; a calibrated Siemens star (100 mm on the
laptop, 180 on the Dell) whose diameter dimension is a true-size millimetre scale;
DETAIL A, the disc inside the star's grid-limit ring redrawn 6:1 on the panel's own
pixels; B, one virtual pixel taken apart into its 3 x 3 (laptop) or 2 x 2 (Dell) panel
pixels beside an edge of the star at both resolutions; fine stripes beside what the
grid really draws from them (resolved, false detail, detail lost); a dimensioned
elevation of the active area at a true 1:N; the theme's roles as surfaces, inks and
signals. Inferred sizes are marked ESTIMATED; unknown ones carry `~`. Binary patterns
are precomputed into an exact texture on display/theme events; the wallpaper does no
work between changes. The live cursor overlay is separate. Design notes:
`plugins/NOTES.md`, "Design".

An optional `~/.config/quadrille/displays.toml` supplies measured dimensions.
Nothing creates it by default. Replace these example sizes with measurements;
output names take priority over make/model keys:

```toml
["eDP-2"]
width_mm = 344.6
height_mm = 215.4

["Dell Inc. DELL U3417W"]
width_mm = 797.2
height_mm = 333.7
# Alternatively, diagonal_inches = 34.0 (without width_mm/height_mm).
```

The `diagonal_inches` configuration key accepts manufacturers' nominal panel
sizes; all on-screen labels remain metric. Width and height describe the
native panel before rotation. Provide both dimensions or a diagonal.

The overlay starts by default in every host mode, including `--no-bar`:

```sh
quadrille-bar --no-bar                     # panels and cursor overlay
quadrille-bar --no-bar --no-overlay        # start with overlay disabled
quadrille-bar --no-bar --overlay-rest-ms 300 # small reticle until 300 ms of rest
quadrille-bar ctl overlay on
quadrille-bar ctl overlay off
quadrille-bar ctl overlay status
```

The switch lasts until the host exits. `--overlay-rest-ms N` defaults to 0 (live);
positive values keep a small reticle while moving and show dimensions after N ms
of rest. Cursor IPC adapts from 60 Hz to 5 Hz; fullscreen and lock suppress the
overlay. Dimensions show the nearest horizontal
and vertical screen distances (`S`) and, over a visible window, its nearest
horizontal and vertical edges (`W`). Plates avoid one another; spanning-window
edges on another output are omitted. Values are whole millimetres because IPC
reports whole logical pixels. Motion repaints and damages only changed line and
plate rectangles in reusable buffers; window geometry is queried at about 4 Hz
and on window events. Off and suppression free the large surface. Only read-only
compositor IPC is sent by the host.

Nested measurements (one core / RSS): moving with dimensions costs 1.55% / 25.02
MiB on QA (2560 × 1600 at 1.666667), 1.52% / 28.26 MiB on QB (3440 × 1440 at 1).
Still costs 0.13–0.18%; off costs 0.00% / 9.30 MiB. Details are in `layershell/NOTES.md`.

If a compositor configuration still fades layers, this **optional** line belongs
in the user's own Lua config (the host never installs it):
`hl.layer_rule({match={namespace="^quadrille-(reticle|dimensions)$"},no_anim=true})`

Off/on reloads display overrides. The wallpaper watches existing overrides;
`omarchy-shell background refresh` also loads a newly created file.

## What is not exact

- The stock widgets the bar keeps (tray, agents, indicators, weather), popup
  text and sliders, and application icons are the shell's own and are not on the
  grid at fractional scales. A plugin cannot restyle the host's UI components;
  `plugins/NOTES.md` lists what a plugin can and cannot override.
- The shell passes a third-party menu clone no application library on Omarchy
  4.0.4, so `quadrille.menu` reads desktop entries itself (no hidden-entries
  filter, no launch feedback).
- Physical calibration infers nominal diagonals from rounded EDID data. Use
  measured overrides when tighter physical accuracy matters.

## Layout

```
themes/        generated by tools/gen_themes.py from quadrille's theme.rs
fontconfig/    antialiasing and subpixel colour off for Departure Mono
tools/         gen_themes.py, install.sh
plugins/       the QML kit and plugins, NOTES.md (findings), screenshots/
layershell/    the panel host: windowing shell, bar crate, nested-compositor tests; NOTES.md
```

`layershell/` depends on quadrille and on the
[pixel-scale iced fork](https://github.com/jwric/iced/tree/0.15-pixel-scale) as
git dependencies (uncomment the `[patch]` in `layershell/Cargo.toml` to build
against local checkouts). Tests inject input only inside a nested Hyprland.
Its temporary visible window and empty-workspace previews take the desktop lock.

## Licence

Original code: MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`). The plugin
clones derive from Omarchy (MIT) and keep its notice; Departure Mono is by
Helena Zhang under the SIL Open Font License. See `NOTICE`.
