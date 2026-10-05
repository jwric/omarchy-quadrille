# Shell plugins: what controls the look, and what we override

Measured on Omarchy 4.0.4, quickshell 0.3.1, Hyprland 0.56, with the
`quadrille-terminal` theme active. The shell is `/usr/share/omarchy/shell`
(package-owned, never edited). Everything here lives in
`~/.config/omarchy/plugins/` through symlinks to this directory.

## 1. What controls the look of the shell

### Token-driven: the theme layer already reaches it

`Commons/Color.qml` and `Commons/Style.qml` parse the theme's `shell.toml` (and
`colors.toml`) and every surface binds to them. `tools/gen_themes.py` sets these;
a theme switch pushes them live (`omarchy-shell shell applyTheme`).

| Token | Reaches | Set by quadrille |
|---|---|---|
| `[bar] background text active size-*` | the bar's fill, text, "active" colour, height | ground, ink, alarm, 16 vpx |
| `[popups] [tooltip] [notifications] [menu] [polkit] [lock] [image-picker]` colours, `border`, `border-width`, alphas | every card, its hairline and its selected row | ground, edge, 1 vpx, selected = accent behind on_accent |
| `[controls]` `*-fill-alpha`, `*-border*` | `Ui.Button`, fields, dropdowns, tabs | flat: hairline, no tint |
| `[font]` `base-size` and each token | `Style.font.*` (caption .. displayLarge, icon*) | every token a whole multiple of Departure Mono's 11 (22 / 44 / 66) |
| `[spacing]` `scale` and tokens | `Style.space()` margins, gaps, row heights | 1.0 |
| Hyprland `decoration:rounding` | `Style.cornerRadius`: every card radius, and `ToggleSwitch.rounded` | 0 (hyprland.lua) |
| Hyprland `general:gaps_out` | `Style.gapsOut`: distance of popups and toasts from an edge | 4 vpx |
| fontconfig `monospace` | `Style.font.family` and every `font.family: root.fontFamily` | Departure Mono, antialiasing off (60-quadrille-pixel.conf) |

Two things about the tables. `Color.shellValues` keeps **every** `section.key` of
`shell.toml`, including tables the shell does not know, so a theme can carry data
for plugins: ours states the fifteen roles in `[quadrille]` (`void ground raised
hover edge ink muted faint line accent on_accent highlight live caution alarm`,
plus `unit`). The shell's own tokens name only about seven of them (there is no
`live`, `caution`, `line`, `faint`, `hover` or `highlight`), so without the table
the kit derives them (see `Q/Role.qml`). And the user's
`~/.config/omarchy/shell.toml` is merged on top of the theme's: on this machine it
holds `[font] base-size = 12`. The theme's pinned per-token sizes outrank it for
fonts, but it sets `Style.fontScale`, and so how `Style.space()` scales: at the
theme's own `base-size = 22` every `space(n)` would be multiplied by 1.83 and land
on odd pixels, so on this machine that override quietly keeps the stock margins
whole. Do not remove it without pinning `[spacing]` tokens.

### Hard-coded in the host, theme cannot reach

Counted over the shell tree (`grep`), then confirmed on screen.

| What | Where | Effect on a pixel look |
|---|---|---|
| **Text rendering.** 288 `Text` items; only `Ui/WidgetButton.qml:84` and `Ui/OpticalGlyph.qml:37` set `NativeRendering`. | everywhere else | Qt Quick's default distance-field renderer with subpixel LCD fringes, even with fontconfig antialiasing off. Measured on the stock audio popup: 55-106 distinct colours per text run, red and blue fringes. The same string natively drawn: 2-8. |
| **Tweens.** `Behavior`/`*Animation` in 25 files: `WidgetButton` opacity 140 ms and colour 160 ms, `PopupCard` and `KeyboardPanel` fade 140 ms, `PanelSlider` 110-140 ms, `ToggleSwitch` 120 ms, `Button` 120 ms, `Bar.qml` colour 420 ms, `OSD` bar 140 ms, `Background` | all popups, the bar | A popup fades in; a slider knob glides. Nothing in `shell.toml` turns them off. (The bar-owned ones are off in our bar: `bar.foregroundAnimationEnabled` is a host-readable flag.) |
| **Rounding** that ignores `cornerRadius`: `Ui/PanelSlider.qml:50,87` (pill track, round knob), `power/Panel.qml:395`, `agents/Panel.qml:778,828` (pill meters), `Ui/SpeedTestOverlay.qml`, `panels/clock` calendar, the media widget | popups | Pills and circles survive `rounding = 0`. (`ToggleSwitch` and the cards do follow it.) |
| **Alpha** instead of role steps: ~100 `opacity:` and `Util.alpha()` sites; selected fills are accent at 0.18-0.35 alpha (`[controls] selected-fill-alpha`) | list rows in popups | A tinted brown row, not an inverse block. The alpha is tokenised, so `1.0` would make it a solid block; the stock theme leaves it. |
| **Gradients and effects**: menu scroll fades (`Menu.qml`), `MultiEffect` in tray/background/lock/image picker, `layer.smooth` | those surfaces | Soft edges, blurred wallpaper transitions. |
| **Fonts**: `NotificationCard.qml:170,185` is `"Liberation Sans"`, bold summary, regardless of font setting; `clock/Panel.qml` 48/52 px, `weather/Panel.qml` 64/56 px | notification toasts, two popups | Proportional sans in the middle of a pixel shell. |
| **Icons**: Nerd Font glyphs (`OpticalGlyph`, `BarIconButton`), app icons as images with `smooth: true` | bar, menu, toasts | Vector shapes at no particular grid. |
| **Odd-pixel metrics**: `Style.space()` rounds `n * scale`, bar/icon slots 21/27/16 px, tooltip `+6`, popup margin `gapsOut` | bar widgets | Half-vpx offsets; stock text is centred by `anchors.centerIn`, so on a 30 px area it lands on odd pixels. |

### Fractional scale: the grid is resolved per window

`eDP-2` runs at scale 1.666667, where the theme's 2 logical px per virtual pixel
would be 3.33 device px. The kit therefore does not have one unit: each window
resolves its own (`Q/Px.qml`),

    phys = max(1, round(target * dpr))    device px per vpx   (2 at scale 1, 3 at 1.666667)
    unit = phys / dpr                     logical px per vpx  (2, and 1.8)

and every length is a count of vpx times that, so every edge is a whole number
of device pixels; Qt Quick takes the fractional logical coordinates and the
scene graph hands them back whole. Items ask `Px.of(root)`, windows
`Px.forWindow(win)`; the answer is a plain object (`g.px(n)`, `g.cellW`,
`g.line`, `g.snap`, `g.centre`...), not a singleton value, so the bar, the menu,
the OSD and a toast each use the screen they are on. Scale 1 is unchanged: the
HDMI-A-1 renders are pixel-identical before and after (0 differing pixels in the
bar strips, the menu card, the OSD card and the toasts).

Four things the host makes awkward, and what the kit does about them:

* **`Screen.devicePixelRatio` is 2 on a 1.666667 output**: Qt reports the integer
  output scale. The *window's* `devicePixelRatio` is 2 until the surface is
  mapped, then 1.6666666666666667; the grid follows it. Reading the screen's
  would put every length on a 4-device-pixel unit.
* **A Qt `Text` cannot be set at the size that makes one font pixel a whole
  number of device pixels.** QML's `font.pixelSize` is an integer, and a
  `pointSize` is rounded to one before the font engine sees it (14.85 pt gave the
  same render as 20 px). Departure Mono at 20 logical px is rasterised at 33.33
  device px; glyph edges fall between pixels and come out anti-aliased
  (58-75 colours in a line of text, stems of unequal width), whatever fontconfig
  says. So **the kit does not use `Text`**: `tools/gen_glyphs.py` bakes Departure
  Mono Tight into 6 x 12 bitmaps (`Q/Glyphs.js`, 526 glyphs, checked pixel for
  pixel against what Qt drew at scale 1) and `PixelText` draws them as
  rectangles of one vpx. That is exact on any screen by construction. What still
  goes through Qt text is what we do not draw: stock widgets' labels and the
  Nerd Font glyph in a toast.
* **A layer surface is a whole number of logical pixels**, and 16 vpx are 28.8 at
  scale 1.666667. The bar's surface is 29 logical px (its buffer 48 device px, the
  frame exactly 16 vpx) and the frame sits at the surface's origin;
  `Px` gives `g.bar` (the frame) and `g.barWindow` (the surface).
* **Right- and centre-anchored things land where the window's width puts them.**
  Slots are a whole number of vpx wide (stock widgets' widths are rounded up to a
  whole device pixel), and the centre slot snaps to the grid, so what is drawn by
  us stays on whole device pixels; but a surface 2560 device px wide is not a
  multiple of 3, so what is anchored to its right edge (the status icons, toasts)
  is on the 3-px grid relative to *itself*, not to the surface origin: measure it
  from its own corner, as `layershell/tools/crisp.py` does.

## 2. What a third-party plugin can and cannot override

Tested, not assumed.

| Can | How | Verified |
|---|---|---|
| Replace the whole bar | `kind: bar`, `bar.id` | yes: `quadrille.bar` |
| Steer every stock widget's look in it | the bar hands them `fontFamily`, `barForeground`, `urgent`, `foregroundAnimationEnabled`, `barSize`, `position` | yes: stock labels come out in Tight and stop easing |
| Draw over or instead of stock widgets | load the registry's `Component` hidden and paint a skin over it; or substitute its own `Component` for an id | yes: skins, `QMenu`, `QWorkspaces` |
| Keep the stock popups working under a replaced button | the hidden stock widget still owns the popup and `open()/close()/opened`; `shell.summon` reaches it through the bar's `summonBarWidget` | yes (audio) |
| Replace a first-party service, panel or menu | `omarchy plugin clone`, or a manifest with `omarchy.clonedFrom`; IPC and `summon` routing follow the clone | yes: OSD, notifications, menu |
| Add a bar widget | `kind: bar-widget`, `barWidget` entry | yes: `quadrille.sysmon` |
| Carry theme data of its own | extra table in `shell.toml`, read from `Color.shellValues` | yes: `[quadrille]` |
| Read host singletons | `import qs.Commons`, `qs.Ui` from a plugin directory works; singletons are the host's (live theme) | yes |

| Cannot (without a worse trade) | Why |
|---|---|
| Change `Ui/*` components (`PanelSlider`, `ToggleSwitch`, `PopupCard`, `KeyboardPanel`, `WidgetButton`, `Button`) | host-owned, imported as `qs.Ui`; a cloned widget could import private copies, at 1,000-1,900 lines per panel (audio 1248, network 1970, bluetooth 1045) |
| Make host `Text` native | no per-window or per-item hook reaches other plugins' items; `QQuickWindow::textRenderType` is not exposed to QML (the window object Quickshell hands out has no such property: assigning one changes nothing). Only the environment does it: `QML_DISABLE_DISTANCEFIELD=1` in the shell's environment took a `QtRendering` run from 657 colours to 8 in an isolated quickshell, and `QSG_DISTANCEFIELD_ANTIALIASING=gray` to 102 (no fringes, still smooth). It is a session setting (Hyprland `hl.env` or `~/.config/uwsm/env.d/`), outside what we may touch; **it is the single highest-value one-liner** |
| Turn off popup fades and pill sliders | as above |
| Restyle lock screen and polkit | authentication plugins; a clone inherits the source's authentication capability, and replacing the thing that guards the session is not something to do on a live desktop. Not attempted |
| Change `Style` / `Color` | shared singletons |
| Hot-reload a `keepLoaded` plugin | the notification, OSD and menu clones are `keepLoaded` like their sources; edits need `omarchy-restart-shell`. Singletons in the kit (`Px`, `Role`, `Sprites`) are cached by the engine: edits to them need a restart as well |

Quirks met on the way, all outside our code:

* **Hot-switching from the stock bar to a replacement leaves it empty.** The
  replacement receives a copy of the widget catalogue made before it filled and
  the host does not refresh it until the next plugin change. Reproduced with an
  unmodified byte-for-byte clone of `omarchy.bar`, so it is the host, not the
  restyle. `Bar.qml` checks 2.5 s after load and, if the catalogue is empty, runs
  `omarchy-shell shell rescanPlugins` once. A cold start is unaffected.
* **First `IpcHandler` for a target wins**, and a handler registered while its
  predecessor is still being torn down is ignored for good. After a hot switch the
  `omarchy.bar` target (`syncHidden`, used by `omarchy-toggle-bar`) can be dead;
  the file watch still works.
* **A notification daemon must not be hot-swapped**: two `NotificationServer`s
  contend for the D-Bus name and the loser never retries. Enable or disable the
  notification clone and restart the shell.
* The plugin watcher is `inotifywait -r` on `~/.config/omarchy/plugins`, which
  does not follow symlinks: editing files in this repo does not reload anything.
  `omarchy-shell shell rescanPlugins` does, for non-`keepLoaded` plugins.
* Disabling a clone's source (`disabledPlugins`) also unregisters the source's
  bar widget. `quadrille.bar` therefore replaces `omarchy.menu` and
  `omarchy.workspaces` without needing the stock widget loaded.

## 3. What is here

```
plugins/
  quadrille.bar/            kind bar      the bar
    Q/                      the kit (a QML module: Px Role Sprites PixelText ...)
    skins/                  QMenu, QWorkspaces (replace) and Audio/Network/... Skin (overlay)
    Bar.qml  BarModel.js    stock Bar.qml, cloned and restyled; BarModel.js verbatim
  quadrille.osd/            kind panel    OSD clone
  quadrille.notifications/  kind service  notification daemon clone, card redrawn
  quadrille.menu/           kind menu     Omarchy menu clone
  quadrille.sysmon/         kind bar-widget  CPU and memory gauges
  quadrille.lab/            kind panel    specimen sheet of the kit (not enabled)
  install.sh  stock.sh      link and enable / put the stock shell back
```

`Q`, `Px`, `Role` and the rest are reached from the other plugins through a
`Q -> ../quadrille.bar/Q` symlink, so `import "Q"` works in each.

### The kit (`quadrille.bar/Q`)

| Piece | What it is |
|---|---|
| `Px` | the grid of a window: `Px.of(item)` / `Px.forWindow(win)` give an object with `unit` (logical px per vpx: the theme's `[quadrille] unit`, else `font body / 11`, resolved so a vpx is a whole number of that window's device pixels), the spacing scale `hair tight gap wide far` (1 2 4 8 16 vpx), the body face's metrics (`cellW` 6, `line` 12, `cap` 8, `capTop` 2, `baseline` 10), `bar` / `barWindow`, and `px snap floor ceil whole centre onCaps cells columns vpx`. Also `Px.face`, the font name stock widgets take from the bar |
| `Role` | the fifteen roles, from `[quadrille]` when the theme has it, derived from the shell's tokens plus `colors.toml` when it does not. Named `Role`, not `Palette`: QtQuick has a `Palette` type and it shadows the singleton silently |
| `PixelText` | Departure Mono Tight at its native size drawn from baked bitmaps (`Glyphs.js`), a 12-vpx line with the baseline at row 10; exactly `length x 6` vpx wide on any screen. `columns` cuts to a width with an ellipsis, `room` drops the label instead of cutting it. Characters the font lacks draw as a box |
| `PixelParagraph` | wrapped on the cell grid (spaces, newlines, anywhere in a word too long for a line), whole cells wide, whole lines tall, ellipsis on the last line |
| `Sprites` + `Sprite` | 7 x 7 bitmaps as strings (`#` lit, `1`-`9` lit when `level` reaches the digit, `!` accent, `.` off): wifi ethernet offline volume muted microphone bluetooth battery plug cpu clock menu brightness bell warning chevrons cross tick keyboard power media lock. Drawn as run-length rectangles, `unit` per pixel, no smoothing |
| `Hairline` `Lamp` `BarGauge` | the 1-vpx rule; a 6-vpx lamp (lit tone or raised glass in a hairline); stepped cells, lit up to the value and never past it, alarm past `redline` |
| `Tab` `Inverse` | a hairline legend that is the accent behind `on_accent` when active (padding is `air` less the face's side bearings, so the ink is centred); the emphasis block |
| `Group` | `┌── NAME ──┐`: a rule broken by a muted name, ends dropping 3 vpx, no sides, no bottom; the rule runs unbroken if the name will not fit |
| `Brackets` | four L marks; the focus and selection frame |
| `Pressable` `BarButton` | a hit area reporting `hovered`/`pressed`; the bar's click contract (registers as a click target, shows tooltips, `pressed(button)`, `wheelMoved`) |

All colour is a `Role`; there is no hex in the kit. All geometry is a multiple of
`Px.unit`.

### The bar: why a clone and not a from-scratch bar

`Bar.qml` is stock `omarchy.bar` with the host contract left whole and the look
replaced. A third-party bar is an `Item` the host loads and hands `shell`,
`barConfig`, `barWidgetRegistry`; the contract the *widgets* expect of their `bar`
(`barForeground`, `urgent`, `fontFamily`, `showTooltip`, `requestPopout`,
`registerClickTarget`, `moduleWidgets`, `switchPanelFrom`, `summonBarWidget`,
`hideBarWidget`, `isBarWidgetOpen`, `run`, `shell.updateEntryInline`) is about a
thousand lines of coordination, and the host calls `summonBarWidget` on whatever
`shell.bar` is. Reimplementing that to change the paint would have been the work
of a bar, not of a look, and would have broken keyboard panel hopping, popout
switching, drag-to-reorder and per-monitor widget routing the first time one
detail differed. Cloning keeps them and changes only what is seen. The change list
is at the top of `Bar.qml`. First-party widgets still receive the bar itself;
third-party ones still receive the scoped `PluginBarApi` facade.

The look:

* A flat `ground` zone, one `edge` hairline on the inner side, nothing tweens.
* Everything is centred in the 16-vpx frame: capitals on rows 4-11, baseline at
  12, icons rows 4-10, tabs rows 2-13, the hairline row 15. Text from stock widgets
  lands on the same rows because `fontFamily` is Tight and the bar height is whole.
* **Workspaces** (`QWorkspaces`, replaces the stock widget): numbered tabs, the
  workspace in use on *this screen* lit, one with windows in ink, an empty one
  muted. The stock widget lights the globally focused workspace on every bar.
* **Menu** (`QMenu`): the 7 x 7 hamburger; left opens the menu, right a terminal.
* **Skins** over stock widgets that keep running hidden (they own the popups):
  clock (upper-case label, from the stock `displayText`, so format cycling and the
  calendar are unchanged), keyboard layout (the code, upper case), network (wifi fan lit to the signal, wired, cross),
  bluetooth (rune: ink connected-in-live, muted idle, faint off), audio (speaker
  waves and a 5-cell gauge, cross when muted), display (sun), power (4-cell
  battery and the percentage; live while charging, alarm under 15%). Each reads the
  stock widget's own properties defensively (`outputVolume`, `kind`,
  `signalStrength`, `batteryFraction`, ...), so a changed widget shows an empty
  icon, not an error.
* Hover steps the face to `raised`; an open popup is corner brackets; the tooltip is
  a raised hairline box in the body face; the drag ghost and the bar-move preview
  are square, opaque and instant.
* Not skinned, still stock: the tray (app icons are pictures), indicators, weather,
  system update, agents. Their glyphs are Nerd Font vectors.
* A vertical bar keeps the chrome and the stock widgets, except the menu button.
* Transparency is accepted and ignored; the `omarchy-bar-text-color` probe is gone.

### OSD, notifications, menu: clones

All three keep their source's contract unchanged (IPC target and payload, open /
close, the card's properties and signals) and change only what is drawn.

* `quadrille.osd`: a hairline card, a 7 x 7 icon at 2x (every pixel 2 vpx), a
  20-cell gauge, a 4-cell readout that does not jitter.
* `quadrille.notifications`: `Service.qml` verbatim; `components/NotificationCard.qml`
  replaced. Muted capitals app, ink summary, muted body wrapped on whole cells,
  alarm hairline and lamp when critical, raised face under the cursor, app icons
  nearest-neighbour. No more Liberation Sans.
* `quadrille.menu`: body face on whole cells so labels are no longer cut to
  `Trigg...`, 14-vpx rows, pixel chevrons, muted instead of half-opacity, hairline
  scroll marks instead of gradient fades, no Nerd Font icon glyphs (`glyphIcons`),
  card snapped to the grid. The stock scrim (a 50% alpha wash) is kept: a dither
  needs a texture per theme and was not worth a generated PNG.

Skipped, with the reason in section 2: the popup panels (audio, network, ...) share
the host `Ui` components; lock and polkit.

### Verification

With `grim` at native resolution, every surface was measured with
`layershell/tools/crisp.py`: how many colours it has, and whether every change of
colour along a row or a column falls on a multiple of the pixel scale from the
surface's own origin.

| Surface | HDMI-A-1 (scale 1, 2 px) | eDP-2 (scale 1.666667, 3 px) |
|---|---|---|
| bar: menu button and workspace tabs | 6 colours, every step on the grid | 6 colours, every step on the grid |
| bar: status icons, gauges, percentage | 6 colours, on the grid | 7 colours, on the grid |
| bar: the clock text | on the grid | 3 colours, on the grid |
| menu card (header, rows, chevrons) | 0 px differ from the render before the change | 6 and 4 colours, on the grid |
| OSD card | 0 px differ | 5 colours, on the grid |
| toasts (text, lamp, hairline) | 0 px differ | 4 colours, on the grid |

The stock glyph icons and the stock popups fail the same test, which is the point
of it. All five palettes were applied in memory (`applyTheme`, nothing written) and
the bar re-themes at once.

Still not exact on eDP-2, because the host draws them: the stock widgets that are
not skinned (tray chevron, agents, indicators, weather: Nerd Font glyphs at
36.7 device px), the stock popups' text and sliders, app icons (pictures) in
toasts and menu rows, the Nerd Font glyph of a glyph toast, and the notification
daemon's and bar's size hints, which still say 32 logical px (`bar.barSize`), so a
toast sits a few pixels lower than it needs to on a 29 px bar.

## 4. Use

Needs Departure Mono and Departure Mono Tight installed (`tools/install.sh`) and,
for the whole thing to read right, a quadrille theme (`omarchy theme set
"Quadrille Terminal"`). On another theme the bar still works and takes its colours
from that theme, with a 1-pixel unit.

```sh
plugins/install.sh            # link, enable all, restart the shell
plugins/install.sh bar        # only the bar
plugins/stock.sh              # the stock bar, menu, OSD, notifications again
plugins/stock.sh --unlink     # ... and remove the plugin links
```

By hand:

```sh
ln -s $PWD/plugins/quadrille.bar ~/.config/omarchy/plugins/quadrille.bar
omarchy-shell shell rescanPlugins
omarchy plugin enable quadrille.bar          # the bar
omarchy plugin disable quadrille.bar         # no off state: use omarchy plugin enable omarchy.bar
omarchy plugin enable quadrille.osd quadrille.menu quadrille.notifications   # then omarchy-restart-shell
omarchy plugin disable quadrille.osd         # restores the stock one
omarchy plugin enable quadrille.sysmon --section right --before omarchy.bluetooth
omarchy-shell shell summon quadrille.lab '{"screen":"HDMI-A-1"}'   # the specimen sheet
```

If a plugin breaks the shell: `omarchy plugin disable <id>` and
`omarchy-restart-shell`. A bar that fails to load falls back to the stock bar on
its own.

Working on them:

```sh
omarchy-shell shell rescanPlugins     # reload bar, skins, widgets, lab
omarchy-restart-shell                 # the kit's singletons, and the osd/menu/notification clones
quickshell log -i "$(quickshell list --all | sed -n 's/^Instance \(.*\):$/\1/p' | head -1)" -t 40
```

What the plugins execute, all through the same `bar.run` the stock widgets use:
the menu button (`omarchy-shell shell toggle omarchy.menu`, `xdg-terminal-exec`),
the CPU gauges' click (`omarchy-launch-or-focus-tui btop`), and the bar's one-off
`omarchy-shell shell rescanPlugins` described above. Nothing touches the network
or writes outside its own directory.

## 5. Not done

* A dither scrim for the menu; pixel icons for the tray, indicators, weather.
* Restyled popup panels (needs private copies of `Ui/*`), lock screen, polkit.
* `QML_DISABLE_DISTANCEFIELD=1` for the session (section 2). It is one line and it
  makes the stock popups' text crisp; it is not ours to set.
* The themes' `shell.toml` files now carry `[quadrille]`; the *running* session
  still has the older copy in `~/.local/state/omarchy/current/theme` until the
  theme is applied again (`omarchy theme set "Quadrille Terminal"`). Until then the
  kit derives the roles, and gets `live`/`caution`/`line` right from `colors.toml`
  and `highlight` approximately.

## The Apps menu opened empty (found 2026-10-05)

`quadrille.menu` showed "Nothing here yet" under Apps. The shell's own menu was
fine, so the application library works; the clone never got it. A third-party
menu is meant to read `shell.appLibrary`, a facade made in `shell.qml`
(`pluginAppLibraryFor`). On Omarchy 4.0.4 the `shell` object the host assigns to
the clone has `appLibrary == null`, and the host then revokes that object
(`prunePluginApis` / `revokePluginShellApi`), leaving `shell == null`. Either
way `mergeAppRows()` had no library and built no rows. This is a host problem
and not fixable from a plugin, so the clone does not depend on it:

- `LocalApps.qml` answers the library's calls from Quickshell's `DesktopEntries`
  (with a copy of the shell's `AppSearch.js`). `Menu.qml` prefers the shell's
  library when the host provides one and falls back to this.
- What the fallback lacks: the shell's hidden-entries filter, launch feedback,
  and the icon-file fallback index. Launching is the same `uwsm-app -- gtk-launch`.

If a later Omarchy fixes the facade the fallback is simply not used. Any other
third-party menu clone will hit the same thing.
