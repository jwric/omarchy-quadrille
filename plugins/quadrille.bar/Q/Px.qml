pragma Singleton
import QtQuick
import Quickshell
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

  function forWindow(win) {
    var dpr = win ? win.devicePixelRatio : 1
    return grid(dpr > 0 ? dpr : 1)
  }

  function of(item) {
    var win = item ? item.QsWindow.window : null
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
