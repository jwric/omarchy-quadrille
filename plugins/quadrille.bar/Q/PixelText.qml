import QtQuick
import "."
import "Glyphs.js" as Glyphs

// Text in the body face, drawn from bitmaps: every font pixel a rectangle of
// one vpx, so it is whole device pixels on any screen. (A Qt Text cannot be
// that: see tools/gen_glyphs.py.) Departure Mono Tight at its native size on a
// 12-vpx line: capitals on rows 2-9, the baseline at row 10, six columns a
// character. The item is exactly `length x 6` vpx wide and one line tall, so
// layout is arithmetic.
//
// Characters the font lacks are drawn as a box (the font's own missing glyph).
Item {
  id: root

  readonly property var g: Px.of(root)

  property string text: ""
  property color ink: Role.ink
  // Cut to this many cells, ending in an ellipsis (quadrille's `Face::fit`);
  // -1 leaves it be.
  property int columns: -1
  // Drop the label instead of cutting it: hidden when it would be wider than
  // `room` (logical pixels). -1 leaves it alone.
  property real room: -1

  readonly property string shown: columns >= 0 ? Glyphs.fit(text, columns) : text
  readonly property int cells: Glyphs.length(shown)
  readonly property real cellsWidth: cells * g.cellW

  width: cellsWidth
  height: g.line
  visible: room < 0 || cellsWidth <= room

  Repeater {
    model: Glyphs.runs(root.shown)
    Rectangle {
      required property var modelData
      x: modelData.x * root.g.unit
      y: modelData.y * root.g.unit
      width: modelData.w * root.g.unit
      height: root.g.unit
      color: root.ink
      antialiasing: false
    }
  }
}
