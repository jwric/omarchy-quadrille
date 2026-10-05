pragma Singleton
import QtQuick
import qs.Commons

// The pixel grid. A virtual pixel ("vpx") is `unit` logical pixels, and every
// length in the kit is a whole number of them, counted in vpx and multiplied
// out here. quadrille's scale is 1, 2, 4, 8, 16; its face is Departure Mono
// Tight at its native 11 (a 6 x 12 cell).
//
// `unit` follows the theme: shell.toml pins every font token to a whole
// multiple of Departure Mono's native 11, so body / 11 is the unit the theme
// was generated for. `[quadrille] unit` states it outright when present.
QtObject {
  id: root

  readonly property int unit: {
    var stated = parseInt(Color.shellValues["quadrille.unit"], 10)
    if (isFinite(stated) && stated > 0) return stated
    return Math.max(1, Math.round(Style.font.body / 11))
  }

  // quadrille's spacing scale, in logical pixels.
  readonly property int hair: unit        // 1  rules, borders, a gap that only separates
  readonly property int tight: 2 * unit   // 2  inside a cluster
  readonly property int gap: 4 * unit     // 4  between siblings
  readonly property int wide: 8 * unit    // 8  between groups
  readonly property int far: 16 * unit    // 16 between regions

  // The body face.
  readonly property string face: "Departure Mono Tight"
  readonly property int size: 11 * unit   // native em
  readonly property int cellW: 6 * unit   // advance of every glyph
  readonly property int line: 12 * unit   // a line of text
  readonly property int cap: 8 * unit     // height of a capital
  readonly property int capTop: 2 * unit  // line top to cap top
  readonly property int baseline: 10 * unit
  readonly property int icon: 7 * unit    // pixel icons are 7 x 7

  // Whole virtual pixels.
  function px(n) { return n * unit }
  function snap(v) { return Math.round(v / unit) * unit }
  function floor(v) { return Math.floor(v / unit) * unit }
  function ceil(v) { return Math.ceil(v / unit) * unit }
  // The offset that centres `inner` in `outer`, rounded down to a whole vpx,
  // so an odd leftover always falls on the same side.
  function centre(outer, inner) { return Math.floor((outer - inner) / (2 * unit)) * unit }
  // y that centres a mark `inner` vpx tall on the capitals of a line of body
  // text (a lamp, a 7-vpx icon).
  function onCaps(inner) { return capTop + Math.floor((cap - inner * unit) / (2 * unit)) * unit }
  // Width of `n` cells of text.
  function cells(n) { return n * cellW }
  // How many whole cells fit in `width`.
  function columns(width) { return Math.max(0, Math.floor(width / cellW)) }
}
