import QtQuick
import "."
import "Glyphs.js" as Glyphs

// PixelText at a whole multiple of its size: every font pixel is `scale`
// virtual pixels square (a hero number: 2 is Departure Mono at 22, the size the
// toolkit calls DISPLAY). Cells are `6 * scale` vpx wide and the line `12 * scale`
// tall, so layout is still arithmetic, and every edge is a whole device pixel
// for the same reason PixelText's are.
Item {
  id: root

  readonly property var g: Px.of(root)

  property string text: ""
  property color ink: Role.ink
  property int scale: 2

  readonly property int cells: Glyphs.length(text)
  readonly property real cellWidth: g.cellW * scale

  width: cells * cellWidth
  height: g.line * scale

  Repeater {
    model: Glyphs.runs(root.text)
    Rectangle {
      required property var modelData
      x: modelData.x * root.g.unit * root.scale
      y: modelData.y * root.g.unit * root.scale
      width: modelData.w * root.g.unit * root.scale
      height: root.g.unit * root.scale
      color: root.ink
      antialiasing: false
    }
  }
}
