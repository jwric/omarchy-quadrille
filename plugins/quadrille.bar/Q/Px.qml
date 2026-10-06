pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.Commons

// The pixel grid, per window.
//
// A virtual pixel ("vpx") is a whole number of *device* pixels. The theme says
// how many logical pixels it was designed to be (`[quadrille] unit`, else the
// body font token / 11: 2), which is right at scale 1 and 2 but not at 1.666667,
// where 2 logical px are 3.33 device px. So the unit is resolved against the
// window it is drawn in:
//
//     phys = max(1, round(target * dpr))      device px per vpx
//     unit = phys / dpr                       logical px per vpx
//
// (1.8 logical px at 1.666667; 2 at scale 1, unchanged.) Every length in the
// kit is a count of vpx times `unit`, so every edge is a whole number of device
// pixels. Qt Quick takes the fractional logical coordinates; the scene graph
// multiplies them by dpr and they come out whole.
//
// Use it from an Item with `readonly property var g: Px.of(root)`, or from a
// window with `Px.forWindow(win)`. `g` is a plain object: `g.px(n)`, `g.unit`,
// `g.cellW`, `g.line`... The dpr is the *window's*: `Screen.devicePixelRatio`
// says 2 on a 1.666667 output, because Qt reports the integer output scale; the
// window's `devicePixelRatio` is the fractional one once it is mapped.
QtObject {
  id: root

  // Logical px per vpx the theme was made for.
  readonly property int target: {
    var stated = parseInt(Color.shellValues["quadrille.unit"], 10)
    if (isFinite(stated) && stated > 0) return stated
    return Math.max(1, Math.round(Style.font.body / 11))
  }
  // The bar's height in vpx, from the theme's bar size.
  readonly property int barVpx: Math.max(4, Math.round(Style.bar.sizeHorizontal / target))

  // The body face, for stock widgets that take their label font from the bar.
  // (Our own text is drawn from bitmaps: see PixelText and tools/gen_glyphs.py.)
  readonly property string face: "Departure Mono Tight"

  property var cache: ({})

  // The scale of the output a window is on, as the compositor has it (Hyprland's
  // monitor scale, in the 1/120 steps wp-fractional-scale speaks), or 0 if it
  // cannot be told. It is known from the start, where a new window's own
  // `devicePixelRatio` is the integer scale of the output (2 on a 1.666667 one)
  // until the compositor's `preferred_scale` arrives, a frame or more after the
  // surface is first drawn: a popup laid out from that would open on a grid of
  // 2 logical px a vpx and then jump to 1.8.
  // (QUADRILLE_LEGACY_DPR=1 in a scratch shell turns it off, to measure what the
  // window-only way did.)
  readonly property bool legacyDpr: Quickshell.env("QUADRILLE_LEGACY_DPR") === "1"
  function outputScale(win) {
    if (legacyDpr) return 0
    // A popup's own `screen` is, until it has been shown, the default screen (the
    // first output), not the one it opens on: it is the window it hangs from that
    // says. A window with a `screen` of its own (a panel) says it itself.
    var screen = null
    if (win) {
      if (win.anchor && win.anchor.window) screen = win.anchor.window.screen
      if (!screen && win.parentWindow) screen = win.parentWindow.screen
      if (!screen) screen = win.screen
    }
    var name = screen ? screen.name : ""
    if (name === "" || !Hyprland.monitors) return 0
    var monitors = Hyprland.monitors.values
    for (var i = 0; i < monitors.length; i++) {
      var m = monitors[i]
      if (m && m.name === name && m.scale > 0) return Math.round(m.scale * 120) / 120
    }
    return 0
  }

  // The screen of the output that has the focus, to hand to a window that is shown
  // on whatever has the focus (an OSD, a menu, a toast): `screen: shownOn`, set
  // from this just before `visible` goes true. A layer surface with no `screen` is
  // put on the focused output by the compositor, but Qt's own idea of its screen
  // stays the first output's until the surface has been mapped, so its first frame
  // is laid out for the wrong one. null if it cannot be told.
  function focusedScreen() {
    var focused = Hyprland.focusedMonitor
    var name = focused ? focused.name : ""
    var screens = Quickshell.screens
    for (var i = 0; i < screens.length; i++) if (screens[i].name === name) return screens[i]
    return null
  }

  function forWindow(win) {
    var dpr = outputScale(win)
    if (!(dpr > 0)) dpr = win ? win.devicePixelRatio : 1
    return grid(dpr > 0 ? dpr : 1)
  }

  // True once the window itself has the scale the grid was made for: until then
  // a surface that was just mapped is drawn at the wrong device ratio (soft, for
  // a frame). Popups keep their content invisible until it is.
  function settled(win) {
    if (!win) return true
    var want = outputScale(win)
    return !(want > 0) || Math.abs(win.devicePixelRatio - want) < 0.01
  }

  function of(item) {
    // A Quickshell window says so (QsWindow); a session-lock surface is not one,
    // and is the QtQuick window of the item (Window), with a device pixel ratio of
    // its own.
    var win = item ? (item.QsWindow.window || item.Window.window) : null
    return forWindow(win)
  }

  function grid(dpr) {
    var key = target + "@" + Math.round(dpr * 1e6)
    var hit = cache[key]
    if (hit) return hit
    var g = makeGrid(dpr, target)
    cache[key] = g
    return g
  }

  function makeGrid(dpr, target) {
    var phys = Math.max(1, Math.round(target * dpr))
    var unit = phys / dpr
    var eps = 1e-6
    var g = {
      dpr: dpr,
      phys: phys,
      unit: unit,
      // quadrille's spacing scale
      hair: unit,
      tight: 2 * unit,
      gap: 4 * unit,
      wide: 8 * unit,
      far: 16 * unit,
      // the body face: a 6 x 12 cell
      cellW: 6 * unit,
      line: 12 * unit,
      cap: 8 * unit,
      capTop: 2 * unit,
      baseline: 10 * unit,
      icon: 7 * unit,
      // a bar, and the window that holds it (a layer surface is a whole number
      // of logical px; its buffer is then round(logical * dpr))
      bar: barVpx * unit,
      barWindow: Math.ceil(barVpx * unit - eps),
      px: function(n) { return n * unit },
      // the nearest whole vpx, the one below, the one above
      snap: function(v) { return Math.round(v / unit) * unit },
      floor: function(v) { return Math.floor(v / unit + eps) * unit },
      ceil: function(v) { return Math.ceil(v / unit - eps) * unit },
      // `v` up to a whole number of device pixels (for what we do not draw: a stock
      // widget's width, so what follows it still starts on a device pixel)
      whole: function(v) { return Math.ceil(v * dpr - 1e-6) / dpr },
      // The offset that centres `inner` in `outer`, rounded down to a whole vpx,
      // so an odd leftover always falls on the same side.
      centre: function(outer, inner) { return Math.floor((outer - inner) / (2 * unit) + eps) * unit },
      // y that centres a mark `inner` vpx tall on the capitals of a line of text
      onCaps: function(inner) { return 2 * unit + Math.floor((8 - inner) / 2) * unit },
      cells: function(n) { return n * 6 * unit },
      // whole cells that fit in `width`
      columns: function(width) { return Math.max(0, Math.floor(width / (6 * unit) + eps)) },
      // whole vpx in `length`
      vpx: function(length) { return Math.floor(length / unit + eps) }
    }
    return g
  }
}
