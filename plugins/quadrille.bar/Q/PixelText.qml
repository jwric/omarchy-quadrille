import QtQuick
import "."

// Text in the body face: Departure Mono Tight at its native size, drawn by the
// platform (no distance field, no smoothing), on a whole 12-vpx line with the
// capitals two vpx below its top and the baseline ten down, as quadrille sets
// it.
//
// The item is exactly `text.length` cells wide and one line tall, so layout is
// arithmetic: width = cells * Px.cellW. Characters the face does not have fall
// back to a foreign advance and put everything after them off the grid; keep
// to what Departure Mono has.
Item {
  id: root

  property alias text: label.text
  property color ink: Role.ink
  // Drop the label instead of cutting it: hidden when it would be wider than
  // `room` (logical pixels). -1 leaves it alone.
  property real room: -1

  readonly property int cells: label.text.length
  readonly property real cellsWidth: cells * Px.cellW

  width: cellsWidth
  height: Px.line
  visible: room < 0 || cellsWidth <= room

  Text {
    id: label
    // Qt puts the baseline at the font's ascent (11 vpx) from the top of the
    // line; quadrille's line puts it at 10. One vpx up.
    y: -Px.unit
    color: root.ink
    font.family: Px.face
    font.pixelSize: Px.size
    font.hintingPreference: Font.PreferFullHinting
    font.kerning: false
    font.preferShaping: false
    renderType: Text.NativeRendering
    textFormat: Text.PlainText
    wrapMode: Text.NoWrap
    elide: Text.ElideNone
  }
}
