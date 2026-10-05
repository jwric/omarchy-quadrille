import QtQuick
import "."
import "Glyphs.js" as Glyphs

// Wrapped text in the body face: `columns` cells wide, whole 12-vpx lines, at
// most `maxLines` of them, the last ending in an ellipsis when cut. Wrapping is
// on the cell grid (a space, a newline, or anywhere in a word too long for a
// line), so the width is columns x 6 vpx and the height lines x 12 vpx.
Item {
  id: root

  readonly property var g: Px.of(root)

  property string text: ""
  property color ink: Role.ink
  property int columns: 30
  property int maxLines: 3

  readonly property var lines: Glyphs.wrap(text, columns, maxLines)

  width: columns * g.cellW
  height: lines.length * g.line
  visible: text.length > 0

  Repeater {
    model: root.lines
    PixelText {
      required property string modelData
      required property int index
      y: index * root.g.line
      text: modelData
      ink: root.ink
    }
  }
}
