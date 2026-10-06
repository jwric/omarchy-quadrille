# omarchy-quadrille

[quadrille](https://github.com/jwric/quadrille)'s pixel-perfect look on an
[Omarchy](https://github.com/basecamp/omarchy) desktop: its five palettes as
Omarchy themes, Departure Mono drawn without antialiasing in every toolkit, a
whole-pixel QML bar, menu, OSD and notifications for the Omarchy shell, and
panels drawn by quadrille itself as Wayland layer-shell surfaces.

The desktop is a metric drafting graticule. Each output gets a centre-zero
wallpaper calibrated to its physical panel, millimetre ticks, centimetre grid
and edge rulers, a display spec plate, and a drawing of the output arrangement
at one physical scale. The drawing describes compositor crossings and offsets.
An always-on, click-through cursor reticle in the Rust host shows millimetres
from the output's top-left and centre, above application windows.

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

The physical model corrects centimetre-rounded EDID sizes by snapping the
diagonal to a nearby nominal panel size and deriving width and height from
the native square-pixel aspect ratio. The laptop resolves to about
344.6 × 215.4 mm. The Dell's rounded EDID diagonal is nearest to 34.1 in the
nominal-size list, giving 799.0 × 334.4 mm; use a measured override for its
actual 797.8 × 333.9 mm. Each millimetre mark is
rounded to the nearest virtual pixel; individual marks are accurate within
half a virtual pixel. The spec plate states the physical size of one vpx.
At the laptop's 3-physical-pixel unit this is 0.40 mm; at the ultrawide's
2-pixel unit it is 0.46 mm.
Two independently snapped marks can differ in their gap by up to one vpx;
the half-vpx bound applies to each mark's physical position.
With missing EDID dimensions the model assumes 96 PPI and prefixes dimensions
with `~`; an override removes that estimate.

An optional `~/.config/quadrille/displays.toml` supplies measured dimensions.
Nothing creates it by default. Output names take priority over make/model keys:

```toml
["eDP-2"]
width_mm = 344.6
height_mm = 215.4

["Dell Inc. DELL U3417W"]
width_mm = 797.8
height_mm = 333.9
# Alternatively, diagonal_inches = 34.0 (without width_mm/height_mm).
```

The `diagonal_inches` configuration key accepts manufacturers' nominal panel
sizes; all on-screen labels remain metric. Width and height describe the
native panel before rotation. Provide both dimensions or a diagonal.

The overlay starts by default in every host mode, including `--no-bar`:

```sh
quadrille-bar --no-bar                     # panels and cursor overlay
quadrille-bar --no-bar --no-overlay        # start with overlay disabled
quadrille-bar ctl overlay on
quadrille-bar ctl overlay off
quadrille-bar ctl overlay status
```

The runtime switch lasts until the host exits. Wallpaper calibration is part
of the theme. The cursor uses Hyprland IPC, polls faster while moving and
slower while still, and hides while fullscreen or locked.
The host applies a namespace-specific runtime layer rule to stop Hyprland
animating the reticle's position; it disables that rule when the overlay stops.
After editing calibration, `quadrille-bar ctl overlay off` followed by
`quadrille-bar ctl overlay on` reloads it; the wallpaper watches the file.

Hyprland 0.56.2 [floors cursor IPC coordinates to whole logical pixels](https://github.com/hyprwm/Hyprland/blob/efb50993780079460b0cbed1363e2166a2de1d9f/src/debug/HyprCtl.cpp#L1291-L1304).
The reticle is exact on the physical grid for the reported coordinate. At
fractional scales, IPC loses the pointer's sub-logical position and can select
the neighbouring vpx. Exact tracking of every actual pointer pixel requires
a compositor API exposing that precision.

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
- The strict nested overlay hotplug check currently finds one-level colour
  fringes after a headless output is recreated. Native buffers and geometry
  are exact; compositor presentation remains unresolved. Live previews were
  deferred because this check did not pass; see `layershell/NOTES.md`.

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
