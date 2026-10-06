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

## Popup panels (agent P)

### Why spacing, not only type: the theme tokens

The stock popups are laid out in pixels for 12 px type (`Style.space(480)`, rows
`space(28)`, hero `space(14)` gaps). Ours is 22. Pinning the *font* tokens alone
cannot fix that: the weather card's temperature overlapped its FEELS label with
`heading` and `display-large` pinned any way at all, because the labels in that row
are `bodySmall` and `title`, which have nowhere smaller than 22 to go on the pixel
grid (11 would be half a virtual pixel per font pixel). What fixed every stock popup
(audio, bluetooth, network, monitor, power, weather, clock, agents, on both
outputs: no overlap, clipping or truncation) is making the spacing grow with the
type: `[spacing] scale = 2.0`, `scale-with-font = false` in the theme. Every
`Style.space(n)` is then `2n` logical px, which is `n` virtual pixels at scale 1, and
it no longer depends on `[font] base-size` (the machine's own `~/.config/omarchy/shell.toml`
says 12; with `scale-with-font` on, that was what kept the margins whole, and it is no
longer load-bearing).

The type pins: body and everything up to a heading 22, display 44, display-large 44,
icon-large 33. No pin exceeds its stock ratio to the body (stock 10 11 12 13 14 16 24
28 over 12) except caption and body-small, which cannot go below 22. Display stays
at 2x because stock is 24 / 12. 33 is the only size besides 66 that is a whole
number of device pixels per font pixel on both outputs (3 x 11 at scale 1, 5 x 11 at
1.666667).

Measured on the eDP-2 laptop output and HDMI-A-1, `[spacing] scale` is the one
setting that moves the unskinned stock widgets' margins too (`WidgetButton`'s
`horizontalMargin 7.5` is scaled); the bar's widths did not change.

### The popup kit (`quadrille.bar/Q`)

The stock popups are made of a handful of host components (`KeyboardPanel`,
`PanelHero`, `PanelSectionHeader`, `PanelSlider`, `ToggleSwitch`, `CursorSurface`,
`Button`, `TextField`, `PanelToolTip`), imported as `qs.Ui`, which a plugin cannot
restyle. The clones keep each panel's logic verbatim (properties, services,
processes, keyboard model, `Model.js` untouched) and replace the view with these:

| Piece | What it is |
|---|---|
| `QPopup` | `KeyboardPanel`'s contract (`anchorItem owner bar open centerOnBar focusTarget contentWidth contentHeight fittedContentWidth/Height`, the dismiss twins on other outputs, the bar click forwarding, the Exclusive-then-OnDemand focus prime) with a flat `ground` card in a 1-vpx `edge` hairline, square, snapped to this screen's grid. No fade. `cardWidth(cells)` is the card for that many text cells; `fittedContentHeight(h, cap)` is whole vpx and capped to the screen; `g`, `inset`, `innerWidth` |
| `QHero` | icon at 2x (a 7 x 7 sprite, 2 vpx a pixel), title in ink over a muted caps line; children go at the right, vertically placed by the caller |
| `QRow` | a list row: icon, name (wraps to `maxLines`, only the last ends in an ellipsis), `sub` line, `detail` reading on the right, `current` (inverse block), `hasCursor` (brackets), `dimmed`; whatever is put inside is parked at its right and gets clicks first. Signals `clicked(button)`, `hovered()` |
| `QSlider` | stepped cells lit to the value, no knob: click or drag sets the cell under the pointer, wheel steps, right click is `rightClicked()`. `discrete` makes a picker of `maximum - minimum + 1` choices. `redline` lights cells past it in alarm |
| `QMeter` | the same cells as a full-width reading (charge, token use, a microphone's level) |
| `QSwitch` | a 15 x 9 box, a 5 x 5 knob left when off, right on the accent when on; brackets for the keyboard |
| `QButton` | a hairline legend a line tall (a `Tab`'s box); `active` is the accent block, `hasCursor` brackets, `danger`, an optional 7 x 7 icon |
| `QReading` | name muted left, value ink right; the name is dropped, not cut, when both do not fit |
| `QScroll` | whole-pixel scroll (a wheel notch is 14 vpx), a one-pixel mark at the right edge, `ensureItemVisible(item)`, a fixed gutter so wrapped text does not reflow when the mark appears |
| `QField` | a hairline box over a hidden `TextInput`: steady block cursor, the stretch around the cursor when long, `password`, `keyPressed(event)` |
| `QTip` `QEmpty` `BigText` | a raised hairline tooltip; an icon and a muted line for an empty list; PixelText at 2x or 3x |
| `PanelSprites` | 7 x 7 icons the bar and overlays do not have. Each clone keeps its own in an `Icons.qml` beside it rather than editing a shared singleton (also: a singleton is cached by the engine and needs a shell restart, an `Icons.qml` reloads with the plugin) |

All geometry is whole virtual pixels counted from the card; widths are in text cells
(`panel.cardWidth(44)`), so a label never lands on half a cell. Sections are `Group`
(`┌── NAME ──┐`, no sides, no bottom). The keyboard cursor and the pointer share one
mark, as the stock panels do.

Opening them for a screenshot: `omarchy-shell shell summon omarchy.audio '{}'` works
(it resolves the id to the enabled clone and calls the bar's `summonBarWidget`), the
popup opening on the focused output. `plugins/tools/popup-shots.sh` wraps it:
empty workspaces on both outputs first, a baseline grab, then each popup, then
the user's workspaces and focus back; a grab is discarded unless every output showed
an empty workspace. Hyprland 0.56 takes dispatchers as Lua expressions
(`hyprctl dispatch 'hl.dsp.focus({ workspace = "8" })'`); `focus({ monitor = "eDP-2" })`
moves between outputs.

### How a clone plugs in

`omarchy.clonedFrom` in the manifest, the stock id kept inside the code as its IPC
target (`ipcTarget`, `moduleName` unchanged: the host routes the stock id to the
enabled clone). `omarchy plugin enable quadrille.audio` replaces `omarchy.audio` in
the bar layout and `plugin disable` puts it back. The bar (`Bar.qml`:
`popupClones`, `stockIdOf`) treats a clone as the stock widget it replaces: it wears
the same skin and is handed the bar itself instead of the third-party `PluginBarApi`
facade (which has no `iconSlot` and would have given `bar.shell` the service-less
entry shell). The facade's `shell` still has `summon`, `updateEntryInline`,
`firstPartyServiceFor` (null) and `pluginCloneMaySummon` allows an audio clone
to summon the OSD and a network clone the speedtest and wifiqr.

## Status at pause (agent B: the bar and the kit)

(Superseded in part by "Agent B, resumed" at the end: the tray is on, the flicker is
fixed, the hang was not reproduced.)

**Done and verified** (both monitors, `crisp.py`-style: colour transitions only on
multiples of 2 px at scale 1 and 3 px at 1.666667, at most 7 colours a section):
the whole bar is now pixel-exact, with no stock glyph left in it. New this round:

* `skins/QIndicators.qml` replaces `omarchy.indicators`: dictation, screen
  recording, reminder, night light, DND, stay-awake as 7 x 7 sprites (lit in a
  tone, faint when off, unlit ones revealed with the centre hover). Same sources
  and `omarchy.indicators refresh` IPC as the stock widget.
* Skins over the stock widgets: `WeatherSkin` (a sprite per condition, from the
  panel's own `label` glyph, and the temperature as text), `UpdateSkin`,
  `AgentsSkin` (robot and a 4-cell gauge), `MediaSkin`, `MicrophoneSkin`,
  `ActiveWindowSkin`, `KeyboardSkin`. `Skin.deep(name)` finds a value the stock
  widget keeps in a child. 30 new sprites in `Q/Sprites.qml`
  (`plugins/tools/preview_sprites.py` draws them without a shell).
* Layout budget in `Bar.qml`: sections know where the centre group ends
  (`centerLeftEdge`/`centerRightEdge`), every slot is offered `room`, and elastic
  widgets (tray, media, title) give way; the right section is placed on the grid
  counted from the window's origin (a 2560-px surface is not a multiple of 3).
* `Q/PixelIcon.qml` + `Q/shaders/pixelicon.frag(.qsb)`: any app icon redrawn as
  11 x 11 (or any size) pixel art, in one fragment shader (4 x 4 box filter per
  cell, alpha cut at 0.5, 4 levels a channel, near-white/black low-saturation
  pixels take the ink so symbolic icons survive any theme). Rebuild shaders with
  `plugins/tools/build_shaders.sh` (`/usr/lib/qt6/bin/qsb`).

**In progress, disabled: the pixel tray** (`skins/QTray.qml`, `TrayMenu.qml`,
`PopupFrame.qml`; `Bar.qml` has `property bool pixelTray: false`). It renders
(isolated harness: pinned icons inline, a chevron popup listing the rest, menus
with submenus, separators, ticks, disabled rows) and `omarchy-shell quadrille.tray
drawer|manage|menu <i>|close` drives it. But: **with `fake_sni.py` items registered
the live shell stopped answering IPC for as long as they lived** (main thread asleep
at 0% CPU, not spinning, so a blocking wait, probably on the fake item's D-Bus, not
a QML loop; real items 1Password and Claude never did it). Not yet found whether it
is the tool (it may answer `GetAll` badly: give it a proper
`org.freedesktop.DBus.Properties`) or QTray. So the stock tray is back until
that is known. To resume: set `pixelTray: true`, test only with the real items first
(`omarchy-shell quadrille.tray drawer`), then with `fake_sni.py`, always in a
short flock.

**Next, in order:** (1) settle the tray hang as above, enable it, measure its
popups on both screens; (2) screenshots for `plugins/screenshots/` of the bar with
the new widgets (empty workspace, wallpaper only) and a `bar-states.png` of forced
states; (3) a QWorkspaces tab for an urgent workspace (`HyprlandWorkspace.urgent`);
(4) vertical bars: left/right keep stock widgets inside the pixel frame (only the
menu button is ours); bottom works (hairline on top, popups open upward) but is not
re-measured; (5) tailscale/dropbox skins (not in this layout); (6) the robot sprite
reads as an invader.

**Learned (a fresh agent needs these):**
* Never `pkill -f` a pattern that appears in your own `bash -c` command line: it
  kills the shell (exit 144). Use `pgrep -f '[p]attern'` or kill by pid.
* A command waiting for the flock counts against the 120 s tool limit and goes to
  the background: queue is long; keep sequences short and few.
* The empty-workspace switch needs `sleep 0.4` between `hl.dsp.focus({ monitor =
  "X" })` and `hl.dsp.focus({ workspace = "N" })`, or the second lands on the old
  monitor. Verify with `hyprctl monitors -j` before and after a grim
  (`scratchpad/shot.sh` does); workspaces 4 (HDMI-A-1) and 5 (eDP-2) are empty.
* `Canvas.loadImage("image://icon/...")` aborts the shell (QPixmap off the GUI
  thread), `drawImage(Image item)` draws nothing, `grabToImage` URLs (`itemgrabber:`)
  do not load in a Canvas: that is why `PixelIcon` is a shader.
* `window.devicePixelRatio` is the window's, `Screen.devicePixelRatio` is not.
* `plugins/tools/testroot.sh DIR` makes an isolated Quickshell root (the shell's
  `Commons`/`Ui` linked, our plugins linked by name) to try one component in, in
  seconds, without touching the live shell; `QSTEST_SCREEN=eDP-2` picks the screen.
* A scratch Quickshell that crashes posts a persistent "Process crashed: quickshell"
  critical toast: `omarchy-shell notifications dismissAll`.
* Agent P's popup clones (`quadrille.audio`, `quadrille.power`, ...) replace the
  layout ids; `Bar.qml`'s `popupClones` map makes them wear the original's skin.

## Status at pause (agent O: menu, OSD, notifications, overlays, app icons, wallpaper)

Done and checked (offscreen render at both scales, see the harness below):

* **Menu** (`quadrille.menu`): empty state no longer overflows its card (magnifier sprite and one line on the row's own height); the sub-text of the row under the cursor is on_accent (was muted on amber, unreadable); Nerd Font icon characters in labels are left out (they drew as boxes); the `✓` marker is a tick sprite on the right (the face has no `✓`; provider rows such as the font list now show the current one); an ellipsis no longer follows a space; the delete confirmation is the pixel `Confirm` (kit); icons are `AppIcon`. `LocalApps.iconSource` returns "" for a missing icon, and absolute-path icons are offered to an Image only once a one-shot `test -f` says the file exists, so the "Cannot open" warnings are gone and the entry shows the placeholder (a hairline box and the app's initial).
* **OSD** (`quadrille.osd`): touchpad, touch screen, download and `media` icons (were a bell); a message wraps over two lines instead of being cut at 36 characters; a message OSD with a volume icon is no longer drawn muted.
* **Notifications** (`quadrille.notifications`): the stack starts 4 vpx under the bar's real height (`Px.barVpx`, not `bar.barSize`, which says 32 px on a 29 px bar); an icon the sender named but that cannot be read is drawn as the placeholder instead of leaving a gap; glyph toasts lead with a pixel sprite (tick, cross, clipboard, plug, ...) instead of a vector glyph; blank lines in a body are collapsed.
* **Kit additions** (new files in `Q/`, one line each in `qmldir`): `Pictograms` (touchpad touchscreen download search clipboard smile image folder file text app key shield trash sun), `AppIcon` (wraps agent B's `PixelIcon`: placeholder on a missing file, plain picture if the pixel pass delivers nothing in 1.5 s), `Confirm` (pixel `Ui.ConfirmDialog`), `Scrim` + `shaders/dither.frag(.qsb)` (the dim layer as a 4 x 4 Bayer dither, one cell per vpx; plain translucent wash when the shader does not build). `Scrim` is written but NOT yet used by any overlay.
* **Wallpaper** (forked, finished): `plugins/quadrille.background` and `tools/gen_wallpapers.py`, commit b48117e. Its install.sh / stock.sh lines are NOT yet added (see next steps).

In progress / not started:

* Nothing half-done is enabled. No lock or polkit clone exists. Not started: reminders, emojis, clipboard, image picker clones; lock `LockView` and polkit restyles.
* `AppIcon` -> `PixelIcon` is agent B's GPU shader and has not been seen on screen by me; the offscreen harness stubs it (the software renderer draws ShaderEffect black). First thing to do after a shell restart: open `omarchy-menu summon apps` over an empty workspace and look at the icons at both scales; if they are wrong, `AppIcon` still falls back to the plain picture after 1.5 s.

Next steps, in order:

1. Verify live (restart the shell inside the flock): menu Apps icons, an OSD (`omarchy-osd -i volume-high -p 60 -d 4000`), two toasts (`notify-send`, then delete the new files in `~/.local/state/omarchy/notifications/history/`). Fix what the GPU path shows.
2. Add `quadrille.background` to the `ids` arrays (lab and `*` cases) of `plugins/install.sh` and to both `for id in` lists of `plugins/stock.sh`; paste the wallpaper section (below) into NOTES.md.
3. Overlays as clones (`kinds: ["overlay"]`, `keepLoaded: true`, `omarchy.clonedFrom: omarchy.reminders|emojis|clipboard|image-picker`, `Q -> ../quadrille.bar/Q`): reminders (a prompt line with a block caret), emojis (grid, each emoji through a `PixelIcon`-style pipeline fed by a `Text` so it is nearest-neighbour at an integer scale; the host emoji are colour bitmaps), clipboard (keep the `wl-paste --watch` Processes and the `pkill` init; list + preview; never open it live, it shows the real history: test with `historyPath` pointed at a mock file), image picker (a grid with `Brackets` on the selection, labels dropped not cut; keep `open`, `preloadRows`, `closeSelector`). Use `Scrim` for the dim layer.
4. Lock (`LockView.qml` only, the service stays verbatim) and polkit: only if tested in the nested compositor (`layershell/tools/nested.sh`); `lock preview` shows LockView without taking the session lock and is the safe live test. Neutralise `omarchy-brightness-*` and `omarchy-system-wake` in any test copy. Ship disabled otherwise.

The offscreen harness (kept in the agent's scratch dir, not the repo): a `PanelWindow` is swapped for a `FloatingWindow` in a throwaway copy of the plugin, the kit's `PixelIcon` is stubbed, and `QT_QPA_PLATFORM=offscreen QT_SCALE_FACTOR=1.666667` gives the laptop's fractional scale; `grabToImage` of an inner item gives device-resolution PNGs. Pitfalls found: a QML `id` shadows a same-named property of the root (`readout` in Osd.qml); an asynchronous `Image` on an `image://icon/` URL aborts the process; keep the harness's output outside the watched config directory or it hot-reloads; `QT_SCALE_FACTOR` works but `ShaderEffect`/`layer.textureSize` do not render in the software scene graph (the live shell must check them).

## Status at pause (agent P)

**Done and verified**
- Theme tokens (`tools/gen_themes.py`, themes regenerated, committed, applied with
  `omarchy-theme-set "Quadrille Terminal"`): `[spacing] scale = 2.0`, `scale-with-font =
  false`; heading 22, display 44, display-large 44, icon-large 33. Checked with the
  tokens pushed in memory (`omarchy-shell shell applyTheme`): audio, bluetooth, network,
  monitor, power, weather, clock and agents on HDMI-A-1; weather, audio, network,
  bluetooth, monitor, power, clock, agents on eDP-2 (no overlap, clipping or truncation;
  the long lists scroll). With tokens alone (spacing 1) the weather card still overlapped.
  NOT checked: tailscale (not installed here, not in the bar layout), the speedtest /
  wifiqr / dropbox panels; and the final regenerated file was applied and the bar seen
  after, but the popups were re-checked with the in-memory copy (icon-large 33 vs 22 only).
- Kit (committed): QPopup QHero QRow QSlider QMeter QSwitch QButton QReading QScroll
  QField QTip QEmpty BigText PanelSprites; `plugins/tools/popup-shots.sh`, `popup-crop.py`.
- Clones (committed, enabled): `quadrille.power`, `quadrille.audio`. Measured with
  layershell/tools/crisp.py on both outputs: 7-11 colours, every transition on the grid
  (eDP-2 mod 3, HDMI mod 2). Audio checked with two silent streams (long name wraps).
- `Bar.qml` has `popupClones` / `stockIdOf` for all nine ids (skins and first-party bar
  handout), committed as a minimal hunk.

**In progress / not started**
- No half-done clone is enabled. `plugins/tools/popup-brief.md` is the brief written for
  sub-agents (rules, grid, kit, deploy loop, shared-file rules): hand it to each one.
- Not started: bluetooth, network, monitor, tailscale, agents, weather, clock clones;
  `plugins/install.sh` / `stock.sh` still list only the old plugins (add the new ids:
  quadrille.audio quadrille.power ...; enable a clone only if its stock widget is in the
  layout, tailscale is not); a specimen sheet of the kit states; NOTES re-measure of the
  bar after `[spacing] scale 2.0` (positions looked unchanged on HDMI-A-1).

**Next steps, in order**
1. bluetooth, monitor, network, agents, tailscale: one clone each from the brief
   (`quadrille.<x>` = `cp -aL` of the stock plugin dir + `Q` symlink + manifest with
   `omarchy.clonedFrom`; keep every line above `KeyboardPanel {`, replace the view).
   Weather and clock are center-section widgets: check `centerAnchor` (config says
   `omarchy.clock`; after `plugin enable quadrille.clock` the layout id changes, so
   `entryIndex(entries, centerAnchor)` in Bar.qml must resolve it through `stockIdOf`).
   The clock clone uses `bar.shell.updateEntryInline`: keep its `typeof` guard.
2. install.sh / stock.sh, then a full stock round trip (`plugins/stock.sh`, check the
   stock popups, `plugins/install.sh`).
3. Re-run popup-shots for every popup, both outputs, crisp.py on each card.

**Pitfalls**
- `omarchy-shell` answers "not responding" for a few seconds after a rescan or when
  another agent restarts the shell: loop on `shell ping` before summoning.
- `omarchy-shell shell summon omarchy.<x> '{}'` opens the popup on the *focused* output;
  popup-shots focuses the output first. Hyprland 0.56 dispatch is
  `hyprctl dispatch 'hl.dsp.focus({ monitor = "eDP-2" })'`.
- Edits to singletons (Px, Role, Sprites, PanelSprites) need `omarchy-restart-shell`.
- Other agents' test surfaces appear in grabs (red brackets, tray lists); crop them out.
- A screenshot taken without the empty-workspace check showed windows once: only use
  popup-shots.sh, and Read every PNG before keeping it.
- `qmldir`, `Bar.qml`, `Sprites.qml` are edited by several agents: stage only your
  hunks (HEAD content plus your replacement) rather than the whole file.

## One pixel size to a surface: native sprites, no mixels (agent P)

The user's rule: no mixels, a mixed pixel size. A 7 x 7 sprite drawn at 2x or 3x
beside text and 1x icons is a mixel, so an icon that spans more than a text row is
**its own sprite at its own native size**, with the detail those pixels allow (an
outline in `#`, a fill in a second tone, inner shapes), and it must read as the
same family as the 7 x 7 one. The popups follow it:

| Slot | Size | Where |
|---|---|---|
| a list row, a button | 7 x 7 | `Sprites`, `PanelSprites` |
| a two-line hero | 15 high | audio `Icons.qml` (speaker with two waves lit by `level`, muted with an accent cross, headphones), clock `Icons.qml` (calendar, today in the accent), power `Icons.qml` (a 17 x 9 battery of four cells) |
| a weather condition | 21 x 21 | weather `Icons.qml`: sun, moon, cloud, cloud with sun or moon, fog, drizzle, rain, sleet, thunder, snow; outline `#`, fill `2` (muted), sun and lightning `!` (caution). Same set in the hero and the forecast. The bar's 9 x 7 ones are the small members of the family |

Sprite tones: `#` and any digit up to `level` are the ink, digits above it are the
`dim` colour, `!` is `accent`; so `level: 1` with `dim: Role.muted` gives ink, muted
and an accent tone, which is what the hero sprites use (`QHero` passes `iconDim`,
`iconAccent`; `iconScale` stays 1). The conditions were drawn by a small generator
(circles for the lobes, an outline of every shape, drops and rays as lines) and
judged on a sheet, not scaled from anything.

What is still bigger than one pixel: the hero number of a popup (the battery
percentage, the temperature, the date) is `BigText`, quadrille's DISPLAY and HERO
faces (Departure Mono at 22 and 33, a font pixel of 2 or 3 vpx). It is one per popup
and it is text, not an icon; if the user wants it gone too, the hero reading becomes
body text in an inverse block.

## Agent B, resumed: the size flicker, the mixel rule, the tray hang

### The size flicker on open (the user's top bug), measured

"An odd size flickering when opening hovers and some of the widget windows, and
opening between them", on the laptop (1.666667). It was not eyeballed: every
popup window now carries a `SurfaceProbe` (`Q/SurfaceProbe.qml`, silent unless the
shell has `QUADRILLE_DEBUG_SURFACES=1`), which logs every change of the window's
size, visibility, screen and device ratio, and (polling every 4 ms while it is up)
of what the card holds, with `Date.now()`, as `SURF <ms> <tag> <event> | size= vis=
screen= dpr= unit= ...`. A scratch shell (`plugins/tools/surfaces/`) runs the
real bar and widgets on a private session bus and a private `HOME`, inside the
nested Hyprland (`layershell/tools/nested.sh`, outputs QA 1.666667 and QB 1),
opens each popup through the shell's own IPC while `grim` records the corner of
the output every ~33 ms, and reads the log.

**H1 was the cause, and it is real.** A window that has just been shown reports
the *default output's* integer scale for its first frames, and the kit took its
unit from that. For a popup (`PopupWindow`) it is worse: its `screen` is the first
output's too, until it has been mapped. Log of the power popup on QA, before:

    +  0 ms  visible=true          dpr=2       unit=2.0  card at 980,40,  548 wide, pad 8
    + 46 ms  dpr=1  (screen QA)
    + 50 ms  dpr=1.6667            unit=1.8  card at 1035,36, 493 wide, pad 7.2     <- jumps 10 %

and the bar's tooltip (a `PopupWindow`): 158x32 at first, then 143x29 at +59 ms,
then 158x32 again when its `screen` changes at +60 ms, then 143x29: three sizes
in 20 ms. A popup's card is built from the owner's lengths (the bar window's unit,
right from the start) plus its own chrome (the popup window's unit, wrong at
first), so even the mixture was wrong. The tray's drawer, laid out for unit 2,
opened 10 % too big and shrank.

The fix, in the kit, so every surface gets it:

* `Px.forWindow(win)` takes the unit from **the output's real scale**
  (`Hyprland.monitors`, rounded to the 1/120 steps of wp-fractional-scale) of the
  window it *hangs from* (`anchor.window`, else `parentWindow`, else `screen`),
  which is known before the surface exists; the window's own ratio is only the
  fallback (no Hyprland, or a monitor not listed yet). `QUADRILLE_LEGACY_DPR=1`
  in a scratch shell turns it off, to measure the old way. After the fix the
  same probe reads `unit=1.8` from the first line and the card is laid out once.
* `Q/Settle.qml`: the first frame of a window is still rasterised at the wrong
  ratio (soft for a frame). A card or a bubble that is `visible: settle.ready`
  skips that frame (and shows anyway after 120 ms if the compositor never says).
* `Q/SizeGate.qml` (H2): a card is sized from data that arrives after it is open
  (the power popup: 120, 142, then 196 px tall in the first 210 ms; the clock:
  380 then 416; the tray menu: 15 then 141, as the D-Bus menu arrives). Drawn at
  once it opens at one size and jumps. `QPopup` holds its card, and `PopupFrame`
  does not map its window, until the size and place have stayed the same for three
  polls of 16 ms, or 260 ms have passed since opening: a static popup is ready as
  soon as its window is, one waiting for data waits at most that long.
* Hand-over between popups (the "opening between them" half): `requestPopout`
  closed the old popup at once and the new one needed 50-90 ms to be mapped, so the
  bar was bare in between. A new popup now sets `bar.popoutHandoff`; the one it
  replaces stays on screen (`handingOff`) until the newcomer's card is drawn (20 ms
  after, so the swap is one frame) or 400 ms. `PopupFrame` also holds its
  `dismissed()` (which resets a menu) until then.

On screen, from the grim frames (about 33 ms apart, `screenshots/tooltip-first-frames.png`):
the tooltip's first frame before the fix is 263 x 53 device px and soft, the next
237 x 48; after, the first frame is 237 x 48 and crisp and stays. The power popup
appears once, at its final 822 device px, and a switch from it to the clock never shows
the bar bare: the changed-pixel count of the frames stays above 40 000 while the old
card waits for the new one.

H3 (the host's popup chrome, 140 ms fades and resizes) does not apply to the
clones, which have no animation at all; it still applies to the stock panels that
are not cloned (speedtest, wifi QR, tailscale, dropbox, the dev gallery). They use
the host's `PopupCard`/`KeyboardPanel`, which this plugin cannot change; they can
only be cloned. H4 (the host's `PanelToolTip`) sizes itself from its text after it
is shown, inside the panel's own window, and draws in the distance-field renderer:
the kit's `QTip` and the bar's tooltip size from bitmap metrics before they show.

What O and P still have to do for their surfaces (not done here, not mine):

* every `PanelWindow` that appears on "whatever has the focus" (the OSD, the menu,
  the clipboard, emojis, the image picker, reminders, a toast) is created with no
  `screen`, so Qt thinks it is on the first output until it is mapped and its first
  frame is laid out for the wrong one. Give it one before `visible` goes true:
  `property var shownOn: null; screen: shownOn`, and set
  `shownOn = Px.focusedScreen()` in the function that opens it (`Px.focusedScreen()`
  is the screen of Hyprland's focused monitor; it returns null if it cannot tell).
* wrap what the window draws in `Settle` (`visible: settle.ready`), and, if the
  content comes from a model or a process, in a `SizeGate` too;
* put a `SurfaceProbe { window: root; tag: "..."; extra: function() { ... } }` in the
  window and read it with `plugins/tools/surfaces/` (`build.sh`, then `flick.sh A|B`
  in one locked hold under `flock -o`, then `analyse.py` and `frames.py`).

### One pixel size to a surface: the guard

`Sprite` reports (once per size) any `unit` other than the surface's own:
`quadrille: a 7 x 7 sprite is drawn at 2.00x the surface's pixel (a mixel)`; it
still draws it, so an old caller does not vanish. `Sprites.qml` states the sizes
(7, 11, 15, 21, odd, one drawing each, the small one never scaled) and has a second
tone (`o`, the sprite's `mid`) and an always-dim one (`,`) for the shading a bigger
icon is allowed. Audit of what was there: nothing of the bar, the kit or the
overlays passes a `unit` any more (`QHero` default is 1, the popup heroes are native
15 and 21 sprites, the OSD's are 13); what is still scaled is `BigText`, the hero
number of the clock, power and weather popups (P's note above). BigText is the one thing left that is scaled: not mine, P's note above names it.

### The tray hang: not reproduced, so the pixel tray is switched on, with a switch

The freeze under `fake_sni.py` could not be made to happen again. Tried, each with
four fake items (pinned and in the drawer), the drawer, a menu and the manage
popup opened over IPC while the shell was pinged every second:

1. a bare `SystemTray` model in a scratch shell on a private session bus: no hang;
2. `QTray` in a `FloatingWindow` offscreen, and in a `PanelWindow` on the live
   compositor: none;
3. the whole `quadrille.bar` with every widget, in a copy of the shell with its own
   `HOME` and no services (`plugins/tools/surfaces/build.sh` builds it): none;
4. the same on the nested compositor with three bars (the nested window's output,
   QA, QB): none.

The first freeze therefore came from something else in that live session (it was
seen with the ultrawide plugged in, other agents restarting the shell and a gdb/
eu-stack attach on it; the "main thread asleep at 0 %" in the pause note was not
backed by a sample). One real difference in `fake_sni.py` was found and fixed: all
its items shared one bus connection, so when it died the watcher dropped only one
of them; each item now has its own connection, as an application would. `QTray`
is on by default; `QUADRILLE_PIXEL_TRAY=0` in the shell's environment, or
`pixelTray: false` in `Bar.qml`, brings the stock tray back. If a freeze ever
shows up with real items, run `plugins/tools/surfaces/tray.sh` (the nested three-bar
tray run) and look at
`top -H` and `gdb -p` of the scratch shell (it is a child of the script, so ptrace
is allowed), not the live one.

### Bar orientations, and what is left of the list

* **Top bar**: everything above is measured on it (both outputs).
* **Bottom bar**: the frame, the skins and the popups (`PopupFrame` and `QPopup` open
  upward when `bar.position === "bottom"`) are written for it; it was not re-measured
  this round.
* **Left and right bars: unsupported, said plainly.** The skins and the replacements
  are for a horizontal bar only (`Bar.qml` turns them off when `vertical`, the stock
  widgets sit inside the pixel frame, only the menu button is ours); the tray, the
  indicators, the weather and the rest would each need a vertical drawing. Not a
  regression: they never were.
* Not done: a tab for an urgent workspace (`HyprlandWorkspace.urgent`), tailscale and
  dropbox skins (not in this layout), hover/pressed/focus states forced and measured
  for every skin, a polish of the sysmon gauges, the 9 x 7 weather sprites redrawn
  (cloud with sun or moon are still lumpy).
